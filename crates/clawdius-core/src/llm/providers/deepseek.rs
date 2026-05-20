//! DeepSeek provider
//!
//! Uses genai's native DeepSeek adapter which auto-detects models starting with
//! `deepseek/` and routes to `https://api.deepseek.com/v1/`.
//!
//! Supports `deepseek-chat`, `deepseek-reasoner`, and any DeepSeek model accessible
//! through their OpenAI-compatible API.
//!
//! Environment: `DEEPSEEK_API_KEY`

use async_trait::async_trait;
use futures::StreamExt;
use genai::chat::{ChatMessage, ChatRequest};
use tokio::sync::mpsc;

use crate::llm::providers::{ChatWithToolsResult, LlmClient};
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct DeepSeekProvider {
    client: genai::Client,
    model: String,
}

impl DeepSeekProvider {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self> {
        let key = api_key.to_string();
        let client = genai::Client::builder()
            .with_auth_resolver_fn(move |_model_iden| {
                Ok(Some(genai::resolver::AuthData::from_single(key.clone())))
            })
            .build();
        Ok(Self {
            client,
            model: model.unwrap_or("deepseek-chat").to_string(),
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
impl LlmClient for DeepSeekProvider {
    async fn chat(&self, messages: Vec<ClawdiusMessage>) -> Result<String> {
        let genai_messages = to_genai_messages(&messages);
        let chat_req = ChatRequest::new(genai_messages);

        let response = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| Error::Llm(e.to_string()))?;

        response
            .content
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
                                tracing::error!("DeepSeek stream error for model {}: {}", model, e);
                                break;
                            },
                        }
                    }
                },
                Err(e) => {
                    tracing::error!("DeepSeek stream init error for model {}: {}", model, e);
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
