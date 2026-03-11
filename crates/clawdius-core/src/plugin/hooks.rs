//! Plugin Hooks System
//!
//! Defines the hook points where plugins can inject custom behavior.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hook types available in the plugin system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookType {
    // Lifecycle hooks
    /// Called when Clawdius starts
    OnStartup,
    /// Called when Clawdius shuts down
    OnShutdown,
    /// Called when a new session is created
    OnSessionCreate,
    /// Called when a session is destroyed
    OnSessionDestroy,
    /// Called when a session becomes active
    OnSessionActivate,

    // LLM hooks
    /// Called before sending a prompt to the LLM
    BeforeLLMRequest,
    /// Called after receiving a response from the LLM
    AfterLLMResponse,
    /// Called when streaming a token
    OnStreamToken,

    // Tool hooks
    /// Called before executing a tool
    BeforeToolExecute,
    /// Called after a tool completes
    AfterToolExecute,
    /// Called to register custom tools
    OnToolRegister,

    // File hooks
    /// Called before reading a file
    BeforeFileRead,
    /// Called after reading a file
    AfterFileRead,
    /// Called before writing a file
    BeforeFileWrite,
    /// Called after writing a file
    AfterFileWrite,
    /// Called before deleting a file
    BeforeFileDelete,

    // Code hooks
    /// Called before applying an edit
    BeforeEdit,
    /// Called after applying an edit
    AfterEdit,
    /// Called before running code analysis
    BeforeAnalysis,
    /// Called after code analysis completes
    AfterAnalysis,

    // Command hooks
    /// Called before executing a shell command
    BeforeCommand,
    /// Called after a command completes
    AfterCommand,

    // Event hooks
    /// Called on any event
    OnEvent,
    /// Called on error
    OnError,
    /// Called on warning
    OnWarning,

    // Custom hooks (plugin-defined)
    Custom,
}

impl HookType {
    /// Get the hook name as a string
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            HookType::OnStartup => "on_startup",
            HookType::OnShutdown => "on_shutdown",
            HookType::OnSessionCreate => "on_session_create",
            HookType::OnSessionDestroy => "on_session_destroy",
            HookType::OnSessionActivate => "on_session_activate",
            HookType::BeforeLLMRequest => "before_llm_request",
            HookType::AfterLLMResponse => "after_llm_response",
            HookType::OnStreamToken => "on_stream_token",
            HookType::BeforeToolExecute => "before_tool_execute",
            HookType::AfterToolExecute => "after_tool_execute",
            HookType::OnToolRegister => "on_tool_register",
            HookType::BeforeFileRead => "before_file_read",
            HookType::AfterFileRead => "after_file_read",
            HookType::BeforeFileWrite => "before_file_write",
            HookType::AfterFileWrite => "after_file_write",
            HookType::BeforeFileDelete => "before_file_delete",
            HookType::BeforeEdit => "before_edit",
            HookType::AfterEdit => "after_edit",
            HookType::BeforeAnalysis => "before_analysis",
            HookType::AfterAnalysis => "after_analysis",
            HookType::BeforeCommand => "before_command",
            HookType::AfterCommand => "after_command",
            HookType::OnEvent => "on_event",
            HookType::OnError => "on_error",
            HookType::OnWarning => "on_warning",
            HookType::Custom => "custom",
        }
    }

    /// Parse a hook type from a string
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "on_startup" => Some(HookType::OnStartup),
            "on_shutdown" => Some(HookType::OnShutdown),
            "on_session_create" => Some(HookType::OnSessionCreate),
            "on_session_destroy" => Some(HookType::OnSessionDestroy),
            "on_session_activate" => Some(HookType::OnSessionActivate),
            "before_llm_request" => Some(HookType::BeforeLLMRequest),
            "after_llm_response" => Some(HookType::AfterLLMResponse),
            "on_stream_token" => Some(HookType::OnStreamToken),
            "before_tool_execute" => Some(HookType::BeforeToolExecute),
            "after_tool_execute" => Some(HookType::AfterToolExecute),
            "on_tool_register" => Some(HookType::OnToolRegister),
            "before_file_read" => Some(HookType::BeforeFileRead),
            "after_file_read" => Some(HookType::AfterFileRead),
            "before_file_write" => Some(HookType::BeforeFileWrite),
            "after_file_write" => Some(HookType::AfterFileWrite),
            "before_file_delete" => Some(HookType::BeforeFileDelete),
            "before_edit" => Some(HookType::BeforeEdit),
            "after_edit" => Some(HookType::AfterEdit),
            "before_analysis" => Some(HookType::BeforeAnalysis),
            "after_analysis" => Some(HookType::AfterAnalysis),
            "before_command" => Some(HookType::BeforeCommand),
            "after_command" => Some(HookType::AfterCommand),
            "on_event" => Some(HookType::OnEvent),
            "on_error" => Some(HookType::OnError),
            "on_warning" => Some(HookType::OnWarning),
            "custom" => Some(HookType::Custom),
            _ => None,
        }
    }

    /// Get all available hook types
    #[must_use]
    pub fn all() -> &'static [HookType] {
        &[
            HookType::OnStartup,
            HookType::OnShutdown,
            HookType::OnSessionCreate,
            HookType::OnSessionDestroy,
            HookType::OnSessionActivate,
            HookType::BeforeLLMRequest,
            HookType::AfterLLMResponse,
            HookType::OnStreamToken,
            HookType::BeforeToolExecute,
            HookType::AfterToolExecute,
            HookType::OnToolRegister,
            HookType::BeforeFileRead,
            HookType::AfterFileRead,
            HookType::BeforeFileWrite,
            HookType::AfterFileWrite,
            HookType::BeforeFileDelete,
            HookType::BeforeEdit,
            HookType::AfterEdit,
            HookType::BeforeAnalysis,
            HookType::AfterAnalysis,
            HookType::BeforeCommand,
            HookType::AfterCommand,
            HookType::OnEvent,
            HookType::OnError,
            HookType::OnWarning,
            HookType::Custom,
        ]
    }
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Context provided to hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Type of hook being executed
    pub hook_type: HookType,
    /// Session ID (if applicable)
    pub session_id: Option<String>,
    /// Timestamp when the hook was triggered
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event data specific to the hook type
    pub data: HashMap<String, serde_json::Value>,
    /// Whether the event can be cancelled
    pub cancellable: bool,
    /// Whether the event has been cancelled
    pub cancelled: bool,
}

