use crate::error::PluginError;
use crate::registry::{ToolInvocation, ToolResult};

/// Trait defining a tool that can be registered with the plugin SDK.
///
/// Implementors provide a name, description, JSON Schema for parameters,
/// argument validation, and execution logic. Tools are registered with a
/// [`ToolRegistry`](crate::registry::ToolRegistry) and invoked by the host.
///
/// # Example
///
/// ```ignore
/// use clawdius_plugin_sdk::prelude::*;
/// use serde_json::json;
///
/// struct GreetTool;
///
/// impl Tool for GreetTool {
///     fn name(&self) -> &str { "greet" }
///     fn description(&self) -> &str { "Greets the user" }
///     fn schema(&self) -> serde_json::Value {
///         json!({
///             "type": "object",
///             "properties": {
///                 "name": { "type": "string" }
///             },
///             "required": ["name"]
///         })
///     }
///     fn validate_args(&self, args: &serde_json::Value) -> Result<(), PluginError> {
///         if args.get("name").and_then(|v| v.as_str()).is_none() {
///             return Err(PluginError::validation_error("greet", "missing 'name' field"));
///         }
///         Ok(())
///     }
///     fn execute(&self, invocation: &ToolInvocation) -> ToolResult {
///         let name = invocation.params.get("name")
///             .and_then(|v| v.as_str())
///             .unwrap_or("world");
///         ToolResult::ok(format!("Hello, {name}!"))
///     }
/// }
/// ```
pub trait Tool: Send + Sync {
    /// Returns the unique name of this tool.
    fn name(&self) -> &str;

    /// Returns a human-readable description for display and discovery.
    fn description(&self) -> &str;

    /// Returns the JSON Schema describing this tool's expected parameters.
    fn schema(&self) -> serde_json::Value;

    /// Validates the given arguments against this tool's requirements.
    ///
    /// Called before [`execute`](Self::execute) to ensure arguments are well-formed.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::ValidationError`] if the arguments are invalid.
    fn validate_args(&self, args: &serde_json::Value) -> Result<(), PluginError>;

    /// Executes the tool with the given invocation context.
    fn execute(&self, invocation: &ToolInvocation) -> ToolResult;
}
