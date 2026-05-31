use thiserror::Error;

/// Errors that can occur during plugin operations.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Plugin initialization failed.
    #[error("plugin initialization failed: {0}")]
    InitFailed(String),

    /// Tool registration failed.
    #[error("tool registration failed: {0}")]
    RegistrationFailed(String),

    /// Tool execution failed.
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),

    /// Plugin shutdown failed.
    #[error("plugin shutdown failed: {0}")]
    ShutdownFailed(String),

    /// Invalid plugin configuration.
    #[error("invalid plugin configuration: {0}")]
    InvalidConfig(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl PluginError {
    /// Creates an `InitFailed` error with the given message.
    pub fn init_failed(msg: impl Into<String>) -> Self {
        Self::InitFailed(msg.into())
    }

    /// Creates a `RegistrationFailed` error with the given message.
    pub fn registration_failed(msg: impl Into<String>) -> Self {
        Self::RegistrationFailed(msg.into())
    }

    /// Creates an `ExecutionFailed` error with the given message.
    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Creates a `ShutdownFailed` error with the given message.
    pub fn shutdown_failed(msg: impl Into<String>) -> Self {
        Self::ShutdownFailed(msg.into())
    }
}

/// Convenience alias for results using [`PluginError`].
pub type PluginResult<T> = Result<T, PluginError>;
