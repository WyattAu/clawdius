//! MCP (Model Context Protocol) integration
//!
//! Provides MCP protocol support for AI assistant integration.

mod host;
mod tools;
mod types;

pub use host::McpHost;
pub use tools::McpTool;
pub use types::*;
