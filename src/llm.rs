//! LLM Integration Types
//!
//! Defines types for LLM provider abstraction per BP-BRAIN-001.
//! Uses genai for provider-agnostic LLM access.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Anthropic Claude
    Anthropic,
    /// `OpenAI` GPT
    OpenAI,
    /// `DeepSeek`
    DeepSeek,
    /// Ollama local
    Ollama,
    /// Z.AI (any models available)
    ZAi,
    /// OpenRouter (FREE models only)
    OpenRouter,
}

impl Provider {
    /// Returns the provider name as a string
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::DeepSeek => "deepseek",
            Self::Ollama => "ollama",
            Self::ZAi => "zai",
            Self::OpenRouter => "openrouter",
        }
    }

    /// Returns the default model for this provider
    #[must_use]
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Anthropic => "claude-3-opus",
            Self::OpenAI => "gpt-4",
            Self::DeepSeek => "deepseek-coder",
            Self::Ollama => "codellama",
            Self::ZAi => "glm-4.5",
            Self::OpenRouter => "liquid/lfm-2.5-1.2b-instruct:free",
        }
    }

    /// Returns the maximum tokens for this provider
    #[must_use]
    pub fn max_tokens(&self) -> u64 {
        match self {
            Self::Anthropic => 100_000,
            Self::OpenAI => 8_192,
            Self::DeepSeek => 16_384,
            Self::Ollama => 4_096,
            Self::ZAi => 32_768,
            Self::OpenRouter => 8_192,
        }
    }

    /// Returns the default embedding model for this provider
    #[must_use]
    pub fn default_embedding_model(&self) -> &'static str {
        match self {
            Self::OpenAI => "text-embedding-3-small",
            Self::ZAi => "text-embedding-3-small",
            Self::OpenRouter => "text-embedding-3-small",
            Self::DeepSeek => "text-embedding-3-small",
            Self::Ollama => "nomic-embed-text",
            Self::Anthropic => "text-embedding-3-small",
        }
    }

    /// Returns the environment variable name for this provider's API key
    #[must_use]
    pub const fn api_key_env(&self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::DeepSeek => "DEEPSEEK_API_KEY",
            Self::Ollama => "",
            Self::ZAi => "ZAI_API_KEY",
            Self::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    /// Loads the API key from the environment
    #[must_use]
    pub fn load_api_key(&self) -> Option<String> {
        std::env::var(self.api_key_env()).ok()
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAI),
            "deepseek" => Ok(Self::DeepSeek),
            "ollama" => Ok(Self::Ollama),
            "zai" | "z.ai" => Ok(Self::ZAi),
            "openrouter" => Ok(Self::OpenRouter),
            _ => Err(format!("Unknown provider: {s}")),
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System instruction
    System,
    /// User message
    User,
    /// Assistant response
    Assistant,
}

impl MessageRole {
    /// Returns the role as a string
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content
    pub content: String,
}

impl Message {
    /// Creates a new message
    #[must_use]
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Creates a system message
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// Creates a user message
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Creates an assistant message
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }
}

/// Chat completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// LLM provider to use
    pub provider: Provider,
    /// Model to use (optional, uses default if not specified)
    pub model: Option<String>,
    /// Conversation messages
    pub messages: Vec<Message>,
    /// Maximum tokens in response
    pub max_tokens: Option<u64>,
    /// Temperature for sampling
    pub temperature: Option<f32>,
    /// Top-p sampling parameter
    pub top_p: Option<f32>,
    /// Whether to stream the response
    pub stream: bool,
}

impl ChatRequest {
    /// Creates a new chat request
    #[must_use]
    pub fn new(provider: Provider, messages: Vec<Message>) -> Self {
        Self {
            provider,
            model: None,
            messages,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: false,
        }
    }

    /// Sets the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the maximum tokens
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets the temperature
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets whether to stream
    #[must_use]
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Returns the effective model name
    #[must_use]
    pub fn effective_model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or_else(|| self.provider.default_model())
    }
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self::new(Provider::OpenAI, Vec::new())
    }
}

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID
    pub id: String,
    /// Provider used
    pub provider: Provider,
    /// Model used
    pub model: String,
    /// Generated message
    pub message: Message,
    /// Token usage
    pub usage: Usage,
    /// Reason for completion
    pub finish_reason: FinishReason,
}

