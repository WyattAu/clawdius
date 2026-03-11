//! Anthropic Claude provider

use async_trait::async_trait;
use futures::StreamExt;
use genai::chat::{ChatMessage, ChatRequest};
use tokio::sync::mpsc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage as ClawdiusMessage, ChatRole};
use crate::{Error, Result};

pub struct AnthropicProvider {
    client: genai::Client,
    model: String,
}

impl AnthropicProvider {
    pub fn new(_api_key: &str, model: Option<&str>) -> Result<Self> {
        Ok(Self {
            client: genai::Client::default(),
            model: model.unwrap_or("claude-3-5-sonnet-20241022").to_string(),
        })
    }
}

#[async_trait]
impl LlmClient for AnthropicProvider {
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
            .map(|s| s.to_string())
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
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(event) => {
                                if let genai::chat::ChatStreamEvent::Chunk(chunk) = event {
                                    if tx.send(chunk.content).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(format!("[Error: {}]", e)).await;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("[Error: {}]", e)).await;
                }
            }
        });

        Ok(rx)
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}
