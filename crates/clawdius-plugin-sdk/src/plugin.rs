use crate::context::PluginContext;
use crate::error::PluginError;
use crate::registry::ToolRegistry;

/// Trait that all Clawdius plugins must implement.
///
/// Plugins are loaded dynamically and can register tools with the
/// central tool registry. Each plugin has a lifecycle managed by the
/// host: init -> register_tools -> (tool execution) -> shutdown.
pub trait Plugin: Send + Sync {
    /// Returns the unique name of this plugin.
    fn name(&self) -> &str;

    /// Returns the version string (semver recommended).
    fn version(&self) -> &str;

    /// Called once after the plugin is loaded.
    ///
    /// Use the provided context to discover filesystem paths for
    /// configuration and data storage.
    fn init(&mut self, ctx: &PluginContext) -> Result<(), PluginError>;

    /// Called once after init to register tools.
    ///
    /// The plugin should call `registry.register(...)` for each tool
    /// it provides.
    fn register_tools(&self, registry: &mut ToolRegistry) -> Result<(), PluginError>;

    /// Called before the plugin is unloaded.
    fn shutdown(&mut self) -> Result<(), PluginError>;
}