/// Reason for chat completion finishing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop
    Stop,
    /// Max tokens reached
    Length,
    /// Content filter triggered
    ContentFilter,
    /// Error occurred
    Error,
}

impl FinishReason {
    /// Returns true if stopped naturally
    #[must_use]
    pub fn is_stop(&self) -> bool {
        matches!(self, Self::Stop)
    }

    /// Returns true if an error occurred
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }
}

/// Token usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in prompt
    pub prompt_tokens: u64,
    /// Tokens in completion
    pub completion_tokens: u64,
    /// Total tokens
    pub total_tokens: u64,
}

impl Usage {
    /// Creates new usage information
    #[must_use]
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// Text embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    /// Provider to use
    pub provider: Provider,
    /// Model to use
    pub model: Option<String>,
    /// Texts to embed
    pub input: Vec<String>,
}

impl EmbedRequest {
    /// Creates a new embedding request
    #[must_use]
    pub fn new(provider: Provider, input: Vec<String>) -> Self {
        Self {
            provider,
            model: None,
            input,
        }
    }

    /// Sets the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Returns the effective model name
    #[must_use]
    pub fn effective_model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or_else(|| self.provider.default_embedding_model())
    }
}

/// Text embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    /// Generated embeddings
    pub embeddings: Vec<Embedding>,
    /// Token usage
    pub usage: Usage,
}

/// A single embedding vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Index in the input array
    pub index: usize,
    /// Embedding vector
    pub vector: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    id: String,
    model: String,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct OpenAiError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiError,
}

#[derive(Debug, Serialize)]
struct OpenAiEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbedData>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbedData {
    index: usize,
    embedding: Vec<f32>,
}

/// LLM client for making API calls
#[derive(Debug, Clone)]
pub struct LlmClient {
    http_client: reqwest::blocking::Client,
}

