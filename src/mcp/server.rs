//! MCP Server - JSON-RPC server for MCP protocol
//!
//! Implements the MCP server that allows external tools to connect to Clawdius
//! and use its tools, resources, and prompts.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::mcp::types::{
    ContentBlock, McpError, ProgressNotification, PromptsCapability, ResourceContent,
    ResourceDefinition, ResourceSubscription, ResourcesCapability, SamplingContent,
    SamplingMessage, SamplingRequest, SamplingResponse, ServerCapabilities, ServerInfo,
    ToolDefinition, ToolRequest, ToolResponse, MCP_VERSION,
};
use crate::mcp::{McpHost, McpTool};

/// MCP Server
pub struct McpServer {
    /// Host for tool management
    host: McpHost,
    /// Server info
    info: ServerInfo,
    /// Server capabilities
    capabilities: ServerCapabilities,
    /// Registered resources
    resources: HashMap<String, ResourceDefinition>,
    /// Registered prompts
    prompts: HashMap<String, PromptDefinition>,
    /// Resource handlers
    resource_handlers:
        HashMap<String, Box<dyn Fn() -> Result<ResourceContent, McpError> + Send + Sync>>,
    /// Prompt handlers
    prompt_handlers: HashMap<
        String,
        Box<dyn Fn(HashMap<String, String>) -> Result<String, McpError> + Send + Sync>,
    >,
}

/// Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    /// Prompt name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Prompt arguments
    pub arguments: Vec<PromptArgument>,
}

/// Prompt argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: Option<String>,
    /// Whether required
    pub required: bool,
}

impl McpServer {
    /// Create a new MCP server
    #[must_use]
    pub fn new() -> Self {
        let mut server = Self {
            host: McpHost::new(),
            info: ServerInfo::new("clawdius", env!("CARGO_PKG_VERSION")),
            capabilities: ServerCapabilities {
                tools: Some(crate::mcp::types::ToolsCapability { list_changed: true }),
                resources: Some(ResourcesCapability {
                    subscribe: false,
                    list_changed: true,
                }),
                prompts: Some(PromptsCapability { list_changed: true }),
            },
            resources: HashMap::new(),
            prompts: HashMap::new(),
            resource_handlers: HashMap::new(),
            prompt_handlers: HashMap::new(),
        };

        // Register built-in prompts
        server.register_builtin_prompts();
        server.register_builtin_resources();

        server
    }

    /// Register a custom tool
    pub fn register_tool(&mut self, tool: Box<dyn McpTool>) {
        self.host.register_tool(tool);
    }

    /// Register a resource
    pub fn register_resource<F>(&mut self, def: ResourceDefinition, handler: F)
    where
        F: Fn() -> Result<ResourceContent, McpError> + Send + Sync + 'static,
    {
        let uri = def.uri.clone();
        self.resources.insert(uri.clone(), def);
        self.resource_handlers.insert(uri, Box::new(handler));
    }

    /// Register a prompt
    pub fn register_prompt<F>(&mut self, def: PromptDefinition, handler: F)
    where
        F: Fn(HashMap<String, String>) -> Result<String, McpError> + Send + Sync + 'static,
    {
        let name = def.name.clone();
        self.prompts.insert(name.clone(), def);
        self.prompt_handlers.insert(name, Box::new(handler));
    }

    /// Register built-in prompts
    fn register_builtin_prompts(&mut self) {
        // Code review prompt
        self.register_prompt(
            PromptDefinition {
                name: "code_review".into(),
                description: "Generate a code review for the given code".into(),
                arguments: vec![
                    PromptArgument {
                        name: "code".into(),
                        description: Some("Code to review".into()),
                        required: true,
                    },
                    PromptArgument {
                        name: "focus".into(),
                        description: Some("Focus area (security, performance, style)".into()),
                        required: false,
                    },
                ],
            },
            |args| {
                let code = args.get("code").cloned().unwrap_or_default();
                let focus = args
                    .get("focus")
                    .cloned()
                    .unwrap_or_else(|| "general".into());
                Ok(format!(
                    "Please review the following code with a focus on {}:\n\n```{}\n{}\n```",
                    focus, focus, code
                ))
            },
        );

        // Test generation prompt
        self.register_prompt(
            PromptDefinition {
                name: "generate_tests".into(),
                description: "Generate unit tests for the given code".into(),
                arguments: vec![PromptArgument {
                    name: "code".into(),
                    description: Some("Code to generate tests for".into()),
                    required: true,
                }],
            },
            |args| {
                let code = args.get("code").cloned().unwrap_or_default();
                Ok(format!(
                    "Generate comprehensive unit tests for the following code:\n\n```\n{}\n```",
                    code
                ))
            },
        );

        // Refactor prompt
        self.register_prompt(
            PromptDefinition {
                name: "refactor".into(),
                description: "Suggest refactoring improvements".into(),
                arguments: vec![
                    PromptArgument {
                        name: "code".into(),
                        description: Some("Code to refactor".into()),
                        required: true,
                    },
                    PromptArgument {
                        name: "goal".into(),
                        description: Some(
                            "Refactoring goal (readability, performance, maintainability)".into(),
                        ),
                        required: false,
                    },
                ],
            },
            |args| {
                let code = args.get("code").cloned().unwrap_or_default();
                let goal = args
                    .get("goal")
                    .cloned()
                    .unwrap_or_else(|| "readability".into());
                Ok(format!(
                    "Refactor the following code to improve {}:\n\n```\n{}\n```",
                    goal, code
                ))
            },
        );
    }

