//! LLM integration with multi-provider support.
//!
//! This module provides a unified interface for interacting with various LLM providers,
//! including Anthropic (Claude), `OpenAI` (GPT), OpenRouter, Ollama (local models), and Z.AI.
//!
//! # Features
//!
//! - **Multi-provider support**: Seamlessly switch between different LLM providers
//! - **Automatic retry**: Exponential backoff with configurable retry conditions
//! - **Rate limit handling**: Automatic detection and handling of rate limits
//! - **Token counting**: Built-in token counting for context management
//! - **Secure API key storage**: Integration with system keyring (feature-gated)
//!
//! # Providers
//!
//! ## Anthropic (Claude)
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
//!
//! # #[tokio::main]
//! # async fn main() -> clawdius_core::Result<()> {
//! // Requires ANTHROPIC_API_KEY environment variable
//! let config = LlmConfig::from_env("anthropic")?;
//! let provider = create_provider(&config)?;
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Explain Rust ownership".to_string(),
//! }];
//!
//! let response = provider.chat(messages).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## `OpenAI` (GPT)
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
//!
//! # #[tokio::main]
//! # async fn main() -> clawdius_core::Result<()> {
//! // Requires OPENAI_API_KEY environment variable
//! let config = LlmConfig::from_env("openai")?;
//! let provider = create_provider(&config)?;
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Write a hello world in Rust".to_string(),
//! }];
//!
//! let response = provider.chat(messages).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## OpenRouter
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
//!
//! # #[tokio::main]
//! # async fn main() -> clawdius_core::Result<()> {
//! // Requires OPENROUTER_API_KEY environment variable
//! let config = LlmConfig::from_env("openrouter")?;
//! let provider = create_provider(&config)?;
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Hello via OpenRouter!".to_string(),
//! }];
//!
//! let response = provider.chat(messages).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Ollama (Local Models)
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
//!
//! # #[tokio::main]
//! # async fn main() -> clawdius_core::Result<()> {
//! // Requires Ollama running locally
//! let config = LlmConfig::from_env("ollama")?;
//! let provider = create_provider(&config)?;
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Hello from local LLM!".to_string(),
//! }];
//!
//! let response = provider.chat(messages).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Retry Configuration
//!
//! The module provides sophisticated retry logic with exponential backoff:
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider_with_retry, ChatMessage, ChatRole};
//! use clawdius_core::config::RetryConfig;
//!
//! # #[tokio::main]
//! # async fn main() -> clawdius_core::Result<()> {
//! let config = LlmConfig::from_env("anthropic")?;
//!
//! // Configure retry behavior
//! let retry_config = RetryConfig {
//!     max_retries: 5,
//!     initial_delay_ms: 1000,
//!     max_delay_ms: 60000,
//!     exponential_base: 2.0,
//!     ..Default::default()
//! };
//!
//! let client = create_provider_with_retry(&config, Some(retry_config))?;
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Hello".to_string(),
//! }];
//!
//! // Automatically retries on rate limits, timeouts, and server errors
//! let response = client.chat(messages).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! The module returns [`Error`] variants for various failure conditions:
//!
//! - [`Error::Config`]: Configuration errors (missing API keys, invalid provider)
//! - [`Error::Llm`]: General LLM errors
//! - [`Error::LlmProvider`]: Provider-specific errors with context
//! - [`Error::RateLimited`]: Rate limit errors with retry-after information
//! - [`Error::Timeout`]: Request timeout errors
//! - [`Error::RetryExhausted`]: All retry attempts failed
//!
//! Errors can be checked for retryability:
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
//! use clawdius_core::Error;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let config = LlmConfig::from_env("anthropic").unwrap();
//! let provider = create_provider(&config).unwrap();
//!
//! let messages = vec![ChatMessage {
//!     role: ChatRole::User,
//!     content: "Hello".to_string(),
//! }];
//!
//! match provider.chat(messages).await {
//!     Ok(response) => println!("Response: {}", response),
//!     Err(Error::RateLimited { retry_after_ms }) => {
//!         println!("Rate limited, retry after {}ms", retry_after_ms);
//!     }
//!     Err(e) if e.is_retryable() => {
//!         println!("Retryable error: {}", e);
//!     }
//!     Err(e) => {
//!         eprintln!("Error: {}", e);
//!     }
//! }
//! # }
//! ```
//!
//! # Token Counting
//!
//! All providers support token counting for context management:
//!
//! ```rust,no_run
//! use clawdius_core::llm::{LlmConfig, create_provider};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let config = LlmConfig::from_env("anthropic")?;
//! let provider = create_provider(&config)?;
//!
//! let text = "Count the tokens in this text";
//! let token_count = provider.count_tokens(text);
//! println!("Token count: {}", token_count);
//! # Ok(())
//! # }
//! ```
//!
//! # Security
//!
//! API keys can be stored securely using the system keyring (requires `keyring` feature):
//!
//! ```rust,ignore
//! use clawdius_core::config::KeyringStorage;
//!
//! let storage = KeyringStorage::global();
//! storage.set_api_key("anthropic", "your-api-key")?;
//!
//! // Later, retrieve it
//! let key = storage.get_api_key("anthropic")?;
//! ```
//!
//! [`Error`]: crate::Error

