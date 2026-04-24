//! ZAI (ZhipuAI) GLM provider
//!
//! Supports native function calling via genai's ZAI adapter. The adapter
//! auto-detects models starting with `glm` and routes to `api.z.ai`.
//! Tool calling works the same as OpenAI's function calling protocol.
//!
//! Endpoints:
//! - Default: `https://api.z.ai/api/paas/v4/` (regular GLM models)
//! - Coding: `https://api.z.ai/api/coding/paas/v4/` (prefixed `zai::glm-*`)
//!
//! Models: glm-4.5, glm-4.5-air, glm-4.6, glm-4.7, glm-5, glm-5-turbo, glm-5.1

use async_trait::async_trait;
use futures::StreamExt;
use genai::chat::{ChatMessage, ChatRequest};
use tokio::sync::mpsc;

use crate::llm::providers::{ChatWithToolsResult, LlmClient};
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct ZaiProvider {
    client: genai::Client,
    model: String,
}

impl ZaiProvider {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self> {
        let key = api_key.to_string();
        let client = genai::Client::builder()
            .with_auth_resolver_fn(move |_model_iden| {
                Ok(Some(genai::resolver::AuthData::from_single(key.clone())))
            })
            .build();
        Ok(Self {
            client,
            model: model.unwrap_or("glm-4.6").to_string(),
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
impl LlmClient for ZaiProvider {
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
                                tracing::error!("Zai stream error for model {}: {}", model, e);
                                break;
                            },
                        }
                    }
                    if had_error {
                        drop(tx);
                    }
                },
                Err(e) => {
                    tracing::error!("Zai stream init error for model {}: {}", model, e);
                    drop(tx);
                },
            }
        });

        Ok(rx)
    }

    /// Send a chat message with function definitions and get structured tool calls back.
    ///
    /// Uses genai's native ZAI adapter which supports OpenAI-compatible function calling.
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

        let text = response
            .content
            .first_text()
            .unwrap_or("")
            .to_string();

        let tool_calls: Vec<genai::chat::ToolCall> = response
            .content
            .tool_calls()
            .into_iter()
            .cloned()
            .collect();

        Ok(ChatWithToolsResult { text, tool_calls })
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}
