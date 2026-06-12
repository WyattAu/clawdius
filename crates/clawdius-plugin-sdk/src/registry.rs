use std::collections::HashMap;

use std::sync::Arc;

use crate::error::PluginError;
use crate::tool::Tool;

/// Describes a single content block within a tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolContent {
    /// Plain text content.
    Text(String),
}

/// Result produced by a tool handler.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default)]
pub struct PluginToolContext {
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
    pub context: PluginToolContext,
}

/// A function pointer type for tool handlers.
pub type ToolHandlerFn = fn(&ToolInvocation) -> ToolResult;

/// Registration record for a single tool (function-pointer based).
pub struct ToolRegistration {
    /// Unique tool name (e.g. `"my_plugin.my_tool"`).
    pub name: String,
    /// Human-readable description for display and discovery.
    pub description: String,
    /// Function pointer invoked when this tool is called.
    pub handler: ToolHandlerFn,
}

/// Internal enum to hold either a trait-object tool or a function-pointer registration.
enum ToolEntry {
    /// A trait-object implementing [`Tool`].
    TraitObj(Arc<dyn Tool>),
    /// A function-pointer based registration.
    FnPtr(ToolRegistration),
}

/// Central registry that collects tools from all loaded plugins.
///
/// The host creates one registry, calls `register_tools` on each plugin,
/// then uses the registry as the single dispatch point.
///
/// Supports two registration styles:
/// - [`Tool`] trait objects (recommended for rich tools with schemas)
/// - [`ToolRegistration`] function pointers (lightweight, no schema)
pub struct ToolRegistry {
    tools: HashMap<String, ToolEntry>,
}

impl ToolRegistry {
    /// Creates an empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a [`Tool`] trait object.
    ///
    /// Returns an error if a tool with the same name already exists.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::DuplicateTool`] if the name is already registered.
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) -> Result<(), PluginError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(PluginError::duplicate_tool(name));
        }
        self.tools.insert(name, ToolEntry::TraitObj(tool));
        Ok(())
    }

    /// Registers a [`ToolRegistration`], overwriting any existing tool with the same name.
    pub fn register(&mut self, reg: ToolRegistration) {
        self.tools.insert(reg.name.clone(), ToolEntry::FnPtr(reg));
    }

    /// Removes a tool from the registry by name.
    ///
    /// Returns `true` if a tool was removed, `false` if no tool with that
    /// name existed.
    pub fn unregister_tool(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }

    /// Looks up a registered [`Tool`] trait object by name.
    ///
    /// Returns `None` if the tool is registered as a function pointer rather
    /// than a trait object.
    #[must_use]
    pub fn get_trait_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).and_then(|entry| match entry {
            ToolEntry::TraitObj(t) => Some(t.as_ref()),
            ToolEntry::FnPtr(_) => None,
        })
    }

    /// Looks up a registered [`ToolRegistration`] by name.
    ///
    /// Returns `None` if the tool is registered as a trait object rather
    /// than a function pointer.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ToolRegistration> {
        self.tools.get(name).and_then(|entry| match entry {
            ToolEntry::FnPtr(r) => Some(r),
            ToolEntry::TraitObj(_) => None,
        })
    }

    /// Returns the names of all registered tools.
    #[must_use]
    pub fn list_tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }

    /// Returns references to all registered [`ToolRegistration`]s.
    ///
    /// Only includes function-pointer based registrations; trait-object
    /// tools are excluded.
    #[must_use]
    pub fn list_tools(&self) -> Vec<&ToolRegistration> {
        self.tools
            .values()
            .filter_map(|entry| match entry {
                ToolEntry::FnPtr(r) => Some(r),
                ToolEntry::TraitObj(_) => None,
            })
            .collect()
    }

    /// Returns `true` if a tool with the given name is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
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

    /// Invokes a registered [`Tool`] trait object by name with the given parameters.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::ToolNotFound`] if the tool doesn't exist.
    pub fn invoke(
        &self,
        name: &str,
        params: serde_json::Value,
        context: PluginToolContext,
    ) -> Result<ToolResult, PluginError> {
        match self.tools.get(name) {
            Some(ToolEntry::TraitObj(tool)) => {
                let invocation = ToolInvocation {
                    name: name.to_string(),
                    params,
                    context,
                };
                Ok(tool.execute(&invocation))
            },
            Some(ToolEntry::FnPtr(reg)) => {
                let invocation = ToolInvocation {
                    name: name.to_string(),
                    params,
                    context,
                };
                Ok((reg.handler)(&invocation))
            },
            None => Err(PluginError::tool_not_found(name)),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_handler(invocation: &ToolInvocation) -> ToolResult {
        ToolResult::ok(format!("echo: {}", invocation.params))
    }

    #[test]
    fn test_registry_new_is_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_fn_ptr_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolRegistration {
            name: "test.echo".into(),
            description: "Echo tool".into(),
            handler: echo_handler,
        });
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test.echo"));
    }

    #[test]
    fn test_register_fn_ptr_overwrites() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolRegistration {
            name: "test.echo".into(),
            description: "First".into(),
            handler: echo_handler,
        });
        registry.register(ToolRegistration {
            name: "test.echo".into(),
            description: "Second".into(),
            handler: echo_handler,
        });
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test.echo"));
    }

    #[test]
    fn test_unregister_existing_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolRegistration {
            name: "test.echo".into(),
            description: "Echo".into(),
            handler: echo_handler,
        });
        let removed = registry.unregister_tool("test.echo");
        assert!(removed);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_unregister_missing_tool() {
        let mut registry = ToolRegistry::new();
        let removed = registry.unregister_tool("nope");
        assert!(!removed);
    }

    #[test]
    fn test_list_tools_fn_ptr_only() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolRegistration {
            name: "a".into(),
            description: "A".into(),
            handler: echo_handler,
        });
        registry.register(ToolRegistration {
            name: "b".into(),
            description: "B".into(),
            handler: echo_handler,
        });
        assert_eq!(registry.list_tools().len(), 2);
    }

    #[test]
    fn test_list_tool_names() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolRegistration {
            name: "alpha".into(),
            description: "Alpha".into(),
            handler: echo_handler,
        });
        registry.register(ToolRegistration {
            name: "beta".into(),
            description: "Beta".into(),
            handler: echo_handler,
        });
        let mut names = registry.list_tool_names();
        names.sort_unstable();
        assert_eq!(names, vec!["alpha", "beta"]);
    }
}