mod messages;
pub mod providers;
pub mod rate_limiter;

pub use messages::{ChatMessage, ChatRole};
pub use providers::ChatWithToolsResult;
pub use providers::Provider;
pub use rate_limiter::{RateLimiter, RateLimiterConfig};

pub use crate::config::{RetryCondition, RetryConfig};
use crate::{Error, Result};
pub use providers::LlmClient;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

/// Check if an error should be retried based on conditions
fn should_retry(error: &Error, conditions: &[RetryCondition]) -> bool {
    let error_str = error.to_string().to_lowercase();

    for condition in conditions {
        let should_retry = match condition {
            RetryCondition::RateLimit => {
                error_str.contains("429")
                    || error_str.contains("rate limit")
                    || error_str.contains("too many requests")
            },
            RetryCondition::Timeout => {
                matches!(error, Error::Timeout(_))
                    || error_str.contains("timeout")
                    || error_str.contains("timed out")
            },
            RetryCondition::ServerError => {
                error_str.contains("500")
                    || error_str.contains("502")
                    || error_str.contains("503")
                    || error_str.contains("504")
                    || error_str.contains("internal server error")
                    || error_str.contains("bad gateway")
                    || error_str.contains("service unavailable")
                    || error_str.contains("gateway timeout")
            },
            RetryCondition::NetworkError => {
                error_str.contains("network")
                    || error_str.contains("connection")
                    || error_str.contains("dns")
                    || error_str.contains("socket")
                    || error_str.contains("refused")
                    || error_str.contains("reset")
            },
        };

        if should_retry {
            return true;
        }
    }

    false
}

/// Execute an async function with retry logic
pub async fn with_retry<T, F, Fut>(config: &RetryConfig, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut delay = config.initial_delay_ms;
    let mut attempts = 0u32;

    loop {
        attempts += 1;

        match f().await {
            Ok(result) => {
                if attempts > 1 {
                    tracing::info!("LLM call succeeded on attempt {}", attempts);
                }
                return Ok(result);
            },
            Err(e) => {
                let is_auth_error = matches!(e, Error::Auth(_))
                    || e.to_string().to_lowercase().contains("401")
                    || e.to_string().to_lowercase().contains("403")
                    || e.to_string().to_lowercase().contains("unauthorized")
                    || e.to_string().to_lowercase().contains("forbidden");

                if is_auth_error {
                    return Err(e);
                }

                if should_retry(&e, &config.retry_on) && attempts <= config.max_retries {
                    tracing::warn!(
                        "LLM call failed (attempt {}/{}): {}. Retrying in {}ms...",
                        attempts,
                        config.max_retries,
                        e,
                        delay
                    );

                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    delay = (delay as f64 * config.exponential_base).min(config.max_delay_ms as f64)
                        as u64;
                } else if attempts > config.max_retries {
                    return Err(Error::RetryExhausted(config.max_retries));
                } else {
                    return Err(e);
                }
            },
        }
    }
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider
    pub provider: String,
    /// Model
    pub model: String,
    /// API key (if required)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Base URL (for custom endpoints)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Maximum tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
}

