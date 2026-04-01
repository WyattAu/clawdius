//! Error types for Clawdius with comprehensive error handling.
//!
//! This module provides a unified error type [`Error`] that encompasses all possible
//! error conditions in Clawdius, along with helper methods for error classification
//! and retry logic.
//!
//! # Features
//!
//! - **Comprehensive coverage**: All error conditions represented
//! - **Retry detection**: Built-in methods to check if errors are retryable
//! - **Rich context**: Detailed error messages with contextual information
//! - **Error chaining**: Proper use of `#[source]` for underlying errors
//! - **Helper methods**: Convenient methods for error classification
//! - **Enhanced errors**: Optional rich error messages with suggestions
//!
//! # Error Categories
//!
//! ## Configuration Errors
//!
//! ```rust
//! use clawdius_core::Error;
//!
//! let error = Error::Config("ANTHROPIC_API_KEY not set".to_string());
//! assert!(!error.is_retryable());
//! ```
//!
//! ## LLM Errors
//!
//! ```rust
//! use clawdius_core::Error;
//!
//! // General LLM error
//! let error = Error::Llm("Model not available".to_string());
//!
//! // Provider-specific error
//! let error = Error::LlmProvider {
//!     message: "Rate limit exceeded".to_string(),
//!     provider: "anthropic".to_string(),
//! };
//!
//! // Rate limit with retry information
//! let error = Error::RateLimited { retry_after_ms: 5000 };
//! assert!(error.is_retryable());
//! assert_eq!(error.retry_after_ms(), Some(5000));
//! ```
//!
//! ## Enhanced Errors
//!
//! Use [`EnhancedError`] for rich, actionable error messages:
//!
//! ```rust
//! use clawdius_core::error::{EnhancedError, ErrorHelpers};
//!
//! let error = ErrorHelpers::file_not_found("/path/to/file");
//! println!("{}", error.format_pretty());
//! // Shows error, context, suggestions, and documentation links
//! ```

pub mod enhanced;

pub use enhanced::{EnhancedError, ErrorHelpers};

use thiserror::Error;