impl LlmClient {
    /// Creates a new LLM client
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: reqwest::blocking::Client::new(),
        }
    }

    fn get_endpoint(&self, provider: Provider) -> &'static str {
        match provider {
            Provider::ZAi => "https://api.z.ai/api/coding/paas/v4/chat/completions",
            Provider::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            Provider::OpenAI => "https://api.openai.com/v1/chat/completions",
            Provider::Anthropic => "https://api.anthropic.com/v1/messages",
            Provider::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
            Provider::Ollama => "http://localhost:11434/v1/chat/completions",
        }
    }

    fn get_embedding_endpoint(&self, provider: Provider) -> &'static str {
        match provider {
            Provider::ZAi => "https://api.z.ai/api/coding/paas/v4/embeddings",
            Provider::OpenRouter => "https://openrouter.ai/api/v1/embeddings",
            Provider::OpenAI => "https://api.openai.com/v1/embeddings",
            Provider::DeepSeek => "https://api.deepseek.com/v1/embeddings",
            Provider::Ollama => "http://localhost:11434/v1/embeddings",
            Provider::Anthropic => "https://api.openai.com/v1/embeddings",
        }
    }

    /// Performs a chat completion request
    ///
    /// # Errors
    /// Returns an error if the LLM call fails or the client is not configured.
    pub fn chat(&self, request: ChatRequest) -> crate::error::Result<ChatResponse> {
        let api_key = request.provider.load_api_key().ok_or_else(|| {
            crate::error::BrainError::LlmCallFailed {
                reason: format!(
                    "API key not found for provider {}. Set {} environment variable.",
                    request.provider,
                    request.provider.api_key_env()
                ),
            }
        })?;

        let endpoint = self.get_endpoint(request.provider);
        let model = request.effective_model().to_string();

        let openai_request = OpenAiChatRequest {
            model: model.clone(),
            messages: request
                .messages
                .iter()
                .map(|m| OpenAiMessage {
                    role: m.role.as_str().to_string(),
                    content: m.content.clone(),
                })
                .collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stream: false,
        };

        let mut http_request = self
            .http_client
            .post(endpoint)
            .bearer_auth(&api_key)
            .json(&openai_request);

        if request.provider == Provider::OpenRouter {
            http_request = http_request.header("HTTP-Referer", "https://clawdius.dev");
        }

        let response =
            http_request
                .send()
                .map_err(|e| crate::error::BrainError::LlmCallFailed {
                    reason: format!("HTTP request failed: {e}"),
                })?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to read response body: {e}"),
            })?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<OpenAiErrorResponse>(&body) {
                return Err(crate::error::BrainError::LlmCallFailed {
                    reason: error_response.error.message,
                }
                .into());
            }
            return Err(crate::error::BrainError::LlmCallFailed {
                reason: format!("API error ({}): {}", status, body),
            }
            .into());
        }

        let openai_response: OpenAiChatResponse =
            serde_json::from_str(&body).map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to parse response: {e}"),
            })?;

        let choice = openai_response.choices.into_iter().next().ok_or_else(|| {
            crate::error::BrainError::LlmCallFailed {
                reason: "No choices in response".into(),
            }
        })?;

        let finish_reason = match choice.finish_reason.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        };

        let usage = openai_response
            .usage
            .map(|u| Usage::new(u.prompt_tokens, u.completion_tokens))
            .unwrap_or_default();

        Ok(ChatResponse {
            id: openai_response.id,
            provider: request.provider,
            model: openai_response.model,
            message: Message::new(
                match choice.message.role.as_str() {
                    "user" => MessageRole::User,
                    "system" => MessageRole::System,
                    _ => MessageRole::Assistant,
                },
                choice.message.content,
            ),
            usage,
            finish_reason,
        })
    }

    /// Performs an embedding request
    ///
    /// # Errors
    /// Returns an error if the embedding call fails or the client is not configured.
    pub fn embed(&self, request: EmbedRequest) -> crate::error::Result<EmbedResponse> {
        if request.provider == Provider::Ollama {
            return self.embed_ollama(request);
        }

        let api_key = request.provider.load_api_key().ok_or_else(|| {
            crate::error::BrainError::LlmCallFailed {
                reason: format!(
                    "API key not found for provider {}. Set {} environment variable.",
                    request.provider,
                    request.provider.api_key_env()
                ),
            }
        })?;

        let endpoint = self.get_embedding_endpoint(request.provider);
        let model = request.effective_model().to_string();

        let openai_request = OpenAiEmbedRequest {
            model: model.clone(),
            input: request.input.clone(),
        };

        let mut http_request = self
            .http_client
            .post(endpoint)
            .bearer_auth(&api_key)
            .json(&openai_request);

        if request.provider == Provider::OpenRouter {
            http_request = http_request.header("HTTP-Referer", "https://clawdius.dev");
        }

        let response =
            http_request
                .send()
                .map_err(|e| crate::error::BrainError::LlmCallFailed {
                    reason: format!("HTTP request failed: {e}"),
                })?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to read response body: {e}"),
            })?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<OpenAiErrorResponse>(&body) {
                return Err(crate::error::BrainError::LlmCallFailed {
                    reason: error_response.error.message,
                }
                .into());
            }
            return Err(crate::error::BrainError::LlmCallFailed {
                reason: format!("API error ({}): {}", status, body),
            }
            .into());
        }

        let openai_response: OpenAiEmbedResponse =
            serde_json::from_str(&body).map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to parse response: {e}"),
            })?;

        let embeddings = openai_response
            .data
            .into_iter()
            .map(|d| Embedding {
                index: d.index,
                vector: d.embedding,
            })
            .collect();

        Ok(EmbedResponse {
            embeddings,
            usage: Usage::new(
                openai_response.usage.prompt_tokens,
                openai_response.usage.completion_tokens,
            ),
        })
    }

    fn embed_ollama(&self, request: EmbedRequest) -> crate::error::Result<EmbedResponse> {
        let endpoint = self.get_embedding_endpoint(request.provider);
        let model = request.effective_model().to_string();

        let openai_request = OpenAiEmbedRequest {
            model: model.clone(),
            input: request.input.clone(),
        };

        let response = self
            .http_client
            .post(endpoint)
            .json(&openai_request)
            .send()
            .map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("HTTP request failed: {e}"),
            })?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to read response body: {e}"),
            })?;

        if !status.is_success() {
            return Err(crate::error::BrainError::LlmCallFailed {
                reason: format!("API error ({}): {}", status, body),
            }
            .into());
        }

        let openai_response: OpenAiEmbedResponse =
            serde_json::from_str(&body).map_err(|e| crate::error::BrainError::LlmCallFailed {
                reason: format!("Failed to parse response: {e}"),
            })?;

        let embeddings = openai_response
            .data
            .into_iter()
            .map(|d| Embedding {
                index: d.index,
                vector: d.embedding,
            })
            .collect();

        Ok(EmbedResponse {
            embeddings,
            usage: Usage::new(
                openai_response.usage.prompt_tokens,
                openai_response.usage.completion_tokens,
            ),
        })
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Model to use
    pub model: String,
    /// Maximum tokens
    pub max_tokens: u64,
    /// Base URL for API (optional)
    pub base_url: Option<String>,
}

