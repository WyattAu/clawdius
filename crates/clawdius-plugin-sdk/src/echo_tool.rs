use crate::error::{PluginError, PluginResult};
use crate::registry::{ToolInvocation, ToolResult};
use crate::tool::Tool;
use serde_json::{json, Value};

/// An in-memory echo tool for testing and demonstration purposes.
///
/// Echoes back the `message` field from its input parameters.
#[derive(Debug, Default)]
pub struct EchoTool {
    /// Optional prefix prepended to every echo output.
    prefix: Option<String>,
}

impl EchoTool {
    /// Creates a new `EchoTool` with no prefix.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `EchoTool` with a custom prefix.
    #[must_use]
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
        }
    }
}

impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &'static str {
        "Echoes back the provided message. Used for testing and demonstration."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"]
        })
    }

    fn validate_args(&self, args: &Value) -> PluginResult<()> {
        let has_message = args.get("message").is_some_and(Value::is_string);
        if !has_message {
            return Err(PluginError::validation_error(
                "echo",
                "missing required string field 'message'",
            ));
        }
        Ok(())
    }

    fn execute(&self, invocation: &ToolInvocation) -> ToolResult {
        invocation
            .params
            .get("message")
            .and_then(Value::as_str)
            .map_or_else(
                || ToolResult::err("missing 'message' parameter"),
                |msg| {
                    let output = self
                        .prefix
                        .as_ref()
                        .map_or_else(|| msg.to_string(), |p| format!("{p}: {msg}"));
                    ToolResult::ok(output)
                },
            )
    }
}

/// A dummy tool that always returns a fixed response. Useful in tests.
#[derive(Debug)]
pub struct DummyTool {
    name: String,
    response: String,
}

impl DummyTool {
    /// Creates a new `DummyTool` that responds with the given string.
    #[must_use]
    pub fn new(name: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            response: response.into(),
        }
    }
}

impl Tool for DummyTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &'static str {
        "A dummy tool for testing purposes"
    }

    fn schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn validate_args(&self, _args: &Value) -> PluginResult<()> {
        Ok(())
    }

    fn execute(&self, _invocation: &ToolInvocation) -> ToolResult {
        ToolResult::ok(self.response.as_str())
    }
}
