//! Local/self-hosted LLM provider (Ollama-compatible API)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use crate::{Error, Result};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

pub struct LocalLlmProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl LocalLlmProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn llama(host: &str) -> Self {
        Self::new(format!("http://{}:11434", host), "llama3.2".to_string())
    }

    pub fn mistral(host: &str) -> Self {
        Self::new(format!("http://{}:11434", host), "mistral".to_string())
    }
}

#[async_trait]
impl LlmClient for LocalLlmProvider {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    ChatRole::System => "system".to_string(),
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "assistant".to_string(),
                },
                content: m.content,
            })
            .collect();

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "Request failed with status {}: {}",
                status, body
            )));
        }

        let json: OllamaResponse = response
            .json()
            .await
            .map_err(|e| Error::Llm(format!("Failed to parse response: {}", e)))?;

        Ok(json.message.content)
    }

    async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
        Err(Error::Llm(
            "Streaming not yet implemented for local provider".into(),
        ))
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}
