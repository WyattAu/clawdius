//! LLM providers

pub mod anthropic;
pub mod google;
pub mod local;
pub mod ollama;
pub mod openai;
pub mod openrouter;

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
    Google,
    OpenAI,
    /// DeepSeek provider (not yet implemented — use OpenAI-compatible API with a
    /// custom base URL pointing to `https://api.deepseek.com`)
    DeepSeek,
    OpenRouter,
    Ollama,
    Local,
}
