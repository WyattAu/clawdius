//! MCP Host - Tool registry and execution

use std::collections::HashMap;
use std::fmt;

use crate::mcp::tools::{builtin_tools, McpTool};
use crate::mcp::types::{
    McpError, ServerCapabilities, ServerInfo, ToolDefinition, ToolRequest, ToolResponse,
};

/// MCP Host for managing tools
pub struct McpHost {
    /// Registered tools
    tools: HashMap<String, Box<dyn McpTool>>,
    /// Server info
    server_info: ServerInfo,
    /// Server capabilities
    capabilities: ServerCapabilities,
}

impl fmt::Debug for McpHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpHost")
            .field("tool_count", &self.tools.len())
            .field("server_info", &self.server_info)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

impl McpHost {
    /// Create a new MCP host
    #[must_use]
    pub fn new() -> Self {
        let mut host = Self {
            tools: HashMap::new(),
            server_info: ServerInfo::new("clawdius-graph-rag", env!("CARGO_PKG_VERSION")),
            capabilities: ServerCapabilities {
                tools: Some(crate::mcp::types::ToolsCapability {
                    list_changed: false,
                }),
                resources: None,
                prompts: None,
            },
        };

        for tool in builtin_tools() {
            host.register_tool(tool);
        }

        host
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: Box<dyn McpTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// List all registered tools
    #[must_use]
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition::new(t.name(), t.description(), t.schema()))
            .collect()
    }

    /// Get server info
    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Get server capabilities
    #[must_use]
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Call a tool by name
    pub fn call_tool(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let tool = self
            .tools
            .get(&request.name)
            .ok_or_else(|| McpError::ToolNotFound(request.name.clone()))?;

        tool.execute(request)
    }

    /// Check if a tool exists
    #[must_use]
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get tool count
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for McpHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_host_creation() {
        let host = McpHost::new();
        assert!(host.tool_count() > 0);
    }

    #[test]
    fn test_list_tools() {
        let host = McpHost::new();
        let tools = host.list_tools();
        assert!(!tools.is_empty());
    }

    #[test]
    fn test_call_tool_not_found() {
        let host = McpHost::new();
        let request = ToolRequest {
            name: "nonexistent".into(),
            arguments: HashMap::new(),
        };

        let result = host.call_tool(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_tool() {
        let host = McpHost::new();
        assert!(host.has_tool("search_ast"));
        assert!(!host.has_tool("nonexistent"));
    }
}
