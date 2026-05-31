use std::collections::HashMap;
use std::sync::Arc;

use crate::context::PluginContext;
use crate::error::{PluginError, PluginResult};
use crate::plugin::Plugin;
use crate::registry::ToolRegistry;
use crate::tool::Tool;

/// Type alias for a plugin initialization callback.
pub type InitCallback = Box<dyn FnMut(&PluginContext) -> Result<(), PluginError> + Send + Sync>;

/// Type alias for a plugin shutdown callback.
pub type ShutdownCallback = Box<dyn FnMut() -> Result<(), PluginError> + Send + Sync>;

/// A minimal [`Plugin`] implementation for straightforward third-party plugins.
///
/// `SimplePlugin` stores a name, version, and a set of [`Tool`] trait objects.
/// It implements the full plugin lifecycle with no-op init/shutdown unless
/// custom callbacks are provided.
///
/// # Example
///
/// ```ignore
/// use clawdius_plugin_sdk::prelude::*;
/// use std::sync::Arc;
///
/// let mut plugin = SimplePlugin::new("my-plugin", "0.1.0")
///     .with_tool(Arc::new(MyTool));
/// ```
pub struct SimplePlugin {
    name: String,
    version: String,
    tools: Vec<Arc<dyn Tool>>,
    on_init: Option<InitCallback>,
    on_shutdown: Option<ShutdownCallback>,
}

impl SimplePlugin {
    /// Creates a new `SimplePlugin` with the given name and version.
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: Vec::new(),
            on_init: None,
            on_shutdown: None,
        }
    }

    /// Adds a [`Tool`] to this plugin.
    #[must_use]
    pub fn with_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Adds multiple [`Tool`]s to this plugin.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Sets a custom initialization callback.
    #[must_use]
    pub fn on_init<F>(mut self, f: F) -> Self
    where
        F: FnMut(&PluginContext) -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.on_init = Some(Box::new(f));
        self
    }

    /// Sets a custom shutdown callback.
    #[must_use]
    pub fn on_shutdown<F>(mut self, f: F) -> Self
    where
        F: FnMut() -> Result<(), PluginError> + Send + Sync + 'static,
    {
        self.on_shutdown = Some(Box::new(f));
        self
    }

    /// Returns the number of tools registered with this plugin.
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Returns the tool names registered with this plugin.
    #[must_use]
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }
}

impl Plugin for SimplePlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn init(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        if let Some(ref mut cb) = self.on_init {
            cb(ctx)?;
        }
        Ok(())
    }

    fn register_tools(&self, registry: &mut ToolRegistry) -> PluginResult<()> {
        let mut seen: HashMap<&str, bool> = HashMap::new();
        for tool in &self.tools {
            if seen.contains_key(tool.name()) {
                return Err(PluginError::registration_failed(format!(
                    "duplicate tool '{}' in plugin '{}'",
                    tool.name(),
                    self.name
                )));
            }
            seen.insert(tool.name(), true);
            registry
                .register_tool(Arc::clone(tool))
                .map_err(|e| PluginError::registration_failed(e.to_string()))?;
        }
        Ok(())
    }

    fn shutdown(&mut self) -> PluginResult<()> {
        if let Some(ref mut cb) = self.on_shutdown {
            cb()?;
        }
        Ok(())
    }
}
