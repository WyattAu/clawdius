use thiserror::Error;

/// Errors that can occur during plugin operations.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin initialization failed: {0}")]
    InitFailed(String),

    #[error("tool registration failed: {0}")]
    RegistrationFailed(String),

    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("plugin shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("invalid plugin configuration: {0}")]
    InvalidConfig(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl PluginError {
    pub fn init_failed(msg: impl Into<String>) -> Self {
        Self::InitFailed(msg.into())
    }

    pub fn registration_failed(msg: impl Into<String>) -> Self {
        Self::RegistrationFailed(msg.into())
    }

    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    pub fn shutdown_failed(msg: impl Into<String>) -> Self {
        Self::ShutdownFailed(msg.into())
    }
}

pub type PluginResult<T> = Result<T, PluginError>;
