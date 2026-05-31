use std::collections::HashMap;

/// Describes a single content block within a tool result.
#[derive(Debug, Clone)]
pub enum ToolContent {
    /// Plain text content.
    Text(String),
}

/// Result produced by a tool handler.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Ordered list of content blocks.
    pub content: Vec<ToolContent>,
    /// Whether this result represents an error.
    pub is_error: bool,
}

impl ToolResult {
    /// Creates a successful tool result with the given text content.
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text(text.into())],
            is_error: false,
        }
    }

    /// Creates an error tool result with the given message.
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text(msg.into())],
            is_error: true,
        }
    }
}

/// Context passed to tool handlers at invocation time.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// The ID of the session or conversation invoking the tool.
    pub session_id: Option<String>,
    /// The ID of the plugin that registered this tool.
    pub plugin_name: String,
}

/// Invocation data passed to a tool handler.
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    /// The tool name being invoked.
    pub name: String,
    /// The parameters provided by the caller.
    pub params: serde_json::Value,
    /// Execution context including session and plugin metadata.
    pub context: ToolContext,
}

/// A function pointer type for tool handlers.
pub type ToolHandlerFn = fn(&ToolInvocation) -> ToolResult;

/// Registration record for a single tool.
pub struct ToolRegistration {
    /// Unique tool name (e.g. `"my_plugin.my_tool"`).
    pub name: String,
    /// Human-readable description for display and discovery.
    pub description: String,
    /// Function pointer invoked when this tool is called.
    pub handler: ToolHandlerFn,
}

/// Central registry that collects tools from all loaded plugins.
///
/// The host creates one registry, calls `register_tools` on each plugin,
/// then uses the registry as the single dispatch point.
pub struct ToolRegistry {
    tools: HashMap<String, ToolRegistration>,
}

impl ToolRegistry {
    /// Creates an empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool. Overwrites any existing tool with the same name.
    pub fn register(&mut self, reg: ToolRegistration) {
        self.tools.insert(reg.name.clone(), reg);
    }

    /// Looks up a registered tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ToolRegistration> {
        self.tools.get(name)
    }

    /// Returns references to all registered tools.
    #[must_use]
    pub fn list_tools(&self) -> Vec<&ToolRegistration> {
        self.tools.values().collect()
    }

    /// Returns the number of registered tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns true if no tools are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