fn default_max_tokens() -> usize {
    4096
}

impl LlmConfig {
    pub fn from_env(provider: &str) -> Result<Self> {
        use crate::error::ErrorHelpers;

        let (api_key, base_url, model) = match provider.to_lowercase().as_str() {
            "anthropic" => {
                let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("Anthropic", "ANTHROPIC_API_KEY").to_string(),
                    )
                })?;
                (Some(key), None, "claude-3-5-sonnet-20241022".to_string())
            },
            "google" => {
                let key = std::env::var("GOOGLE_API_KEY").map_err(|_| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("Google", "GOOGLE_API_KEY").to_string(),
                    )
                })?;
                (Some(key), None, "gemini-2.0-flash".to_string())
            },
            "openai" => {
                let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("OpenAI", "OPENAI_API_KEY").to_string(),
                    )
                })?;
                (Some(key), None, "gpt-4o".to_string())
            },
            "openrouter" => {
                let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("OpenRouter", "OPENROUTER_API_KEY")
                            .to_string(),
                    )
                })?;
                let model = std::env::var("OPENROUTER_MODEL")
                    .unwrap_or_else(|_| "openai/gpt-oss-20b:free".to_string());
                (Some(key), None, model)
            },
            "ollama" => {
                let url = std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".into());
                (None, Some(url), "llama3.2".to_string())
            },
            "local" | "llama" => {
                let url = std::env::var("LOCAL_LLM_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".into());
                (None, Some(url), "llama3.2".to_string())
            },
            "mistral" => {
                let url = std::env::var("LOCAL_LLM_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".into());
                (None, Some(url), "mistral".to_string())
            },
            "zai" => {
                let key = std::env::var("ZAI_API_KEY").map_err(|_| {
                    Error::Config(ErrorHelpers::api_key_missing("Z.AI", "ZAI_API_KEY").to_string())
                })?;
                (Some(key), None, "zai-default".to_string())
            },
            _ => {
                return Err(Error::Config(
                    ErrorHelpers::unknown_provider(
                        provider,
                        &[
                            "anthropic",
                            "google",
                            "openai",
                            "openrouter",
                            "ollama",
                            "zai",
                        ],
                    )
                    .to_string(),
                ))
            },
        };

        Ok(Self {
            provider: provider.to_lowercase(),
            model,
            api_key,
            base_url,
            max_tokens: 4096,
        })
    }

    fn load_api_key(
        env_var: &str,
        provider: &str,
        config_key: &Option<String>,
        config_env_var: &Option<String>,
    ) -> Result<Option<String>> {
        if let Ok(key) = std::env::var(env_var) {
            return Ok(Some(key));
        }

        #[cfg(feature = "keyring")]
        {
            if let Ok(Some(key)) = crate::config::KeyringStorage::global().get_api_key(provider) {
                return Ok(Some(key));
            }
        }

        if let Some(key) = config_key {
            tracing::warn!(
                "API key for {} loaded from config file. Consider using environment variable {} or keyring storage instead.",
                provider, env_var
            );
            return Ok(Some(key.clone()));
        }

        if let Some(env_name) = config_env_var {
            if let Ok(key) = std::env::var(env_name) {
                return Ok(Some(key));
            }
        }

        Ok(None)
    }

    pub fn from_config(config: &crate::config::LlmConfig, provider: &str) -> Result<Self> {
        use crate::error::ErrorHelpers;

        let provider_lower = provider.to_lowercase();

        let (model, api_key, base_url) = match provider_lower.as_str() {
            "anthropic" => {
                let cfg = config.anthropic.as_ref();
                let model = cfg
                    .and_then(|c| c.model.clone())
                    .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

                let api_key = Self::load_api_key(
                    "ANTHROPIC_API_KEY",
                    "anthropic",
                    &cfg.and_then(|c| c.api_key.clone()),
                    &cfg.and_then(|c| c.api_key_env.clone()),
                )?;

                let api_key = api_key.ok_or_else(|| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("Anthropic", "ANTHROPIC_API_KEY").to_string(),
                    )
                })?;

                (model, Some(api_key), None)
            },
            "openai" => {
                let cfg = config.openai.as_ref();
                let model = cfg
                    .and_then(|c| c.model.clone())
                    .unwrap_or_else(|| "gpt-4o".to_string());

                let api_key = Self::load_api_key(
                    "OPENAI_API_KEY",
                    "openai",
                    &cfg.and_then(|c| c.api_key.clone()),
                    &cfg.and_then(|c| c.api_key_env.clone()),
                )?;

                let api_key = api_key.ok_or_else(|| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("OpenAI", "OPENAI_API_KEY").to_string(),
                    )
                })?;

                (model, Some(api_key), None)
            },
            "google" => {
                let cfg = config.google.as_ref();
                let model = cfg
                    .and_then(|c| c.model.clone())
                    .unwrap_or_else(|| "gemini-2.0-flash".to_string());

                let api_key = Self::load_api_key(
                    "GOOGLE_API_KEY",
                    "google",
                    &cfg.and_then(|c| c.api_key.clone()),
                    &cfg.and_then(|c| c.api_key_env.clone()),
                )?;

                let api_key = api_key.ok_or_else(|| {
                    Error::Config(
                        ErrorHelpers::api_key_missing("Google", "GOOGLE_API_KEY").to_string(),
                    )
                })?;

                (model, Some(api_key), None)
            },
            "ollama" => {
                let cfg = config.ollama.as_ref();
                let model = cfg
                    .and_then(|c| c.model.clone())
                    .unwrap_or_else(|| "llama3.2".to_string());
                let base_url = std::env::var("OLLAMA_BASE_URL")
                    .ok()
                    .or_else(|| cfg.map(|c| c.base_url.clone()));
                (model, None, base_url)
            },
            "local" | "llama" | "mistral" => {
                let model = if provider_lower == "mistral" {
                    "mistral".to_string()
                } else {
                    "llama3.2".to_string()
                };
                let base_url = std::env::var("LOCAL_LLM_BASE_URL")
                    .ok()
                    .or_else(|| config.ollama.as_ref().map(|c| c.base_url.clone()));
                (model, None, base_url)
            },
            "zai" => {
                let cfg = config.zai.as_ref();
                // Use "zai::" prefix to route to coding endpoint (api.z.ai/api/coding/paas/v4/)
                // Regular endpoint (api.z.ai/api/paas/v4/) may have separate balance
                let model = cfg
                    .and_then(|c| c.model.clone())
                    .map(|m| {
                        if m.starts_with("zai::") { m } else { format!("zai::{m}") }
                    })
                    .unwrap_or_else(|| "zai::glm-4.6".to_string());

                let api_key = Self::load_api_key(
                    "ZAI_API_KEY",
                    "zai",
                    &cfg.and_then(|c| c.api_key.clone()),
                    &cfg.and_then(|c| c.api_key_env.clone()),
                )?;

                let api_key = api_key.ok_or_else(|| {
                    Error::Config(ErrorHelpers::api_key_missing("Z.AI", "ZAI_API_KEY").to_string())
                })?;

                (model, Some(api_key), None)
            },
            "openrouter" => {
                let model = std::env::var("OPENROUTER_MODEL")
                    .unwrap_or_else(|_| "openai/gpt-oss-20b:free".to_string());
                let api_key = match std::env::var("OPENROUTER_API_KEY") {
                    Ok(key) => key,
                    Err(_) => {
                        return Err(Error::Config(
                            ErrorHelpers::api_key_missing("OpenRouter", "OPENROUTER_API_KEY")
                                .to_string(),
                        ))
                    },
                };
                (model, Some(api_key), None)
            },
            _ => {
                return Err(Error::Config(
                    ErrorHelpers::unknown_provider(
                        provider,
                        &[
                            "anthropic",
                            "google",
                            "openai",
                            "ollama",
                            "zai",
                            "openrouter",
                        ],
                    )
                    .to_string(),
                ))
            },
        };

        Ok(Self {
            provider: provider_lower,
            model,
            api_key,
            base_url,
            max_tokens: config.max_tokens,
        })
    }
}

