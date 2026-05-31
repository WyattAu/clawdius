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

    /// A tool with the same name is already registered.
    #[error("duplicate tool name: {0}")]
    DuplicateTool(String),

    /// Tool not found in registry.
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    /// Argument validation failed.
    #[error("argument validation failed for tool '{tool}': {reason}")]
    ValidationError {
        /// The name of the tool that failed validation.
        tool: String,
        /// The reason for the validation failure.
        reason: String,
    },
}

impl PluginError {
    /// Creates an `InitFailed` error.
    #[must_use]
    pub fn init_failed(msg: impl Into<String>) -> Self {
        Self::InitFailed(msg.into())
    }

    /// Creates a `RegistrationFailed` error.
    #[must_use]
    pub fn registration_failed(msg: impl Into<String>) -> Self {
        Self::RegistrationFailed(msg.into())
    }

    /// Creates an `ExecutionFailed` error.
    #[must_use]
    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Creates a `ShutdownFailed` error.
    #[must_use]
    pub fn shutdown_failed(msg: impl Into<String>) -> Self {
        Self::ShutdownFailed(msg.into())
    }

    /// Creates a `DuplicateTool` error.
    #[must_use]
    pub fn duplicate_tool(name: impl Into<String>) -> Self {
        Self::DuplicateTool(name.into())
    }

    /// Creates a `ToolNotFound` error.
    #[must_use]
    pub fn tool_not_found(name: impl Into<String>) -> Self {
        Self::ToolNotFound(name.into())
    }

    /// Creates a `ValidationError` error.
    #[must_use]
    pub fn validation_error(tool: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationError {
            tool: tool.into(),
            reason: reason.into(),
        }
    }
}

/// A type alias for `Result` with [`PluginError`] as the error type.
pub type PluginResult<T> = Result<T, PluginError>;
