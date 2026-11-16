//! OpenAI GPT provider
//!
//! Supports native function calling via genai's OpenAI adapter. GPT-4o and later
//! models receive tool definitions as `functions` and return structured
//! `ToolCall` responses with function names and JSON arguments.

use async_trait::async_trait;
use futures::StreamExt;
use genai::chat::{ChatMessage, ChatRequest};
use tokio::sync::mpsc;

use crate::llm::providers::{ChatWithToolsResult, LlmClient};
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct OpenAIProvider {
    client: genai::Client,
    model: String,
    /// Retained for introspection/logging; the resolver pins the
    /// endpoint at request time.
    #[allow(dead_code)]
    base_url: Option<String>,
}

/// Pin a custom endpoint to genai's OpenAI adapter.
///
/// genai infers the adapter from the model name, and a name it does not
/// know — a router alias like `auto`, or any local model — falls back to
/// the Ollama adapter (live: `POST /v1/api/chat` 404 against a
/// FreeLLMAPI router). A custom base URL is OpenAI-compatible by
/// definition, so the adapter is pinned and the model name is passed
/// through untouched.
fn pin_openai_compatible(
    url: String,
) -> impl Fn(genai::ServiceTarget) -> genai::resolver::Result<genai::ServiceTarget>
       + Clone
       + Send
       + Sync
       + 'static {
    move |service_target: genai::ServiceTarget| -> genai::resolver::Result<genai::ServiceTarget> {
        let genai::ServiceTarget { model, auth, .. } = service_target;
        Ok(genai::ServiceTarget {
            endpoint: genai::resolver::Endpoint::from_owned(url.clone()),
            auth,
            model: genai::ModelIden::new(genai::adapter::AdapterKind::OpenAI, model.model_name),
        })
    }
}

impl OpenAIProvider {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self> {
        Self::with_base_url(api_key, model, None)
    }

    /// Create the provider pointing at a custom OpenAI-compatible
    /// endpoint (freellmapi, LocalAI, vLLM, llama.cpp, OpenRouter).
    ///
    /// The resolver pins `ServiceTarget.base_url` to `base_url` — genai's
    /// OpenAI adapter would otherwise always target `api.openai.com`.
    pub fn with_base_url(
        api_key: &str,
        model: Option<&str>,
        base_url: Option<String>,
    ) -> Result<Self> {
        let key = api_key.to_string();
        let model_name = model.unwrap_or("gpt-4o").to_string();

        let client = match base_url.clone() {
            Some(url) => {
                let target_resolver = genai::resolver::ServiceTargetResolver::from_resolver_fn(
                    pin_openai_compatible(url),
                );
                genai::Client::builder()
                    .with_service_target_resolver(target_resolver)
                    .with_auth_resolver_fn(move |_model_iden| {
                        Ok(Some(genai::resolver::AuthData::from_single(key.clone())))
                    })
                    .build()
            },
            None => genai::Client::builder()
                .with_auth_resolver_fn(move |_model_iden| {
                    Ok(Some(genai::resolver::AuthData::from_single(key.clone())))
                })
                .build(),
        };

        Ok(Self {
            client,
            model: model_name,
            base_url,
        })
    }
}

fn to_genai_messages(messages: &[ClawdiusMessage]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role {
            ChatRole::System => ChatMessage::system(m.content.clone()),
            ChatRole::User => ChatMessage::user(m.content.clone()),
            ChatRole::Assistant => ChatMessage::assistant(m.content.clone()),
        })
        .collect()
}

#[async_trait]
impl LlmClient for OpenAIProvider {
    async fn chat(&self, messages: Vec<ClawdiusMessage>) -> Result<String> {
        let genai_messages = to_genai_messages(&messages);
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
    async fn chat_with_options(
        &self,
        messages: Vec<crate::llm::ChatMessage>,
        options: crate::llm::LlmChatOptions,
    ) -> crate::Result<String> {
        let genai_messages = to_genai_messages(&messages);
        let chat_req = ChatRequest::new(genai_messages);
        let genai_opts = options.to_genai_options();
        let response = self
            .client
            .exec_chat(&self.model, chat_req, Some(&genai_opts))
            .await
            .map_err(|e| crate::Error::Llm(e.to_string()))?;
        response
            .first_text()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| crate::Error::Llm("No response text".into()))
    }

    async fn chat_stream(&self, messages: Vec<ClawdiusMessage>) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        let genai_messages = to_genai_messages(&messages);
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
                                tracing::error!("Openai stream error for model {}: {}", model, e);
                                break;
                            },
                        }
                    }
                    if had_error {
                        drop(tx);
                    }
                },
                Err(e) => {
                    tracing::error!("Openai stream init error for model {}: {}", model, e);
                    drop(tx);
                },
            }
        });

        Ok(rx)
    }

    /// Send a chat message with function definitions and get structured tool calls back.
    ///
    /// Uses genai's native OpenAI function calling support. GPT-4o and later
    /// models receive tools as `functions` and return `ToolCall` responses
    /// with function names and JSON arguments.
    async fn chat_with_tools(
        &self,
        messages: Vec<ClawdiusMessage>,
        tools: Vec<genai::chat::Tool>,
    ) -> Result<ChatWithToolsResult> {
        let genai_messages = to_genai_messages(&messages);
        let chat_req = ChatRequest::new(genai_messages).with_tools(tools);

        let response = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        // Extract text and tool calls from the response MessageContent.
        // `response.content` is `MessageContent` (transparent wrapper over Vec<ContentPart>).
        let text = response.content.first_text().unwrap_or("").to_string();

        // `MessageContent::tool_calls()` returns `Vec<&ToolCall>`.
        let tool_calls: Vec<genai::chat::ToolCall> =
            response.content.tool_calls().into_iter().cloned().collect();

        Ok(ChatWithToolsResult { text, tool_calls })
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genai::adapter::AdapterKind;

    fn target(adapter: AdapterKind, model: &str) -> genai::ServiceTarget {
        genai::ServiceTarget {
            endpoint: genai::resolver::Endpoint::from_owned(
                "https://api.openai.com/v1/".to_string(),
            ),
            auth: genai::resolver::AuthData::from_single("k".to_string()),
            model: genai::ModelIden::new(adapter, model),
        }
    }

    #[test]
    fn custom_endpoints_always_use_the_openai_adapter() {
        // Live: model `auto` through a router alias resolved to the
        // Ollama adapter and posted to /v1/api/chat (404).
        let resolve = pin_openai_compatible("http://localhost:3001/v1/".to_string());
        for (incoming, name) in [
            (AdapterKind::Ollama, "auto"),
            (AdapterKind::Ollama, "glm-4.5-flash"),
            (AdapterKind::Gemini, "gemini-3.6-flash"),
        ] {
            let out = resolve(target(incoming, name)).expect("resolver");
            assert_eq!(out.model.adapter_kind, AdapterKind::OpenAI);
            assert_eq!(out.model.model_name.as_str(), name);
            assert_eq!(out.endpoint.base_url(), "http://localhost:3001/v1/");
        }
    }

    #[test]
    fn base_url_without_a_trailing_slash_is_passed_through_verbatim() {
        // The adapter appends `chat/completions`, so the caller owns the
        // `/v1/` suffix; this test pins that we do not rewrite it.
        let resolve = pin_openai_compatible("http://localhost:3001/v1".to_string());
        let out = resolve(target(AdapterKind::Ollama, "auto")).expect("resolver");
        assert_eq!(out.endpoint.base_url(), "http://localhost:3001/v1");
    }
}