pub enum LlmProvider {
    Anthropic(providers::anthropic::AnthropicProvider),
    Google(providers::google::GoogleProvider),
    OpenAi(providers::openai::OpenAIProvider),
    OpenRouter(providers::openrouter::OpenRouterProvider),
    Zai(providers::zai::ZaiProvider),
    Ollama(providers::ollama::OllamaProvider),
    Local(providers::local::LocalLlmProvider),
}

pub struct LlmClientWithRetry {
    provider: LlmProvider,
    retry_config: RetryConfig,
}

impl LlmClientWithRetry {
    #[must_use]
    pub fn new(provider: LlmProvider, retry_config: RetryConfig) -> Self {
        Self {
            provider,
            retry_config,
        }
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let retry_config = self.retry_config.clone();
        let provider = &self.provider;

        with_retry(&retry_config, || async {
            match provider {
                LlmProvider::Anthropic(p) => p.chat(messages.clone()).await,
                LlmProvider::Google(p) => p.chat(messages.clone()).await,
                LlmProvider::OpenAi(p) => p.chat(messages.clone()).await,
                LlmProvider::OpenRouter(p) => p.chat(messages.clone()).await,
                LlmProvider::Zai(p) => p.chat(messages.clone()).await,
                LlmProvider::Ollama(p) => p.chat(messages.clone()).await,
                LlmProvider::Local(p) => p.chat(messages.clone()).await,
            }
        })
        .await
    }

