//! MCP Client - Connect to external MCP servers
//!
//! Implements an MCP client that can connect to external MCP servers
//! and use their tools, resources, and prompts.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::process::{Child, Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::mcp::server::{JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::mcp::types::{
    McpError, ResourceContent, ResourceDefinition, ServerCapabilities, ServerInfo, ToolDefinition,
    ToolRequest, ToolResponse,
};

/// MCP Client configuration
#[derive(Debug, Clone)]
pub struct McpClientConfig {
    /// Server name (for identification)
    pub name: String,
    /// Command to start the server
    pub command: String,
    /// Arguments for the command
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
}

impl McpClientConfig {
    /// Create a new client config
    #[must_use]
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Add an argument
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add an environment variable
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Connected MCP server info
#[derive(Debug, Clone)]
pub struct ConnectedServer {
    /// Server name
    pub name: String,
    /// Server info
    pub info: ServerInfo,
    /// Server capabilities
    pub capabilities: ServerCapabilities,
    /// Available tools
    pub tools: Vec<ToolDefinition>,
    /// Available resources
    pub resources: Vec<ResourceDefinition>,
}

/// MCP Client for connecting to external MCP servers
pub struct McpClient {
    /// Client configuration
    config: McpClientConfig,
    /// Server process (if spawned)
    process: Option<Child>,
    /// Connected server info
    server: Option<ConnectedServer>,
    /// Request ID counter
    request_id: i64,
}

impl McpClient {
    /// Create a new MCP client
    #[must_use]
    pub fn new(config: McpClientConfig) -> Self {
        Self {
            config,
            process: None,
            server: None,
            request_id: 0,
        }
    }

