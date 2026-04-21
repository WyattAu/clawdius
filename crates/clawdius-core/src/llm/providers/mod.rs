//! LLM providers
//!
//! Each provider wraps the `genai` crate's multi-provider LLM client.
//! Tool calling (function calling) is supported natively via `genai::chat::Tool`
//! for Anthropic, OpenAI, and any provider that supports the OpenAI-compatible
//! tools format.

pub mod anthropic;
pub mod google;
pub mod local;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod zai;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Re-export genai tool types so callers don't need to depend on genai directly.
pub use genai::chat::Tool;
pub use genai::chat::ToolCall;
pub use genai::chat::ToolResponse;

/// Result of a chat-with-tools call.
///
/// Contains the assistant's text response (if any) and any tool calls
/// the model wants to make.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatWithToolsResult {
    /// Text content from the assistant (may be empty if only tool calls were made).
    pub text: String,
    /// Tool calls requested by the assistant.
    pub tool_calls: Vec<ToolCall>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a chat message and get a text response.
    async fn chat(&self, messages: Vec<crate::llm::ChatMessage>) -> crate::Result<String>;

    /// Send a chat message with streaming response.
    async fn chat_stream(
        &self,
        messages: Vec<crate::llm::ChatMessage>,
    ) -> crate::Result<mpsc::Receiver<String>>;

    /// Send a chat message with tool definitions and get structured tool calls back.
    ///
    /// This uses the provider's native function calling / tool_use API:
    /// - Anthropic: tool_use with `anthropic-beta` header
    /// - OpenAI: function calling with `tools` parameter
    /// - OpenRouter: proxied from the underlying provider
    ///
    /// Returns both the text response and any tool calls the model wants to make.
    /// The caller is responsible for executing tool calls and sending results back
    /// via a follow-up `chat` call with tool responses appended.
    ///
    /// Default implementation returns an error — providers that don't support
    /// tool calling should override this.
    async fn chat_with_tools(
        &self,
        messages: Vec<crate::llm::ChatMessage>,
        tools: Vec<Tool>,
    ) -> crate::Result<ChatWithToolsResult> {
        // Default: not supported. Provider-specific implementations override this.
        Err(crate::Error::Llm(
            "Tool calling not supported by this provider".to_string(),
        ))
    }

    /// Estimate token count for a text string.
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
    /// ZAI (ZhipuAI) GLM provider — supports glm-4.5, glm-4.6, glm-4.7, glm-5
    Zai,
    Ollama,
    Local,
}