    #[must_use]
    pub fn count_tokens(&self, text: &str) -> usize {
        match &self.provider {
            LlmProvider::Anthropic(p) => p.count_tokens(text),
            LlmProvider::Google(p) => p.count_tokens(text),
            LlmProvider::OpenAi(p) => p.count_tokens(text),
            LlmProvider::OpenRouter(p) => p.count_tokens(text),
            LlmProvider::Zai(p) => p.count_tokens(text),
            LlmProvider::Ollama(p) => p.count_tokens(text),
            LlmProvider::Local(p) => p.count_tokens(text),
        }
    }

    pub async fn chat_with_tools(
        &self, messages: Vec<ChatMessage>, tools: Vec<genai::chat::Tool>,
    ) -> Result<ChatWithToolsResult> {
        match &self.provider {
            LlmProvider::Anthropic(p) => p.chat_with_tools(messages, tools).await,
            LlmProvider::OpenAi(p) => p.chat_with_tools(messages, tools).await,
            LlmProvider::OpenRouter(p) => p.chat_with_tools(messages, tools).await,
            LlmProvider::Zai(p) => p.chat_with_tools(messages, tools).await,
            LlmProvider::Google(_) | LlmProvider::Ollama(_) | LlmProvider::Local(_) => {
                Err(crate::Error::Llm(
                    "Tool calling not supported by this provider. Use Anthropic, OpenAI, OpenRouter, or ZAI."
                        .to_string(),
                ))
            },
        }
    }

    pub async fn chat_with_retry(
        &self,
        messages: Vec<ChatMessage>,
        retry_config: &RetryConfig,
    ) -> Result<String> {
        let messages_clone = messages.clone();
        with_retry(retry_config, || async {
            self.chat(messages_clone.clone()).await
        })
        .await
    }
}