    /// Register built-in resources
    fn register_builtin_resources(&mut self) {
        // Project context resource
        self.register_resource(
            ResourceDefinition {
                uri: "clawdius://context/project".into(),
                name: "Project Context".into(),
                description: Some("Current project context and configuration".into()),
                mime_type: Some("application/json".into()),
            },
            || {
                Ok(ResourceContent {
                    uri: "clawdius://context/project".into(),
                    mime_type: Some("application/json".into()),
                    text: Some(
                        serde_json::json!({
                            "name": "clawdius-project",
                            "version": env!("CARGO_PKG_VERSION"),
                        })
                        .to_string(),
                    ),
                    blob: None,
                })
            },
        );

        // Available modes resource
        self.register_resource(
            ResourceDefinition {
                uri: "clawdius://modes".into(),
                name: "Available Modes".into(),
                description: Some("List of available Clawdius modes".into()),
                mime_type: Some("application/json".into()),
            },
            || {
                Ok(ResourceContent {
                    uri: "clawdius://modes".into(),
                    mime_type: Some("application/json".into()),
                    text: Some(
                        serde_json::json!({
                            "modes": ["code", "architect", "debug", "review", "refactor"]
                        })
                        .to_string(),
                    ),
                    blob: None,
                })
            },
        );
    }

    /// Get server info
    #[must_use]
    pub fn info(&self) -> &ServerInfo {
        &self.info
    }