/// Main error type for Clawdius
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// LLM error
    #[error("LLM error: {0}")]
    Llm(String),

    /// LLM provider error with provider context
    #[error("LLM provider error from {provider}: {message}")]
    LlmProvider {
        /// Error message
        message: String,
        /// Provider name
        provider: String,
    },

    /// Rate limited error
    #[error("Rate limited. Retry after {retry_after_ms}ms")]
    RateLimited {
        /// Milliseconds to wait before retry
        retry_after_ms: u64,
    },

    /// Context limit exceeded
    #[error("Context limit exceeded: {current}/{limit} tokens")]
    ContextLimit {
        /// Current token count
        current: usize,
        /// Maximum token limit
        limit: usize,
    },

    /// Tool execution error
    #[error("Tool execution failed '{tool}': {reason}")]
    ToolExecution {
        /// Tool name
        tool: String,
        /// Failure reason
        reason: String,
    },

    /// Session error
    #[error("Session error: {0}")]
    Session(String),

    /// Session not found
    #[error("Session not found: {id}")]
    SessionNotFound {
        /// Session ID
        id: String,
    },

    /// Context error
    #[error("Context error: {0}")]
    Context(String),

    /// Tool error
    #[error("Tool error: {0}")]
    Tool(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// TOML deserialization error
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    /// TOML serialization error
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Checkpoint error
    #[error("Checkpoint error: {0}")]
    Checkpoint(String),

    /// Sandbox error
    #[error("Sandbox error: {0}")]
    Sandbox(String),

    /// Brain runtime error
    #[error("Brain runtime error: {0}")]
    Brain(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Not found error
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Unsupported language
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    /// Out of range
    #[error("Index out of range")]
    OutOfRange,

    /// Operation cancelled
    #[error("Operation cancelled")]
    Cancelled,

    /// Timeout
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Retry exhausted
    #[error("Retry exhausted after {0} attempts")]
    RetryExhausted(u32),

    /// Circuit breaker open
    #[error("Circuit breaker open for {service}. Last error: {last_error}")]
    CircuitBreakerOpen {
        /// Service name
        service: String,
        /// Last error message
        last_error: String,
    },

    /// Generic error
    #[error("{0}")]
    Generic(String),

    /// Generic error
    #[error("{0}")]
    Other(String),

    /// Model loading error
    #[error("Model error: {0}")]
    Model(String),

    /// Processing error
    #[error("Processing error: {0}")]
    Processing(String),
}

/// Result type alias for Clawdius
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Check if this error is retryable
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::RateLimited { .. } | Error::Timeout(_) | Error::CircuitBreakerOpen { .. }
        )
    }

    /// Get retry delay in milliseconds if applicable
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Error::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            Error::Timeout(duration) => Some(duration.as_millis() as u64),
            _ => None,
        }
    }

    /// Check if error is a rate limit error
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Error::RateLimited { .. })
    }

    /// Check if error is a timeout
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout(_))
    }

    /// Check if error is a circuit breaker error
    #[must_use]
    pub fn is_circuit_breaker(&self) -> bool {
        matches!(self, Error::CircuitBreakerOpen { .. })
    }

    /// Convert to an enhanced error with context and suggestions
    #[must_use]
    pub fn into_enhanced(self) -> EnhancedError {
        EnhancedError::from(self)
    }

    /// Get a user-friendly error message
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Error::Config(msg) if msg.contains("API_KEY") => {
                if msg.contains("ANTHROPIC") {
                    format!(
                        "{msg}\n\nSet your Anthropic API key:\n  export ANTHROPIC_API_KEY=your-key"
                    )
                } else if msg.contains("OPENAI") {
                    format!("{msg}\n\nSet your OpenAI API key:\n  export OPENAI_API_KEY=your-key")
                } else if msg.contains("ZAI") {
                    format!("{msg}\n\nSet your Z.AI API key:\n  export ZAI_API_KEY=your-key")
                } else {
                    msg.clone()
                }
            },
            Error::RateLimited { retry_after_ms } => {
                format!(
                    "Rate limited. Wait {} seconds before retrying.",
                    retry_after_ms / 1000
                )
            },
            Error::ContextLimit { current, limit } => {
                format!(
                    "Context limit exceeded ({current} of {limit} tokens used).\n\nSuggestions:\n\
                     - Use 'clawdius compact' to reduce context size\n\
                     - Start a new session for a fresh context"
                )
            },
            Error::SessionNotFound { id } => {
                format!(
                    "Session '{id}' not found.\n\nList available sessions:\n  clawdius sessions"
                )
            },
            Error::Sandbox(msg) => {
                format!(
                    "Sandbox violation: {msg}\n\nThis command was blocked for security.\n\
                     Check .clawdius/config.toml for allowed commands."
                )
            },
            _ => self.to_string(),
        }
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<notify::Error> for Error {
    fn from(e: notify::Error) -> Self {
        Error::Other(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_retryable() {
        let error = Error::RateLimited {
            retry_after_ms: 1000,
        };
        assert!(error.is_retryable());

        let error = Error::Timeout(std::time::Duration::from_secs(30));
        assert!(error.is_retryable());

        let error = Error::Config("test".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_retry_after_ms() {
        let error = Error::RateLimited {
            retry_after_ms: 5000,
        };
        assert_eq!(error.retry_after_ms(), Some(5000));

        let error = Error::Timeout(std::time::Duration::from_millis(30000));
        assert_eq!(error.retry_after_ms(), Some(30000));

        let error = Error::Config("test".to_string());
        assert_eq!(error.retry_after_ms(), None);
    }

    #[test]
    fn test_user_message() {
        let error = Error::SessionNotFound {
            id: "test-123".to_string(),
        };
        let msg = error.user_message();
        assert!(msg.contains("test-123"));
        assert!(msg.contains("clawdius sessions"));
    }

    #[test]
    fn test_into_enhanced() {
        let error = Error::SessionNotFound {
            id: "test".to_string(),
        };
        let enhanced = error.into_enhanced();
        assert!(enhanced.message().contains("Session not found"));
    }
}
