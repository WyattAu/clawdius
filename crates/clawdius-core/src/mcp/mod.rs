//! Model Context Protocol (MCP) Implementation
//!
//! MCP is a protocol for connecting AI models to external tools and resources.

pub mod client;
pub mod handler;
pub mod protocol;
// NOTE: `sandboxed_executor` is intentionally NOT declared as a module here.
// It is a pre-existing file with unresolved imports (`crate::session::storage`)
// and was never wired into the module tree. Enable it only after those imports
// are fixed.

pub use client::{McpClient, McpClientManager, McpTransport, StdioTransport};
pub use handler::handle_mcp_request;
pub use protocol::*;