impl ProviderConfig {
    /// Creates a new provider configuration
    #[must_use]
    pub fn new(model: impl Into<String>, max_tokens: u64) -> Self {
        Self {
            enabled: true,
            model: model.into(),
            max_tokens,
            base_url: None,
        }
    }

    /// Sets a custom base URL
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Creates a disabled provider configuration
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            model: String::new(),
            max_tokens: 0,
            base_url: None,
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self::new("gpt-4", 8192)
    }
}

/// LLM configuration for all providers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmConfig {
    /// `OpenAI` configuration
    pub openai: Option<ProviderConfig>,
    /// Anthropic configuration
    pub anthropic: Option<ProviderConfig>,
    /// `DeepSeek` configuration
    pub deepseek: Option<ProviderConfig>,
    /// Ollama configuration
    pub ollama: Option<ProviderConfig>,
    /// Z.AI configuration (can use any models)
    pub zai: Option<ProviderConfig>,
    /// OpenRouter configuration (FREE models only)
    pub openrouter: Option<ProviderConfig>,
}

impl LlmConfig {
    /// Gets the configuration for a provider
    #[must_use]
    pub fn get_provider_config(&self, provider: Provider) -> Option<&ProviderConfig> {
        match provider {
            Provider::OpenAI => self.openai.as_ref(),
            Provider::Anthropic => self.anthropic.as_ref(),
            Provider::DeepSeek => self.deepseek.as_ref(),
            Provider::Ollama => self.ollama.as_ref(),
            Provider::ZAi => self.zai.as_ref(),
            Provider::OpenRouter => self.openrouter.as_ref(),
        }
    }

    /// Checks if a provider is enabled
    #[must_use]
    pub fn is_provider_enabled(&self, provider: Provider) -> bool {
        self.get_provider_config(provider)
            .is_some_and(|c| c.enabled)
    }

