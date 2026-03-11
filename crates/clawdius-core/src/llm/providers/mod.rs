//! LLM providers

pub mod anthropic;
pub mod local;
pub mod ollama;
pub mod openai;
pub mod zai;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<crate::llm::ChatMessage>) -> crate::Result<String>;

    async fn chat_stream(
        &self,
        messages: Vec<crate::llm::ChatMessage>,
    ) -> crate::Result<mpsc::Receiver<String>>;

    fn count_tokens(&self, text: &str) -> usize;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    DeepSeek,
    Ollama,
    Local,
    Zai,
    OpenRouter,
}
