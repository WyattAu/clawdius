//! Model Context Protocol (MCP) Implementation
//!
//! MCP is a protocol for connecting AI models to external tools and resources.

pub mod handler;
pub mod protocol;

pub use handler::handle_mcp_request;
pub use protocol::{
    McpCapabilities, McpContent, McpError, McpMessage, McpPrompt, McpRequest, McpResource,
    McpResponse, McpServerInfo, McpTool, McpToolResult,
};
