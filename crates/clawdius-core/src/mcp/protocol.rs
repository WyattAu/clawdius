//! MCP Protocol Types
//!
//! Defines the core types for the Model Context Protocol.

use serde::{Deserialize, Serialize};

/// MCP Protocol version
pub const MCP_VERSION: &str = "2024-11-05";

/// An MCP message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpMessage {
    /// A request
    Request(McpRequest),
    /// A response
    Response(McpResponse),
}

/// An MCP request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: u64,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Creates a new request.
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params: None,
        }
    }

    /// Adds parameters to the request.
    #[must_use]
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }
}

/// An MCP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: u64,
    /// Result (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    /// Creates a successful response.
    #[must_use]
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error response.
    #[must_use]
    pub fn error(id: u64, error: McpError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// An MCP error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    /// Creates a new error.
    #[must_use]
    pub const fn new(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }

    /// Parse error (-32700)
    #[must_use]
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::new(-32700, msg.into())
    }

    /// Invalid request (-32600)
    #[must_use]
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new(-32600, msg.into())
    }

    /// Method not found (-32601)
    #[must_use]
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::new(-32601, format!("Method not found: {}", method.into()))
    }

    /// Invalid params (-32602)
    #[must_use]
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(-32602, msg.into())
    }

    /// Internal error (-32603)
    #[must_use]
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::new(-32603, msg.into())
    }
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MCP Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for McpError {}

/// Server capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpCapabilities {
    /// Tool support
    #[serde(default)]
    pub tools: Option<McpToolCapabilities>,
    /// Resource support
    #[serde(default)]
    pub resources: Option<McpResourceCapabilities>,
    /// Prompt support
    #[serde(default)]
    pub prompts: Option<McpPromptCapabilities>,
}

/// Tool capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCapabilities {
    /// Supported capabilities
    #[serde(default)]
    pub list_changed: bool,
}

/// Resource capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceCapabilities {
    /// Supports list_changed
    #[serde(default)]
    pub list_changed: bool,
    /// Supports subscribe
    #[serde(default)]
    pub subscribe: bool,
}

/// Prompt capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptCapabilities {
    /// Supports list_changed
    #[serde(default)]
    pub list_changed: bool,
}

/// Server information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
}

impl McpServerInfo {
    /// Creates server info.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

/// Initialize result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    /// Protocol version
    pub protocol_version: String,
    /// Server capabilities
    pub capabilities: McpCapabilities,
    /// Server info
    pub server_info: McpServerInfo,
}

/// An MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema
    pub input_schema: serde_json::Value,
}

impl McpTool {
    /// Creates a new tool.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Sets the input schema.
    #[must_use]
    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = schema;
        self
    }

    /// Adds a string parameter.
    #[must_use]
    pub fn with_string_param(mut self, name: &str, description: &str, required: bool) -> Self {
        if let Some(obj) = self.input_schema.as_object_mut() {
            if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
                props.insert(
                    name.to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": description
                    }),
                );
            }
            if required {
                if let Some(req) = obj.get_mut("required").and_then(|r| r.as_array_mut()) {
                    req.push(serde_json::json!(name));
                }
            }
        }
        self
    }
}

/// Tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Content items
    pub content: Vec<McpContent>,
    /// Whether the tool failed
    #[serde(default)]
    pub is_error: bool,
}

impl McpToolResult {
    /// Creates a text result.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::text(text)],
            is_error: false,
        }
    }

    /// Creates an error result.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::text(message)],
            is_error: true,
        }
    }
}

/// Content in an MCP message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    /// Text content
    Text {
        /// The text
        text: String,
    },
    /// Image content
    Image {
        /// Base64 data
        data: String,
        /// MIME type
        mime_type: String,
    },
    /// Resource reference
    Resource {
        /// Resource URI
        uri: String,
        /// MIME type
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        /// Resource text (if text)
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

impl McpContent {
    /// Creates text content.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Creates image content.
    #[must_use]
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// An MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Resource URI
    pub uri: String,
    /// Resource name
    pub name: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl McpResource {
    /// Creates a new resource.
    #[must_use]
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            mime_type: None,
        }
    }
}

/// An MCP prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    /// Prompt name
    pub name: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments
    #[serde(default)]
    pub arguments: Vec<McpPromptArgument>,
}

/// A prompt argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgument {
    /// Argument name
    pub name: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether required
    #[serde(default)]
    pub required: bool,
}

impl McpPrompt {
    /// Creates a new prompt.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            arguments: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let req = McpRequest::new(1, "test_method");
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "test_method");
    }

    #[test]
    fn test_response_success() {
        let resp = McpResponse::success(1, serde_json::json!({"ok": true}));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_error_codes() {
        let err = McpError::method_not_found("foo");
        assert_eq!(err.code, -32601);
    }

    #[test]
    fn test_tool_creation() {
        let tool =
            McpTool::new("test", "A test tool").with_string_param("input", "The input", true);
        assert_eq!(tool.name, "test");
        assert!(tool.input_schema["properties"]["input"].is_object());
    }

    #[test]
    fn test_content() {
        let text = McpContent::text("Hello");
        match text {
            McpContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text"),
        }
    }

    #[test]
    fn test_serialization() {
        let tool = McpTool::new("test", "Test");
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("test"));
    }
}