    /// Get capabilities
    #[must_use]
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// List tools
    #[must_use]
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.host.list_tools()
    }

    /// List resources
    #[must_use]
    pub fn list_resources(&self) -> Vec<&ResourceDefinition> {
        self.resources.values().collect()
    }

    /// List prompts
    #[must_use]
    pub fn list_prompts(&self) -> Vec<&PromptDefinition> {
        self.prompts.values().collect()
    }

    /// Call a tool
    pub fn call_tool(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        self.host.call_tool(request)
    }

    /// Read a resource
    pub fn read_resource(&self, uri: &str) -> Result<ResourceContent, McpError> {
        self.resource_handlers
            .get(uri)
            .ok_or_else(|| McpError::ResourceNotFound(uri.into()))?()
    }

    /// Get a prompt
    pub fn get_prompt(
        &self,
        name: &str,
        args: HashMap<String, String>,
    ) -> Result<String, McpError> {
        self.prompt_handlers
            .get(name)
            .ok_or_else(|| McpError::ResourceNotFound(name.into()))?(args)
    }

    /// Handle a JSON-RPC request
    pub fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(request.params),
            "resources/subscribe" => self.handle_resources_subscribe(request.params),
            "resources/unsubscribe" => self.handle_resources_unsubscribe(request.params),
            "prompts/list" => self.handle_prompts_list(),
            "prompts/get" => self.handle_prompts_get(request.params),
            "sampling/createMessage" => self.handle_sampling(request.params),
            "notifications/progress" => self.handle_progress(request.params),
            "notifications/cancelled" => self.handle_cancelled(request.params),
            "notifications/resources/updated" => self.handle_resource_updated(request.params),
            _ => return JsonRpcResponse::error(request.id, -32601, "Method not found"),
        };

        match result {
            Ok(value) => JsonRpcResponse::success(request.id, value),
            Err(e) => JsonRpcResponse::error(request.id, -32603, &e.to_string()),
        }
    }

    /// Handle resource subscription
    fn handle_resources_subscribe(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;
        let uri = params["uri"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing uri".into()))?;

        // Check if resource exists
        if !self.resources.contains_key(uri) {
            return Err(McpError::ResourceNotFound(uri.into()));
        }

        // In a full implementation, we would track subscriptions per client
        // For now, acknowledge the subscription
        Ok(serde_json::json!({
            "subscribed": true,
            "uri": uri
        }))
    }

    /// Handle resource unsubscription
    fn handle_resources_unsubscribe(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;
        let uri = params["uri"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing uri".into()))?;

        Ok(serde_json::json!({
            "unsubscribed": true,
            "uri": uri
        }))
    }

    /// Handle sampling request (LLM completion)
    fn handle_sampling(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let _params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;

        // Sampling is a request from the server to the client
        // The server cannot fulfill this itself - it should be forwarded to the client
        // Return a not-implemented response
        Err(McpError::Internal(
            "Sampling requests should be sent to the client, not handled by the server".into(),
        ))
    }

    /// Handle progress notification
    fn handle_progress(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;

        // Log progress for monitoring
        tracing::info!(
            progress_token = %params["progressToken"],
            progress = %params["progress"],
            "Progress notification received"
        );

        // Acknowledge the notification
        Ok(serde_json::json!({ "received": true }))
    }

    /// Handle cancellation notification
    fn handle_cancelled(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;

        tracing::info!(
            request_id = %params["requestId"],
            reason = %params["reason"].as_str().unwrap_or("no reason provided"),
            "Cancellation notification received"
        );

        // In a full implementation, we would cancel the running operation
        Ok(serde_json::json!({ "cancelled": true }))
    }

    /// Handle resource updated notification
    fn handle_resource_updated(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;

        tracing::info!(
            uri = %params["uri"].as_str().unwrap_or("unknown"),
            "Resource updated notification received"
        );

        Ok(serde_json::json!({ "acknowledged": true }))
    }

    fn handle_initialize(
        &self,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        Ok(serde_json::json!({
            "protocolVersion": MCP_VERSION,
            "capabilities": self.capabilities,
            "serverInfo": self.info
        }))
    }

    fn handle_tools_list(&self) -> Result<serde_json::Value, McpError> {
        Ok(serde_json::json!({
            "tools": self.list_tools()
        }))
    }

    fn handle_tools_call(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;
        let request: ToolRequest = serde_json::from_value(params)
            .map_err(|e| McpError::InvalidArguments(e.to_string()))?;
        let response = self.call_tool(request)?;
        Ok(serde_json::to_value(response).unwrap_or_default())
    }

    fn handle_resources_list(&self) -> Result<serde_json::Value, McpError> {
        Ok(serde_json::json!({
            "resources": self.list_resources()
        }))
    }

    fn handle_resources_read(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;
        let uri = params["uri"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing uri".into()))?;
        let content = self.read_resource(uri)?;
        Ok(serde_json::json!({
            "contents": [content]
        }))
    }

    fn handle_prompts_list(&self) -> Result<serde_json::Value, McpError> {
        Ok(serde_json::json!({
            "prompts": self.list_prompts()
        }))
    }

    fn handle_prompts_get(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McpError> {
        let params = params.ok_or_else(|| McpError::InvalidArguments("Missing params".into()))?;
        let name = params["name"]
            .as_str()
            .ok_or_else(|| McpError::InvalidArguments("Missing name".into()))?;
        let args: HashMap<String, String> = params["arguments"]
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        let prompt = self.get_prompt(name, args)?;
        Ok(serde_json::json!({
            "description": self.prompts.get(name).map(|p| p.description.as_str()).unwrap_or(""),
            "messages": [{
                "role": "user",
                "content": {
                    "type": "text",
                    "text": prompt
                }
            }]
        }))
    }

    /// Run the server using stdio
    pub fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonRpcRequest>(line) {
                Ok(request) => {
                    let response = self.handle_request(request);
                    let response_json = serde_json::to_string(&response)?;
                    writeln!(stdout, "{}", response_json)?;
                    stdout.flush()?;
                }
                Err(e) => {
                    let response = JsonRpcResponse::error(
                        JsonRpcId::Null,
                        -32700,
                        &format!("Parse error: {}", e),
                    );
                    let response_json = serde_json::to_string(&response)?;
                    writeln!(stdout, "{}", response_json)?;
                    stdout.flush()?;
                }
            }
        }

        Ok(())
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON-RPC Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID
    pub id: JsonRpcId,
    /// Method name
    pub method: String,
    /// Method parameters
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Request ID
    pub id: JsonRpcId,
    /// Result (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a successful response
    #[must_use]
    pub fn success(id: JsonRpcId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    #[must_use]
    pub fn error(id: JsonRpcId, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// JSON-RPC Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC ID
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    /// Null ID
    Null,
    /// String ID
    String(String),
    /// Number ID
    Number(i64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = McpServer::new();
        assert!(server.list_tools().len() > 0);
        assert!(server.list_resources().len() > 0);
        assert!(server.list_prompts().len() > 0);
    }

    #[test]
    fn test_initialize() {
        let server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::Number(1),
            method: "initialize".into(),
            params: None,
        };
        let response = server.handle_request(request);
        assert!(response.result.is_some());
    }

    #[test]
    fn test_list_tools() {
        let server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::Number(2),
            method: "tools/list".into(),
            params: None,
        };
        let response = server.handle_request(request);
        assert!(response.result.is_some());
    }

    #[test]
    fn test_list_prompts() {
        let server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::Number(3),
            method: "prompts/list".into(),
            params: None,
        };
        let response = server.handle_request(request);
        assert!(response.result.is_some());
    }

    #[test]
    fn test_get_prompt() {
        let server = McpServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: JsonRpcId::Number(4),
            method: "prompts/get".into(),
            params: Some(serde_json::json!({
                "name": "code_review",
                "arguments": {
                    "code": "fn main() {}",
                    "focus": "security"
                }
            })),
        };
        let response = server.handle_request(request);
        assert!(response.result.is_some());
    }
}
