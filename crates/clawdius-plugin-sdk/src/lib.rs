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
//! use std::sync::Arc;
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

#![deny(unsafe_code)]

/// Plugin initialization context and filesystem paths.
pub mod context;
/// Echo and dummy tools for testing.
pub mod echo_tool;
/// Error types for plugin operations.
pub mod error;
/// Procedural and declarative macros for plugin registration.
pub mod macros;
/// Core [`Plugin`] trait defining the plugin lifecycle.
pub mod plugin;
/// [`Tool`] trait and tool registration, invocation, and dispatch types.
pub mod registry;
/// Convenience [`SimplePlugin`] implementation.
pub mod simple;
/// Core [`Tool`] trait definition.
pub mod tool;

/// Convenience re-exports for plugin authors.
pub mod prelude {
    pub use crate::context::{PluginContext, PluginContextBuilder};
    pub use crate::echo_tool::{DummyTool, EchoTool};
    pub use crate::error::{PluginError, PluginResult};
    pub use crate::plugin::Plugin;
    pub use crate::registry::{
        PluginToolContext, ToolContent, ToolInvocation, ToolRegistration, ToolRegistry, ToolResult,
    };
    pub use crate::simple::SimplePlugin;
    pub use crate::tool::Tool;
}
