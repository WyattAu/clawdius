//! Ollama local provider

use async_trait::async_trait;
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{ModelIden, ServiceTarget};
use tokio::sync::mpsc;

use crate::llm::providers::{ChatWithToolsResult, LlmClient};
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct OllamaProvider {
    client: genai::Client,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: Option<&str>) -> Result<Self> {
        let model_name = model.unwrap_or("llama3.2").to_string();
        let base_url = normalize_base_url(base_url);

        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> genai::resolver::Result<ServiceTarget> {
                let ServiceTarget { model, .. } = service_target;
                let endpoint = Endpoint::from_owned(base_url.clone());
                let auth = AuthData::from_single(String::new());
                let model = ModelIden::new(AdapterKind::Ollama, model.model_name);
                Ok(ServiceTarget {
                    endpoint,
                    auth,
                    model,
                })
            },
        );

        let client = genai::Client::builder()
            .with_service_target_resolver(target_resolver)
            .build();

        Ok(Self {
            client,
            model: model_name,
        })
    }
}

/// genai's Ollama adapter builds `{base_url}api/chat` by concatenation, so
/// a base URL without a trailing slash produces `http://localhost:11434api/chat`
/// and reqwest fails with a bare "builder error" — no hint that the URL is
/// malformed. Normalise once, here.
fn normalize_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    format!("{trimmed}/")
}

#[async_trait]
impl LlmClient for OllamaProvider {
    async fn chat(&self, messages: Vec<ClawdiusMessage>) -> Result<String> {
        let genai_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                ChatRole::System => ChatMessage::system(m.content),
                ChatRole::User => ChatMessage::user(m.content),
                ChatRole::Assistant => ChatMessage::assistant(m.content),
            })
            .collect();

        let chat_req = ChatRequest::new(genai_messages);

        let response = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        response
            .first_text()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| Error::Llm("No response text".into()))
    }

    async fn chat_stream(&self, messages: Vec<ClawdiusMessage>) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);

        let genai_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                ChatRole::System => ChatMessage::system(m.content),
                ChatRole::User => ChatMessage::user(m.content),
                ChatRole::Assistant => ChatMessage::assistant(m.content),
            })
            .collect();

        let chat_req = ChatRequest::new(genai_messages);
        let client = self.client.clone();
        let model = self.model.clone();

        tokio::spawn(async move {
            match client.exec_chat_stream(&model, chat_req, None).await {
                Ok(stream_response) => {
                    let mut stream = stream_response.stream;
                    let mut had_error = false;
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(event) => {
                                if let genai::chat::ChatStreamEvent::Chunk(chunk) = event {
                                    if tx.send(chunk.content).await.is_err() {
                                        break;
                                    }
                                }
                            },
                            Err(e) => {
                                had_error = true;
                                tracing::error!("Ollama stream error for model {}: {}", model, e);
                                break;
                            },
                        }
                    }
                    if had_error {
                        drop(tx);
                    }
                },
                Err(e) => {
                    tracing::error!("Ollama stream init error for model {}: {}", model, e);
                    drop(tx);
                },
            }
        });

        Ok(rx)
    }

    /// Native tool calling through Ollama's `/api/chat` `tools` field.
    ///
    /// The default trait impl refuses, which made every local run of the
    /// dev loop fail at the implementation stage. Ollama has supported
    /// tools since 0.3 and `llama3.2` is the flagship tool-calling model,
    /// so the local path can run the loop with no quota at all.
    async fn chat_with_tools(
        &self,
        messages: Vec<ClawdiusMessage>,
        tools: Vec<genai::chat::Tool>,
    ) -> Result<ChatWithToolsResult> {
        let genai_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                ChatRole::System => ChatMessage::system(m.content),
                ChatRole::User => ChatMessage::user(m.content),
                ChatRole::Assistant => ChatMessage::assistant(m.content),
            })
            .collect();

        let chat_req = ChatRequest::new(genai_messages).with_tools(tools);
        let response = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        let text = response.content.first_text().unwrap_or("").to_string();
        let tool_calls: Vec<genai::chat::ToolCall> =
            response.content.tool_calls().into_iter().cloned().collect();

        Ok(ChatWithToolsResult { text, tool_calls })
    }

    fn count_tokens(&self, text: &str) -> usize {
        // Detect if content looks like code
        let is_code = text.contains("fn ")
            || text.contains("function ")
            || text.contains("class ")
            || text.contains("def ")
            || text.contains("import ")
            || text.contains("export ");

        if is_code {
            let char_count = text.chars().count();
            let punct_count = text.chars().filter(|c| c.is_ascii_punctuation()).count();
            ((char_count as f64 / 4.0).ceil() as usize) + (punct_count / 3).max(1)
        } else {
            let words = text.split_whitespace().count();
            let punct_count = text.chars().filter(|c| c.is_ascii_punctuation()).count();
            words + (punct_count / 4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_always_ends_with_a_slash() {
        assert_eq!(
            normalize_base_url("http://localhost:11434"),
            "http://localhost:11434/"
        );
        assert_eq!(
            normalize_base_url("http://localhost:11434/"),
            "http://localhost:11434/"
        );
        assert_eq!(
            normalize_base_url("http://box:11434///"),
            "http://box:11434/"
        );
        assert_eq!(
            normalize_base_url("http://localhost:11434/ollama"),
            "http://localhost:11434/ollama/"
        );
    }
}
