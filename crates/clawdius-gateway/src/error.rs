//! Gateway error types.

use thiserror::Error;

/// Errors that can occur in the gateway layer.
#[derive(Debug, Error)]
pub enum GatewayError {
    /// The requested platform is not configured.
    #[error("platform not configured: {0}")]
    PlatformNotConfigured(String),

    /// An adapter operation failed.
    #[error("adapter error on platform '{platform}': {message}")]
    Adapter {
        /// Platform identifier.
        platform: String,
        /// Error message.
        message: String,
        /// Underlying source error, if any.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Rate limit exceeded.
    #[error("rate limit exceeded for user '{user_id}' on platform '{platform}': retry after {retry_after_ms}ms")]
    RateLimited {
        /// User identifier.
        user_id: String,
        /// Platform identifier.
        platform: String,
        /// Milliseconds until the rate limit resets.
        retry_after_ms: u64,
    },

    /// Message too large for the platform.
    #[error("message too large for platform '{platform}': {size} bytes (max {max_size})")]
    MessageTooLarge {
        /// Platform identifier.
        platform: String,
        /// Actual message size in bytes.
        size: usize,
        /// Maximum allowed size in bytes.
        max_size: usize,
    },

    /// Authentication/authorization failure.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// The underlying agent engine returned an error.
    #[error("agent error: {0}")]
    Agent(String),

    /// IO or network error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