impl HookContext {
    /// Create a new hook context
    #[must_use]
    pub fn new(hook_type: HookType) -> Self {
        Self {
            hook_type,
            session_id: None,
            timestamp: chrono::Utc::now(),
            data: HashMap::new(),
            cancellable: false,
            cancelled: false,
        }
    }

    /// Create a context with session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add data to the context
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Make the event cancellable
    #[must_use]
    pub fn cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    /// Cancel the event
    pub fn cancel(&mut self) {
        if self.cancellable {
            self.cancelled = true;
        }
    }

    /// Get a data value
    #[must_use]
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

/// Hook subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSubscription {
    /// Hook type to subscribe to
    pub hook_type: HookType,
    /// Priority (higher = runs first)
    pub priority: i32,
    /// Whether to run asynchronously
    pub async_execution: bool,
    /// Filter expression (optional)
    pub filter: Option<String>,
}

impl HookSubscription {
    #[must_use]
    pub fn new(hook_type: HookType) -> Self {
        Self {
            hook_type,
            priority: 0,
            async_execution: false,
            filter: None,
        }
    }

    #[must_use]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn async_execution(mut self) -> Self {
        self.async_execution = true;
        self
    }

    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }
}

/// Hook execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookStats {
    /// Total number of invocations
    pub invocations: u64,
    /// Number of successful executions
    pub successes: u64,
    /// Number of failed executions
    pub failures: u64,
    /// Total execution time in microseconds
    pub total_time_us: u64,
    /// Maximum execution time in microseconds
    pub max_time_us: u64,
}

impl HookStats {
    pub fn record(&mut self, duration_us: u64, success: bool) {
        self.invocations += 1;
        if success {
            self.successes += 1;
        } else {
            self.failures += 1;
        }
        self.total_time_us += duration_us;
        self.max_time_us = self.max_time_us.max(duration_us);
    }

    #[must_use]
    pub fn average_time_us(&self) -> f64 {
        if self.invocations == 0 {
            0.0
        } else {
            self.total_time_us as f64 / self.invocations as f64
        }
    }
}
