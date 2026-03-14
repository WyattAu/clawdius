//! MCP Protocol Types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// MCP protocol version
pub const MCP_VERSION: &str = "2024.11";

/// Cancellation token for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationToken {
    /// Unique token ID
    pub id: String,
    /// Reason for cancellation (optional)
    pub reason: Option<String>,
}

/// Progress notification for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotification {
    /// Progress token (matches request token)
    pub progress_token: String,
    /// Current progress (0.0 to 1.0)
    pub progress: f64,
    /// Total units (optional)
    pub total: Option<u64>,
    /// Message describing current state
    pub message: Option<String>,
}

/// Sampling request (LLM completion from server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    /// Messages to include in the prompt
    pub messages: Vec<SamplingMessage>,
    /// System prompt (optional)
    pub system_prompt: Option<String>,
    /// Include context (none/thisServer/allServers)
    #[serde(default)]
    pub include_context: Option<String>,
    /// Temperature (0.0 to 1.0)
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Maximum tokens
    pub max_tokens: u32,
    /// Stop sequences
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    /// Model preferences
    #[serde(default)]
    pub model_preferences: Option<ModelPreferences>,
}

/// Sampling message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    /// Role (user or assistant)
    pub role: String,
    /// Message content
    pub content: SamplingContent,
}

/// Sampling content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SamplingContent {
    /// Text content
    Text {
        /// Text
        text: String,
    },
    /// Image content
    Image {
        /// Base64 data
        data: String,
        /// MIME type
        mime_type: String,
    },
}

/// Model preferences for sampling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreferences {
    /// Hints for model selection
    #[serde(default)]
    pub hints: Option<Vec<ModelHint>>,
    /// Cost priority (0.0 to 1.0)
    #[serde(default)]
    pub cost_priority: Option<f64>,
    /// Speed priority (0.0 to 1.0)
    #[serde(default)]
    pub speed_priority: Option<f64>,
    /// Intelligence priority (0.0 to 1.0)
    #[serde(default)]
    pub intelligence_priority: Option<f64>,
}

/// Model hint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHint {
    /// Model name hint
    pub name: Option<String>,
}

/// Sampling response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingResponse {
    /// Model used
    pub model: String,
    /// Stop reason
    pub stop_reason: Option<String>,
    /// Generated content
    pub content: SamplingContent,
}

/// Resource subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSubscription {
    /// Resource URI
    pub uri: String,
}

/// Resource update notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdatedNotification {
    /// Resource URI
    pub uri: String,
}

/// Tool definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (unique identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: schema,
        }
    }
}

/// Tool execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Tool name to execute
    pub name: String,
    /// Tool arguments
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Tool execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Tool output content
    pub content: Vec<ContentBlock>,
    /// Whether the tool execution failed
    pub is_error: bool,
}

impl ToolResponse {
    /// Create a successful text response
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
            is_error: false,
        }
    }

    /// Create an error response
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

/// Content block in tool response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    /// Text content
    Text {
        /// Text content
        text: String,
    },
    /// Image content
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type
        mime_type: String,
    },
    /// Resource reference
    Resource {
        /// Resource URI
        uri: String,
        /// MIME type
        mime_type: Option<String>,
    },
}

/// Resource definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// Resource URI
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Resource description
    pub description: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
}

/// Resource content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// Resource URI
    pub uri: String,
    /// MIME type
    pub mime_type: Option<String>,
    /// Text content (if text)
    pub text: Option<String>,
    /// Binary content (if binary, base64 encoded)
    pub blob: Option<String>,
}

/// Prompt definition for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    /// Prompt name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Prompt arguments
    pub arguments: Vec<PromptArgument>,
}

/// Prompt argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: Option<String>,
    /// Whether argument is required
    pub required: bool,
}

/// MCP error
#[derive(Debug, Clone, Error)]
pub enum McpError {
    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    /// Invalid arguments
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    /// Execution error
    #[error("Execution error: {0}")]
    ExecutionError(String),
    /// Resource not found
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl Serialize for McpError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for McpError {
    fn deserialize<D>(_deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(serde::de::Error::custom("Cannot deserialize McpError"))
    }
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Protocol version
    pub protocol_version: String,
}

impl ServerInfo {
    /// Create new server info
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            protocol_version: MCP_VERSION.to_string(),
        }
    }
}

/// Server capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Tool support
    pub tools: Option<ToolsCapability>,
    /// Resource support
    pub resources: Option<ResourcesCapability>,
    /// Prompt support
    pub prompts: Option<PromptsCapability>,
}

/// Tools capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    /// Whether list changed notifications are supported
    pub list_changed: bool,
}

/// Resources capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    /// Whether subscribe is supported
    pub subscribe: bool,
    /// Whether list changed notifications are supported
    pub list_changed: bool,
}

/// Prompts capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    /// Whether list changed notifications are supported
    pub list_changed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            }
        });

        let tool = ToolDefinition::new("search", "Search for code", schema);
        assert_eq!(tool.name, "search");
    }

    #[test]
    fn test_tool_response() {
        let response = ToolResponse::text("Hello, world!");
        assert!(!response.is_error);
        assert_eq!(response.content.len(), 1);

        let error = ToolResponse::error("Something went wrong");
        assert!(error.is_error);
    }

    #[test]
    fn test_server_info() {
        let info = ServerInfo::new("clawdius", "0.1.0");
        assert_eq!(info.name, "clawdius");
        assert_eq!(info.protocol_version, MCP_VERSION);
    }
}