    /// Loads API keys from environment variables for all providers
    pub fn load_env_keys(&mut self) {
        if Provider::ZAi.load_api_key().is_some() && self.zai.is_none() {
            self.zai = Some(ProviderConfig::new(
                Provider::ZAi.default_model(),
                Provider::ZAi.max_tokens(),
            ));
        }
        if Provider::OpenRouter.load_api_key().is_some() && self.openrouter.is_none() {
            self.openrouter = Some(ProviderConfig::new(
                Provider::OpenRouter.default_model(),
                Provider::OpenRouter.max_tokens(),
            ));
        }
        if Provider::OpenAI.load_api_key().is_some() && self.openai.is_none() {
            self.openai = Some(ProviderConfig::new(
                Provider::OpenAI.default_model(),
                Provider::OpenAI.max_tokens(),
            ));
        }
        if Provider::Anthropic.load_api_key().is_some() && self.anthropic.is_none() {
            self.anthropic = Some(ProviderConfig::new(
                Provider::Anthropic.default_model(),
                Provider::Anthropic.max_tokens(),
            ));
        }
        if Provider::DeepSeek.load_api_key().is_some() && self.deepseek.is_none() {
            self.deepseek = Some(ProviderConfig::new(
                Provider::DeepSeek.default_model(),
                Provider::DeepSeek.max_tokens(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_default_model() {
        assert_eq!(Provider::OpenAI.default_model(), "gpt-4");
        assert_eq!(Provider::Anthropic.default_model(), "claude-3-opus");
        assert_eq!(Provider::DeepSeek.default_model(), "deepseek-coder");
        assert_eq!(Provider::Ollama.default_model(), "codellama");
        assert_eq!(Provider::ZAi.default_model(), "glm-4.5");
        assert_eq!(
            Provider::OpenRouter.default_model(),
            "liquid/lfm-2.5-1.2b-instruct:free"
        );
    }

    #[test]
    fn test_provider_max_tokens() {
        assert_eq!(Provider::Anthropic.max_tokens(), 100_000);
        assert_eq!(Provider::OpenAI.max_tokens(), 8_192);
        assert_eq!(Provider::ZAi.max_tokens(), 32_768);
        assert_eq!(Provider::OpenRouter.max_tokens(), 8_192);
    }

    #[test]
    fn test_provider_from_str() {
        assert!(Provider::from_str("openai").is_ok());
        assert!(Provider::from_str("ANTHROPIC").is_ok());
        assert!(Provider::from_str("unknown").is_err());
        assert!(Provider::from_str("zai").is_ok());
        assert!(Provider::from_str("z.ai").is_ok());
        assert!(Provider::from_str("openrouter").is_ok());
    }

    #[test]
    fn test_message_creation() {
        let system = Message::system("You are helpful");
        assert_eq!(system.role, MessageRole::System);

        let user = Message::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let assistant = Message::assistant("Hi there");
        assert_eq!(assistant.role, MessageRole::Assistant);
    }

    #[test]
    fn test_chat_request_builder() {
        let request = ChatRequest::new(Provider::OpenAI, vec![Message::user("test")])
            .with_model("gpt-4-turbo")
            .with_max_tokens(4096)
            .with_temperature(0.7);

        assert_eq!(request.effective_model(), "gpt-4-turbo");
        assert_eq!(request.max_tokens, Some(4096));
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_chat_request_default_model() {
        let request = ChatRequest::new(Provider::Anthropic, vec![]);
        assert_eq!(request.effective_model(), "claude-3-opus");
    }

    #[test]
    fn test_usage() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_finish_reason() {
        assert!(FinishReason::Stop.is_stop());
        assert!(!FinishReason::Length.is_stop());
        assert!(FinishReason::Error.is_error());
    }

    #[test]
    fn test_provider_config() {
        let config = ProviderConfig::new("gpt-4", 8192).with_base_url("http://localhost:8080");

        assert!(config.enabled);
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.max_tokens, 8192);
        assert_eq!(config.base_url, Some("http://localhost:8080".into()));
    }

    #[test]
    fn test_llm_config() {
        let config = LlmConfig {
            openai: Some(ProviderConfig::new("gpt-4", 8192)),
            anthropic: None,
            deepseek: Some(ProviderConfig::disabled()),
            ollama: None,
            zai: None,
            openrouter: None,
        };

        assert!(config.is_provider_enabled(Provider::OpenAI));
        assert!(!config.is_provider_enabled(Provider::Anthropic));
        assert!(!config.is_provider_enabled(Provider::DeepSeek));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Hello, world!");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.role, deserialized.role);
        assert_eq!(msg.content, deserialized.content);
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest::new(
            Provider::Anthropic,
            vec![Message::system("Be helpful"), Message::user("Hello")],
        )
        .with_temperature(0.5);

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ChatRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.provider, deserialized.provider);
        assert_eq!(request.messages.len(), deserialized.messages.len());
    }

    #[test]
    fn test_provider_default_embedding_model() {
        assert_eq!(
            Provider::OpenAI.default_embedding_model(),
            "text-embedding-3-small"
        );
        assert_eq!(
            Provider::ZAi.default_embedding_model(),
            "text-embedding-3-small"
        );
        assert_eq!(
            Provider::OpenRouter.default_embedding_model(),
            "text-embedding-3-small"
        );
        assert_eq!(
            Provider::DeepSeek.default_embedding_model(),
            "text-embedding-3-small"
        );
        assert_eq!(
            Provider::Ollama.default_embedding_model(),
            "nomic-embed-text"
        );
        assert_eq!(
            Provider::Anthropic.default_embedding_model(),
            "text-embedding-3-small"
        );
    }

    #[test]
    fn test_embed_request_builder() {
        let request = EmbedRequest::new(Provider::OpenAI, vec!["hello".into(), "world".into()])
            .with_model("text-embedding-3-large");

        assert_eq!(request.effective_model(), "text-embedding-3-large");
        assert_eq!(request.input.len(), 2);
    }

    #[test]
    fn test_embed_request_default_model() {
        let request = EmbedRequest::new(Provider::Ollama, vec!["test".into()]);
        assert_eq!(request.effective_model(), "nomic-embed-text");
    }

    #[test]
    fn test_embed_request_serialization() {
        let request = EmbedRequest::new(Provider::OpenAI, vec!["hello".into(), "world".into()])
            .with_model("text-embedding-3-small");

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: EmbedRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.provider, deserialized.provider);
        assert_eq!(request.input.len(), deserialized.input.len());
        assert_eq!(request.model, deserialized.model);
    }

    #[test]
    fn test_embed_response_serialization() {
        let response = EmbedResponse {
            embeddings: vec![
                Embedding {
                    index: 0,
                    vector: vec![0.1, 0.2, 0.3],
                },
                Embedding {
                    index: 1,
                    vector: vec![0.4, 0.5, 0.6],
                },
            ],
            usage: Usage::new(10, 0),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: EmbedResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.embeddings.len(), deserialized.embeddings.len());
        assert_eq!(
            response.embeddings[0].index,
            deserialized.embeddings[0].index
        );
        assert_eq!(
            response.embeddings[0].vector.len(),
            deserialized.embeddings[0].vector.len()
        );
    }

    #[test]
    fn test_openai_embed_request_serialization() {
        let request = OpenAiEmbedRequest {
            model: "text-embedding-3-small".into(),
            input: vec!["test input".into()],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("text-embedding-3-small"));
        assert!(json.contains("test input"));
    }

    #[test]
    fn test_openai_embed_response_deserialization() {
        let json = r#"{
            "data": [
                {"index": 0, "embedding": [0.1, 0.2, 0.3]},
                {"index": 1, "embedding": [0.4, 0.5, 0.6]}
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 0, "total_tokens": 10}
        }"#;

        let response: OpenAiEmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].index, 0);
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.usage.prompt_tokens, 10);
    }

    #[test]
    fn test_llm_client_embed_missing_api_key() {
        let client = LlmClient::new();
        let request = EmbedRequest::new(Provider::OpenAI, vec!["test".into()]);
        let result = client.embed(request);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("API key not found"));
    }

    #[test]
    fn test_llm_client_embed_ollama_no_api_key_needed() {
        let client = LlmClient::new();
        let request = EmbedRequest::new(Provider::Ollama, vec!["test".into()]);
        let result = client.embed(request);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.contains("API key not found"));
    }

    #[test]
    fn test_get_embedding_endpoint() {
        let client = LlmClient::new();
        assert_eq!(
            client.get_embedding_endpoint(Provider::OpenAI),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            client.get_embedding_endpoint(Provider::ZAi),
            "https://api.z.ai/api/coding/paas/v4/embeddings"
        );
        assert_eq!(
            client.get_embedding_endpoint(Provider::OpenRouter),
            "https://openrouter.ai/api/v1/embeddings"
        );
        assert_eq!(
            client.get_embedding_endpoint(Provider::DeepSeek),
            "https://api.deepseek.com/v1/embeddings"
        );
        assert_eq!(
            client.get_embedding_endpoint(Provider::Ollama),
            "http://localhost:11434/v1/embeddings"
        );
    }
}
