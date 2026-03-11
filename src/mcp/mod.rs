//! MCP (Model Context Protocol) integration
//!
//! Provides MCP protocol support for AI assistant integration.
//!
//! # Components
//!
//! - **Host**: Tool registry and execution for built-in tools
//! - **Server**: JSON-RPC server for external tool connections
//! - **Client**: Connect to external MCP servers
//! - **Types**: Protocol types (ToolDefinition, ToolRequest, etc.)
//!
//! # Example
//!
//! ```no_run
//! use clawdius::mcp::{McpServer, McpClientConfig, McpClientManager};
//!
//! // Start an MCP server
//! let server = McpServer::new();
//! server.run_stdio().unwrap();
//!
//! // Or connect to external servers
//! let mut manager = McpClientManager::new();
//! manager.add_client(McpClientConfig::new("filesystem", "mcp-filesystem")).unwrap();
//! ```

mod client;
mod host;
mod server;
mod tools;
mod types;

pub use client::{ConnectedServer, McpClient, McpClientConfig, McpClientManager};
pub use host::McpHost;
pub use server::{
    JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, McpServer, PromptArgument,
    PromptDefinition,
};
pub use tools::McpTool;
pub use types::*;