#[async_trait::async_trait]
impl providers::LlmClient for LlmProvider {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        match self {
            LlmProvider::Anthropic(p) => p.chat(messages).await,
            LlmProvider::Google(p) => p.chat(messages).await,
            LlmProvider::OpenAi(p) => p.chat(messages).await,
            LlmProvider::OpenRouter(p) => p.chat(messages).await,
            LlmProvider::Zai(p) => p.chat(messages).await,
            LlmProvider::Ollama(p) => p.chat(messages).await,
            LlmProvider::Local(p) => p.chat(messages).await,
        }
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        match self {
            LlmProvider::Anthropic(p) => p.chat_stream(messages).await,
            LlmProvider::Google(p) => p.chat_stream(messages).await,
            LlmProvider::OpenAi(p) => p.chat_stream(messages).await,
            LlmProvider::OpenRouter(p) => p.chat_stream(messages).await,
            LlmProvider::Zai(p) => p.chat_stream(messages).await,
            LlmProvider::Ollama(p) => p.chat_stream(messages).await,
            LlmProvider::Local(p) => p.chat_stream(messages).await,
        }
    }

    fn count_tokens(&self, text: &str) -> usize {
        match self {
            LlmProvider::Anthropic(p) => p.count_tokens(text),
            LlmProvider::Google(p) => p.count_tokens(text),
            LlmProvider::OpenAi(p) => p.count_tokens(text),
            LlmProvider::OpenRouter(p) => p.count_tokens(text),
            LlmProvider::Zai(p) => p.count_tokens(text),
            LlmProvider::Ollama(p) => p.count_tokens(text),
            LlmProvider::Local(p) => p.count_tokens(text),
        }
    }
}

pub fn create_provider(config: &LlmConfig) -> Result<LlmProvider> {
    use crate::error::ErrorHelpers;

    match config.provider.to_lowercase().as_str() {
        "anthropic" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                Error::Config(
                    ErrorHelpers::api_key_missing("Anthropic", "ANTHROPIC_API_KEY").to_string(),
                )
            })?;
            Ok(LlmProvider::Anthropic(
                providers::anthropic::AnthropicProvider::new(api_key, Some(&config.model))?,
            ))
        },
        "google" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                Error::Config(ErrorHelpers::api_key_missing("Google", "GOOGLE_API_KEY").to_string())
            })?;
            Ok(LlmProvider::Google(providers::google::GoogleProvider::new(
                api_key,
                Some(&config.model),
            )?))
        },
        "openai" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                Error::Config(ErrorHelpers::api_key_missing("OpenAI", "OPENAI_API_KEY").to_string())
            })?;
            Ok(LlmProvider::OpenAi(providers::openai::OpenAIProvider::new(
                api_key,
                Some(&config.model),
            )?))
        },
        "openrouter" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                Error::Config(
                    ErrorHelpers::api_key_missing("OpenRouter", "OPENROUTER_API_KEY").to_string(),
                )
            })?;
            Ok(LlmProvider::OpenRouter(
                providers::openrouter::OpenRouterProvider::new(api_key, Some(&config.model))?,
            ))
        },
        "zai" | "glm" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                Error::Config(
                    ErrorHelpers::api_key_missing("ZAI", "ZAI_API_KEY").to_string(),
                )
            })?;
            Ok(LlmProvider::Zai(providers::zai::ZaiProvider::new(
                api_key,
                Some(&config.model),
            )?))
        },
        "ollama" => {
            let base_url = config
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            Ok(LlmProvider::Ollama(providers::ollama::OllamaProvider::new(
                base_url,
                Some(&config.model),
            )?))
        },
        "local" | "llama" | "mistral" => {
            let base_url = config
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            Ok(LlmProvider::Local(providers::local::LocalLlmProvider::new(
                base_url.to_string(),
                config.model.clone(),
            )))
        },
        _ => Err(Error::Config(
            ErrorHelpers::unknown_provider(
                &config.provider,
                &["anthropic", "google", "openai", "openrouter", "zai", "ollama"],
            )
            .to_string(),
        )),
    }
}

pub fn create_provider_with_retry(
    config: &LlmConfig,
    retry_config: Option<RetryConfig>,
) -> Result<LlmClientWithRetry> {
    let provider = create_provider(config)?;
    let retry = retry_config.unwrap_or_default();
    Ok(LlmClientWithRetry::new(provider, retry))
}
