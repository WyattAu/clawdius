//! Tool Executor Interface
//!
//! This module provides the trait for executing tools from the agentic system.
//! The main crate implements this trait using the MCP host.

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A tool execution request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Tool name
    pub name: String,
    /// Tool arguments
    pub arguments: HashMap<String, serde_json::Value>,
}

impl ToolRequest {
    /// Creates a new tool request.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: HashMap::new(),
        }
    }

    /// Adds an argument to the request.
    #[must_use]
    pub fn with_arg(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.arguments.insert(key.into(), value);
        self
    }

    /// Creates a request with string arguments.
    #[must_use]
    pub fn with_string_args(name: impl Into<String>, args: HashMap<String, String>) -> Self {
        Self {
            name: name.into(),
            arguments: args
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        }
    }
}

/// A tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution succeeded
    pub success: bool,
    /// Result content
    pub content: String,
    /// Whether this is an error result
    pub is_error: bool,
}

impl ToolResult {
    /// Creates a successful result.
    #[must_use]
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            is_error: false,
        }
    }

    /// Creates an error result.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: message.into(),
            is_error: true,
        }
    }

    /// Parses the content as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid JSON.
    pub fn parse_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_str(&self.content)
            .map_err(|e| crate::error::Error::ParseError(
                format!("Failed to parse tool result as JSON: {}", e)
            ))
    }
}

/// Trait for executing tools from the agentic system.
///
/// This trait is implemented by the main crate using the MCP host,
/// allowing the executor agent to call tools without knowing the
/// implementation details.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool by name with the given arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails.
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult>;

    /// Check if a tool exists.
    fn has_tool(&self, name: &str) -> bool;

    /// List available tools.
    fn list_tools(&self) -> Vec<ToolDefinition>;
}

/// Definition of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Creates a new tool definition.
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
}

/// A no-op tool executor for testing and when no tools are available.
pub struct NoOpToolExecutor;

#[async_trait]
impl ToolExecutor for NoOpToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult> {
        Ok(ToolResult::success(format!(
            "No-op executor: tool '{}' called with {:?}",
            request.name, request.arguments
        )))
    }

    fn has_tool(&self, _name: &str) -> bool {
        false
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_request_builder() {
        let request = ToolRequest::new("search_ast")
            .with_arg("node_type", serde_json::json!("function"))
            .with_arg("limit", serde_json::json!(10));

        assert_eq!(request.name, "search_ast");
        assert_eq!(request.arguments.get("node_type").unwrap(), "function");
        assert_eq!(request.arguments.get("limit").unwrap(), 10);
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("Hello, world!");
        assert!(result.success);
        assert!(!result.is_error);
        assert_eq!(result.content, "Hello, world!");
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("Something went wrong");
        assert!(!result.success);
        assert!(result.is_error);
        assert_eq!(result.content, "Something went wrong");
    }

    #[tokio::test]
    async fn test_noop_executor() {
        let executor = NoOpToolExecutor;
        let request = ToolRequest::new("test_tool");
        let result = executor.execute(request).await.unwrap();

        assert!(result.success);
        assert!(result.content.contains("test_tool"));
    }
}