    /// Connect to the MCP server
    pub fn connect(&mut self) -> Result<(), McpError> {
        // Start the server process
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut process = cmd
            .spawn()
            .map_err(|e| McpError::Internal(format!("Failed to start server: {}", e)))?;

        // Initialize connection
        let response = self.send_request(
            &mut process,
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024.11",
                "clientInfo": {
                    "name": "clawdius",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        )?;

        let result = response
            .result
            .ok_or_else(|| McpError::Internal("Initialize failed: no result".into()))?;

        let info: ServerInfo = serde_json::from_value(result["serverInfo"].clone())
            .map_err(|e| McpError::Internal(format!("Invalid server info: {}", e)))?;

        let capabilities: ServerCapabilities =
            serde_json::from_value(result["capabilities"].clone()).unwrap_or_default();

        // Fetch tools, resources
        let tools = self.fetch_tools(&mut process)?;
        let resources = self.fetch_resources(&mut process)?;

        self.server = Some(ConnectedServer {
            name: self.config.name.clone(),
            info,
            capabilities,
            tools,
            resources,
        });
        self.process = Some(process);

        Ok(())
    }

    /// Fetch tools from server
    fn fetch_tools(&mut self, process: &mut Child) -> Result<Vec<ToolDefinition>, McpError> {
        let response = self.send_request(process, "tools/list", None)?;
        let result = response
            .result
            .ok_or_else(|| McpError::Internal("No tools result".into()))?;

        let tools: Vec<ToolDefinition> =
            serde_json::from_value(result["tools"].clone()).unwrap_or_default();
        Ok(tools)
    }

    /// Fetch resources from server
    fn fetch_resources(
        &mut self,
        process: &mut Child,
    ) -> Result<Vec<ResourceDefinition>, McpError> {
        let response = self.send_request(process, "resources/list", None)?;
        let result = response
            .result
            .ok_or_else(|| McpError::Internal("No resources result".into()))?;

        let resources: Vec<ResourceDefinition> =
            serde_json::from_value(result["resources"].clone()).unwrap_or_default();
        Ok(resources)
    }

    /// Send a JSON-RPC request
    fn send_request(
        &mut self,
        process: &mut Child,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, McpError> {
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::Number(self.request_id),
            method: method.into(),
            params,
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| McpError::Internal(format!("Failed to serialize request: {}", e)))?;

        let stdin = process
            .stdin
            .as_mut()
            .ok_or_else(|| McpError::Internal("No stdin".into()))?;
        let stdout = process
            .stdout
            .as_mut()
            .ok_or_else(|| McpError::Internal("No stdout".into()))?;

        // Send request
        writeln!(stdin, "{}", request_json)
            .map_err(|e| McpError::Internal(format!("Failed to write request: {}", e)))?;

        // Read response
        let mut response_line = String::new();
        let mut reader = io::BufReader::new(stdout);
        reader
            .read_line(&mut response_line)
            .map_err(|e| McpError::Internal(format!("Failed to read response: {}", e)))?;

        let response: JsonRpcResponse = serde_json::from_str(&response_line)
            .map_err(|e| McpError::Internal(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response.error {
            return Err(McpError::ExecutionError(error.message));
        }

        Ok(response)
    }

    /// Get connected server info
    #[must_use]
    pub fn server(&self) -> Option<&ConnectedServer> {
        self.server.as_ref()
    }

    /// List available tools
    #[must_use]
    pub fn list_tools(&self) -> &[ToolDefinition] {
        self.server
            .as_ref()
            .map(|s| s.tools.as_slice())
            .unwrap_or(&[])
    }

    /// List available resources
    #[must_use]
    pub fn list_resources(&self) -> &[ResourceDefinition] {
        self.server
            .as_ref()
            .map(|s| s.resources.as_slice())
            .unwrap_or(&[])
    }

    /// Call a tool
    pub fn call_tool(&mut self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let process = self
            .process
            .as_mut()
            .ok_or_else(|| McpError::Internal("Not connected".into()))?;

        let response =
            self.send_request(process, "tools/call", Some(serde_json::to_value(&request)?))?;
        let result = response
            .result
            .ok_or_else(|| McpError::Internal("No result".into()))?;

        serde_json::from_value(result)
            .map_err(|e| McpError::Internal(format!("Invalid response: {}", e)))
    }

    /// Read a resource
    pub fn read_resource(&mut self, uri: &str) -> Result<ResourceContent, McpError> {
        let process = self
            .process
            .as_mut()
            .ok_or_else(|| McpError::Internal("Not connected".into()))?;

        let response = self.send_request(
            process,
            "resources/read",
            Some(serde_json::json!({
                "uri": uri
            })),
        )?;

        let result = response
            .result
            .ok_or_else(|| McpError::Internal("No result".into()))?;
        let contents: Vec<ResourceContent> = serde_json::from_value(result["contents"].clone())
            .map_err(|e| McpError::Internal(format!("Invalid contents: {}", e)))?;

        contents
            .into_iter()
            .next()
            .ok_or_else(|| McpError::ResourceNotFound(uri.into()))
    }

    /// Disconnect from the server
    pub fn disconnect(&mut self) -> Result<(), McpError> {
        if let Some(mut process) = self.process.take() {
            process
                .kill()
                .map_err(|e| McpError::Internal(format!("Failed to kill process: {}", e)))?;
        }
        self.server = None;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

/// MCP Client Manager - manages connections to multiple MCP servers
pub struct McpClientManager {
    /// Connected clients
    clients: HashMap<String, McpClient>,
}

impl McpClientManager {
    /// Create a new client manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Add a client
    pub fn add_client(&mut self, config: McpClientConfig) -> Result<(), McpError> {
        let name = config.name.clone();
        let mut client = McpClient::new(config);
        client.connect()?;
        self.clients.insert(name, client);
        Ok(())
    }

    /// Remove a client
    pub fn remove_client(&mut self, name: &str) -> Result<(), McpError> {
        if let Some(mut client) = self.clients.remove(name) {
            client.disconnect()?;
        }
        Ok(())
    }

    /// Get a client by name
    #[must_use]
    pub fn get_client(&self, name: &str) -> Option<&McpClient> {
        self.clients.get(name)
    }

    /// Get a mutable client by name
    pub fn get_client_mut(&mut self, name: &str) -> Option<&mut McpClient> {
        self.clients.get_mut(name)
    }

    /// List all connected servers
    #[must_use]
    pub fn list_servers(&self) -> Vec<&ConnectedServer> {
        self.clients.values().filter_map(|c| c.server()).collect()
    }

    /// Call a tool on a specific server
    pub fn call_tool(
        &mut self,
        server: &str,
        request: ToolRequest,
    ) -> Result<ToolResponse, McpError> {
        self.clients
            .get_mut(server)
            .ok_or_else(|| McpError::ToolNotFound(server.into()))?
            .call_tool(request)
    }

    /// Find a tool across all servers
    #[must_use]
    pub fn find_tool(&self, tool_name: &str) -> Option<(&str, &ToolDefinition)> {
        for (server_name, client) in &self.clients {
            if let Some(server) = client.server() {
                for tool in &server.tools {
                    if tool.name == tool_name {
                        return Some((server_name, tool));
                    }
                }
            }
        }
        None
    }

    /// Call a tool by name (finds the server automatically)
    pub fn call_tool_by_name(
        &mut self,
        tool_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Result<ToolResponse, McpError> {
        let (server_name, _) = self
            .find_tool(tool_name)
            .ok_or_else(|| McpError::ToolNotFound(tool_name.into()))?;

        let request = ToolRequest {
            name: tool_name.into(),
            arguments,
        };

        self.call_tool(server_name, request)
    }
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config() {
        let config = McpClientConfig::new("test", "node")
            .arg("server.js")
            .env("DEBUG", "1");

        assert_eq!(config.name, "test");
        assert_eq!(config.command, "node");
        assert_eq!(config.args, vec!["server.js"]);
        assert_eq!(config.env.get("DEBUG"), Some(&"1".to_string()));
    }

    #[test]
    fn test_client_manager() {
        let manager = McpClientManager::new();
        assert_eq!(manager.list_servers().len(), 0);
    }
}
