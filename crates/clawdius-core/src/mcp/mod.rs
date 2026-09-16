//! Model Context Protocol (MCP) Implementation
//!
//! MCP is a protocol for connecting AI models to external tools and resources.

// Unwrap purge batch 1: execution-surface module — production code must not
// unwrap/expect; propagate, use invariant-expect with a written INVARIANT
// argument, or restructure.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

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
