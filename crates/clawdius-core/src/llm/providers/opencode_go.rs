//! OpenCode Go provider (OpenAI-compatible endpoint)
//!
//! Routes to the OpenCode Go endpoint at `https://opencode.ai/zen/go/v1/`
//! using genai's OpenAI-compatible adapter with a custom base URL.
//!
//! Available models: minimax-m3, minimax-m2.7, minimax-m2.5, kimi-k2.7-code,
//! kimi-k2.6, kimi-k2.5, glm-5.1, glm-5, deepseek-v4-pro, deepseek-v4-flash,
//! qwen3.7-max, qwen3.7-plus, qwen3.6-plus, qwen3.5-plus, mimo-v2-pro,
//! mimo-v2-omni, mimo-v2.5-pro, mimo-v2.5, hy3-preview
//!
//! Environment: `OPENCODE_GO_API_KEY`

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

const OPENCODE_GO_BASE_URL: &str = "https://opencode.ai/zen/go/v1/";

pub struct OpencodeGoProvider {
    client: genai::Client,
    model: String,
}

impl OpencodeGoProvider {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self> {
        let model_name = model.unwrap_or("mimo-v2.5").to_string();
        let api_key = api_key.to_string();

        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> genai::resolver::Result<ServiceTarget> {
                let ServiceTarget { model, .. } = service_target;
                let endpoint = Endpoint::from_owned(OPENCODE_GO_BASE_URL.to_string());
                let auth = AuthData::from_single(api_key.clone());
                let model = ModelIden::new(AdapterKind::OpenAI, model.model_name);
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
impl LlmClient for OpencodeGoProvider {
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
                                tracing::error!(
                                    "OpenCode Go stream error for model {}: {}",
                                    model,
                                    e
                                );
                                break;
                            },
                        }
                    }
                    if had_error {
                        drop(tx);
                    }
                },
                Err(e) => {
                    tracing::error!("OpenCode Go stream init error for model {}: {}", model, e);
                    drop(tx);
                },
            }
        });

        Ok(rx)
    }

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

        let text = response.content.first_text().unwrap_or("").to_string();

        let tool_calls: Vec<genai::chat::ToolCall> =
            response.content.tool_calls().into_iter().cloned().collect();

        Ok(ChatWithToolsResult { text, tool_calls })
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}
