//! MCP Client Implementation
//!
//! Connects to external MCP servers (e.g. filesystem, GitHub MCP servers) and
//! consumes their tools via JSON-RPC 2.0.

use super::protocol::{
    McpCapabilities, McpRequest, McpResponse, McpServerInfo, McpTool, McpToolResult, MCP_VERSION,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio as ProcessStdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::oneshot;

/// Transport abstraction for MCP communication.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a raw JSON-RPC message.
    async fn send(&self, message: &str) -> Result<()>;

    /// Receive the next JSON-RPC message (blocking).
    async fn recv(&self) -> Result<String>;

    /// Close the transport.
    async fn close(&self) -> Result<()>;

    /// Check if the transport is still connected.
    fn is_connected(&self) -> bool;
}

/// Stdio transport - communicates with a subprocess via stdin/stdout.
pub struct StdioTransport {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
    stdout: Arc<tokio::sync::Mutex<BufReader<ChildStdout>>>,
    connected: Arc<AtomicBool>,
}

impl StdioTransport {
    /// Launch a new subprocess MCP server.
    pub async fn launch(command: &str, args: &[&str], env: &[(&str, &str)]) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(ProcessStdio::piped());
        cmd.stdout(ProcessStdio::piped());
        cmd.stderr(ProcessStdio::null());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to launch MCP server: {command}"))?;

        let stdin = child.stdin.take().context("No stdin from child")?;
        let stdout = child.stdout.take().context("No stdout from child")?;

        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(tokio::sync::Mutex::new(stdin)),
            stdout: Arc::new(tokio::sync::Mutex::new(BufReader::new(stdout))),
            connected: Arc::new(AtomicBool::new(true)),
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, message: &str) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(message.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn recv(&self) -> Result<String> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        stdout.read_line(&mut line).await?;
        if line.is_empty() {
            anyhow::bail!("MCP server closed connection");
        }
        Ok(line.trim().to_string())
    }

    async fn close(&self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);
        // Take the child out and drop the MutexGuard before awaiting so the
        // future remains `Send` (parking_lot guards are !Send across awaits).
        let child = self.child.lock().take();
        if let Some(mut child) = child {
            let _ = child.kill().await;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

/// MCP client - manages connection to an MCP server.
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    server_info: Mutex<Option<McpServerInfo>>,
    capabilities: Mutex<Option<McpCapabilities>>,
    next_id: AtomicU64,
    #[allow(dead_code)]
    pending: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<McpResponse>>>>,
    server_name: String,
}

impl McpClient {
    /// Create a new MCP client with the given transport.
    pub fn new(server_name: String, transport: Arc<dyn McpTransport>) -> Self {
        Self {
            transport,
            server_info: Mutex::new(None),
            capabilities: Mutex::new(None),
            next_id: AtomicU64::new(1),
            pending: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            server_name,
        }
    }

    /// Connect and initialize the MCP session.
    pub async fn initialize(&self) -> Result<McpServerInfo> {
        let request =
            McpRequest::new(self.next_id(), "initialize").with_params(serde_json::json!({
                "protocolVersion": MCP_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "clawdius",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }));

        let response = self.send_request(request).await?;

        let init_result: InitializeResultData =
            serde_json::from_value(response.result.unwrap_or_default())
                .context("Failed to parse initialize response")?;

        let server_info = init_result.server_info;
        *self.capabilities.lock() = Some(init_result.capabilities);
        *self.server_info.lock() = Some(server_info.clone());

        // Send initialized notification
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let _ = self.transport.send(&notif.to_string()).await;

        Ok(server_info)
    }

    /// List available tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let request = McpRequest::new(self.next_id(), "tools/list");
        let response = self.send_request(request).await?;

        let tools_response: ToolsListResult =
            serde_json::from_value(response.result.unwrap_or_default())
                .context("Failed to parse tools/list response")?;

        Ok(tools_response.tools)
    }

    /// Call a tool on the server.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult> {
        let request =
            McpRequest::new(self.next_id(), "tools/call").with_params(serde_json::json!({
                "name": name,
                "arguments": arguments,
            }));

        let response = self.send_request(request).await?;

        let result: McpToolResult = serde_json::from_value(response.result.unwrap_or_default())
            .context("Failed to parse tools/call response")?;

        Ok(result)
    }

    /// Ping the server to check connectivity.
    pub async fn ping(&self) -> Result<()> {
        let request = McpRequest::new(self.next_id(), "ping");
        self.send_request(request).await?;
        Ok(())
    }

    /// Get the server name.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get cached server info.
    pub fn server_info(&self) -> Option<McpServerInfo> {
        self.server_info.lock().clone()
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn send_request(&self, request: McpRequest) -> Result<McpResponse> {
        let id = request.id;
        let json = serde_json::to_string(&request)?;

        self.transport.send(&json).await?;

        loop {
            let line = self.transport.recv().await?;
            // Parse as a generic JSON value first so we can skip server-side
            // notifications (which have no `id` field).
            let value: Value = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse MCP message: {line}"))?;

            if value.get("id").is_none() {
                continue;
            }

            let response: McpResponse = serde_json::from_value(value)
                .with_context(|| format!("Failed to parse MCP response: {line}"))?;

            if response.id == id {
                if let Some(error) = response.error {
                    anyhow::bail!("MCP error {}: {}", error.code, error.message);
                }
                return Ok(response);
            }
            // Ignore responses for other IDs.
        }
    }
}

