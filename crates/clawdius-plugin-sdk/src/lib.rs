//! Clawdius Plugin SDK
//!
//! SDK for building Clawdius plugins that can be loaded as native
//! dynamic libraries or WASM modules. See the [design document]
//! for the full architecture specification.
//!
//! [design document]: https://github.com/WyattAu/clawdius/blob/main/.specs/02_architecture/plugin_sdk_design.md
//!
//! # Quick Start
//!
//! ```ignore
//! use clawdius_plugin_sdk::prelude::*;
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &str { "my-plugin" }
//!     fn version(&self) -> &str { "0.1.0" }
//!     fn init(&mut self, ctx: &PluginContext) -> Result<(), PluginError> { Ok(()) }
//!     fn register_tools(&self, registry: &mut ToolRegistry) -> Result<(), PluginError> {
//!         registry.register(ToolRegistration {
//!             name: "my_tool".into(),
//!             description: "A sample tool".into(),
//!             handler: my_tool_handler,
//!         });
//!         Ok(())
//!     }
//!     fn shutdown(&mut self) -> Result<(), PluginError> { Ok(()) }
//! }
//!
//! fn my_tool_handler(invocation: &ToolInvocation) -> ToolResult {
//!     ToolResult::ok(format!("handled: {}", invocation.name))
//! }
//! ```

pub mod context;
pub mod error;
pub mod macros;
pub mod plugin;
pub mod registry;

pub mod prelude {
    pub use crate::context::PluginContext;
    pub use crate::error::{PluginError, PluginResult};
    pub use crate::plugin::Plugin;
    pub use crate::registry::{
        ToolContent, ToolContext, ToolInvocation, ToolRegistration, ToolRegistry, ToolResult,
    };
}