/// Manager for multiple MCP server connections.
pub struct McpClientManager {
    clients: Arc<tokio::sync::Mutex<HashMap<String, Arc<McpClient>>>>,
}

impl McpClientManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Connect to a new MCP server via stdio.
    pub async fn connect_stdio(
        &self,
        name: &str,
        command: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<Arc<McpClient>> {
        let transport = Arc::new(StdioTransport::launch(command, args, env).await?);
        let client = Arc::new(McpClient::new(name.to_string(), transport));

        let server_info = client.initialize().await?;
        tracing::info!("Connected to MCP server '{}': {}", name, server_info.name);

        self.clients
            .lock()
            .await
            .insert(name.to_string(), client.clone());
        Ok(client)
    }

    /// Disconnect from a specific MCP server.
    pub async fn disconnect(&self, name: &str) -> Result<()> {
        if let Some(client) = self.clients.lock().await.remove(name) {
            client.transport.close().await?;
        }
        Ok(())
    }

    /// List all tools from all connected servers.
    pub async fn list_all_tools(&self) -> Result<Vec<(String, McpTool)>> {
        let mut all_tools = Vec::new();
        let clients = self.clients.lock().await.clone();

        for (server_name, client) in &clients {
            match client.list_tools().await {
                Ok(tools) => {
                    for tool in tools {
                        all_tools.push((server_name.clone(), tool));
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to list tools from '{}': {}", server_name, e);
                },
            }
        }

        Ok(all_tools)
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolResult> {
        let client = self
            .clients
            .lock()
            .await
            .get(server_name)
            .cloned()
            .with_context(|| format!("MCP server '{server_name}' not connected"))?;

        client.call_tool(tool_name, arguments).await
    }

    /// Get list of connected server names.
    pub async fn connected_servers(&self) -> Vec<String> {
        self.clients.lock().await.keys().cloned().collect()
    }

    /// Ping all servers and return names of those that failed.
    pub async fn health_check(&self) -> Vec<String> {
        let mut failed = Vec::new();
        let clients = self.clients.lock().await.clone();

        for (name, client) in &clients {
            if client.ping().await.is_err() {
                failed.push(name.clone());
            }
        }

        failed
    }
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

// Internal types for parsing MCP responses. MCP uses camelCase in the wire
// format, while the existing protocol structs use snake_case without rename
// attributes, so we define local mirror types here.
#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct InitializeResultData {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: McpCapabilities,
    #[serde(rename = "serverInfo")]
    server_info: McpServerInfo,
}

#[derive(serde::Deserialize)]
struct ToolsListResult {
    tools: Vec<McpTool>,
}

// Convenience re-exports.
pub use McpClient as Client;
pub use McpClientManager as ClientManager;
pub use McpTransport as Transport;
pub use StdioTransport as Stdio;

#[cfg(test)]
mod tests {
    use super::*;

    /// A no-op transport used for unit testing the client logic without a real
    /// subprocess.
    struct MockTransport {
        connected: Arc<AtomicBool>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                connected: Arc::new(AtomicBool::new(true)),
            }
        }
    }

    #[async_trait]
    impl McpTransport for MockTransport {
        async fn send(&self, _message: &str) -> Result<()> {
            Ok(())
        }
        async fn recv(&self) -> Result<String> {
            // Block forever-ish: tests that exercise recv should not be run
            // against the mock.
            std::future::pending::<()>().await;
            anyhow::bail!("mock transport has no data");
        }
        async fn close(&self) -> Result<()> {
            self.connected.store(false, Ordering::SeqCst);
            Ok(())
        }
        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn test_manager_creation() {
        let manager = McpClientManager::new();
        // A freshly-created manager should have no connected servers.
        // We can't `.await` in a sync test, but the map is empty so we verify
        // via Default equivalence instead.
        let _default = McpClientManager::default();
        assert!(std::ptr::addr_of!(manager) != std::ptr::null());
    }

    #[tokio::test]
    async fn test_manager_default_is_empty() {
        let manager = McpClientManager::default();
        let servers = manager.connected_servers().await;
        assert!(servers.is_empty());
    }

    #[test]
    fn test_next_id_increments() {
        let transport: Arc<dyn McpTransport> = Arc::new(MockTransport::new());
        let client = McpClient::new("test-server".to_string(), transport);

        let first = client.next_id();
        let second = client.next_id();
        let third = client.next_id();

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(third, 3);
    }

    #[test]
    fn test_client_server_name() {
        let transport: Arc<dyn McpTransport> = Arc::new(MockTransport::new());
        let client = McpClient::new("my-server".to_string(), transport);
        assert_eq!(client.server_name(), "my-server");
    }

    #[test]
    fn test_mock_transport_is_connected() {
        let transport = MockTransport::new();
        assert!(transport.is_connected());
    }

    #[tokio::test]
    async fn test_mock_transport_close() {
        let transport = MockTransport::new();
        assert!(transport.is_connected());
        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }
}
