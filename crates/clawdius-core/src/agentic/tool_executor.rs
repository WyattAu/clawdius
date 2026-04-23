//! Tool Executor Interface
//!
//! This module provides the trait for executing tools from the agentic system.
//! Includes `ShellToolExecutor` for real command execution via tokio.

use crate::error::Result;
use crate::sandbox::{executor::SandboxExecutor, tiers::SandboxConfig, SandboxTier};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// A tool execution request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    /// Tool name
    pub name: String,
    /// Tool arguments
    pub arguments: HashMap<String, serde_json::Value>,
}

impl ToolRequest {
    /// Creates a new tool request.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: HashMap::new(),
        }
    }

    /// Adds an argument to the request.
    #[must_use]
    pub fn with_arg(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.arguments.insert(key.into(), value);
        self
    }

    /// Creates a request with string arguments.
    #[must_use]
    pub fn with_string_args(name: impl Into<String>, args: HashMap<String, String>) -> Self {
        Self {
            name: name.into(),
            arguments: args
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        }
    }
}

/// A tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution succeeded
    pub success: bool,
    /// Result content
    pub content: String,
    /// Whether this is an error result
    pub is_error: bool,
}

impl ToolResult {
    /// Creates a successful result.
    #[must_use]
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            is_error: false,
        }
    }

    /// Creates an error result.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: message.into(),
            is_error: true,
        }
    }

    /// Parses the content as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid JSON.
    pub fn parse_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_str(&self.content).map_err(|e| {
            crate::error::Error::ParseError(format!("Failed to parse tool result as JSON: {}", e))
        })
    }
}

/// Trait for executing tools from the agentic system.
///
/// This trait is implemented by the main crate using the MCP host,
/// allowing the executor agent to call tools without knowing the
/// implementation details.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool by name with the given arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails.
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult>;

    /// Check if a tool exists.
    fn has_tool(&self, name: &str) -> bool;

    /// List available tools.
    fn list_tools(&self) -> Vec<ToolDefinition>;
}

/// Definition of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Creates a new tool definition.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Sets the input schema.
    #[must_use]
    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = schema;
        self
    }
}

/// A no-op tool executor for testing and when no tools are available.
pub struct NoOpToolExecutor;

#[async_trait]
impl ToolExecutor for NoOpToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult> {
        Ok(ToolResult::success(format!(
            "No-op executor: tool '{}' called with {:?}",
            request.name, request.arguments
        )))
    }

    fn has_tool(&self, _name: &str) -> bool {
        false
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// ShellToolExecutor — real command execution for sprint Build/Test phases
// ---------------------------------------------------------------------------

/// Commands that are blocked for safety.
const BLOCKED_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "mkfs",
    "dd",
    "shred",
    "wipe",
    "chmod",
    "chown",
    "chgrp",
    "sudo",
    "su",
    "doas",
    "run0",
    "kill",
    "killall",
    "pkill",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "passwd",
    "useradd",
    "userdel",
    "usermod",
    "crontab",
    "at",
    "batch",
    "iptables",
    "nft",
    "ufw",
    "firewalld",
    "mount",
    "umount",
    "nc",
    "ncat",
    "socat",
];

fn is_command_blocked(command: &str) -> bool {
    // Extract first word (the base command) to handle "sudo apt install"
    let base = command.split_whitespace().next().unwrap_or(command);
    let base = std::path::Path::new(base)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(base);
    BLOCKED_COMMANDS.contains(&base)
}

/// A real shell command executor that implements [`ToolExecutor`].
///
/// Used by [`SprintEngine`](crate::agentic::sprint::SprintEngine) to run
/// build and test commands during sprint phases. Executes commands
/// asynchronously via `tokio::process::Command` with timeout, working
/// directory, output capture, and a safety blocklist.
///
/// When a sandbox tier is configured (via [`with_sandbox_tier`] or
/// [`with_sandbox_executor`]), all shell commands are routed through the
/// sandbox backend (bubblewrap on Linux, container, or filtered fallback).
pub struct ShellToolExecutor {
    /// Working directory for command execution.
    working_dir: PathBuf,
    /// Command timeout.
    timeout: Duration,
    /// Maximum output bytes (stdout + stderr combined).
    max_output_bytes: usize,
    /// Optional sandbox executor for isolated command execution.
    sandbox: Option<Arc<SandboxExecutor>>,
}

impl ShellToolExecutor {
    /// Creates a new executor with the given working directory.
    ///
    /// Uses `SandboxTier::Trusted` by default (blocklist only, no real
    /// isolation). Call [`with_sandbox_tier`] to upgrade to real sandboxing.
    #[must_use]
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            timeout: Duration::from_secs(120),
            max_output_bytes: 512 * 1024, // 512 KB
            sandbox: None,
        }
    }

    /// Sets the command timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the maximum output bytes.
    #[must_use]
    pub fn with_max_output(mut self, max_bytes: usize) -> Self {
        self.max_output_bytes = max_bytes;
        self
    }

    /// Configure sandbox isolation at the given tier.
    ///
    /// Uses [`SandboxExecutor::new_with_fallback`] which cascades through
    /// available backends (container > bubblewrap > filtered) and never
    /// falls back to zero-isolation `direct` execution.
    ///
    /// # Errors
    ///
    /// This method cannot fail — it always uses `new_with_fallback` which
    /// degrades gracefully to filtered execution if no backend is available.
    #[must_use]
    pub fn with_sandbox_tier(mut self, tier: SandboxTier) -> Self {
        let config = SandboxConfig {
            tier,
            network: !matches!(tier, SandboxTier::Hardened),
            mounts: vec![],
        };
        self.sandbox = Some(Arc::new(SandboxExecutor::new_with_fallback(tier, config)));
        self
    }

    /// Inject a pre-built sandbox executor.
    #[must_use]
    pub fn with_sandbox_executor(mut self, executor: Arc<SandboxExecutor>) -> Self {
        self.sandbox = Some(executor);
        self
    }

    /// Returns the name of the active sandbox backend, or `"none"` if
    /// no sandbox is configured.
    #[must_use]
    pub fn sandbox_backend_name(&self) -> &'static str {
        match &self.sandbox {
            Some(exec) => exec.backend_name(),
            None => "none (direct execution with blocklist only)",
        }
    }

    fn truncate_output(&self, output: &str) -> String {
        if output.len() > self.max_output_bytes {
            let end = self.max_output_bytes;
            format!(
                "{}\n... [truncated at {} bytes, total {}]",
                &output[..end],
                end,
                output.len()
            )
        } else {
            output.to_string()
        }
    }

    async fn run_shell_command(&self, command: &str) -> Result<ToolResult> {
        // Safety check
        if is_command_blocked(command) {
            return Ok(ToolResult::error(format!(
                "Command '{}' is blocked for safety.",
                command
            )));
        }

        // Route through sandbox if configured, otherwise use direct execution.
        let output: Result<std::process::Output> = if let Some(sandbox) = &self.sandbox {
            let (shell, flag) = if cfg!(target_os = "windows") {
                ("cmd", "/C")
            } else {
                ("sh", "-c")
            };

            match tokio::time::timeout(
                self.timeout,
                sandbox.execute_async(shell, &[flag, command], &self.working_dir),
            )
            .await
            {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(e)) => Err(e),
                Err(_) => {
                    // Timeout — return as a successful ToolResult with error content
                    return Ok(ToolResult::error(format!(
                        "Command timed out after {} seconds",
                        self.timeout.as_secs()
                    )));
                },
            }
        } else {
            let (shell, flag) = if cfg!(target_os = "windows") {
                ("cmd", "/C")
            } else {
                ("sh", "-c")
            };

            match tokio::time::timeout(
                self.timeout,
                tokio::process::Command::new(shell)
                    .arg(flag)
                    .arg(command)
                    .current_dir(&self.working_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output(),
            )
            .await
            {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(e)) => Err(crate::error::Error::Io(e)),
                Err(_) => {
                    return Ok(ToolResult::error(format!(
                        "Command timed out after {} seconds",
                        self.timeout.as_secs()
                    )));
                },
            }
        };

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                let success = exit_code == 0;

                let stdout = self.truncate_output(&stdout);
                let stderr = self.truncate_output(&stderr);

                let content = if success {
                    if stderr.is_empty() {
                        stdout
                    } else {
                        format!("{stdout}\n[stderr]\n{stderr}")
                    }
                } else {
                    format!("Command exited with code {exit_code}\n{stdout}\n[stderr]\n{stderr}")
                };

                Ok(ToolResult {
                    success,
                    content,
                    is_error: !success,
                })
            },
            Err(e) => Ok(ToolResult::error(format!("Command failed to execute: {e}"))),
        }
    }
}

impl Default for ShellToolExecutor {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

#[async_trait]
impl ToolExecutor for ShellToolExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult> {
        match request.name.as_str() {
            "shell" => {
                let command = request
                    .arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::Error::Tool("Missing 'command' argument for shell tool".to_string())
                    })?;

                self.run_shell_command(command).await
            },
            _ => Ok(ToolResult::error(format!(
                "Unknown tool: '{}'. ShellToolExecutor only supports 'shell'.",
                request.name
            ))),
        }
    }

    fn has_tool(&self, name: &str) -> bool {
        name == "shell"
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition::new(
            "shell",
            "Execute a shell command. Args: command (required), cwd (optional).",
        )
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory (optional, defaults to config)"
                }
            },
            "required": ["command"]
        }))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_request_builder() {
        let request = ToolRequest::new("search_ast")
            .with_arg("node_type", serde_json::json!("function"))
            .with_arg("limit", serde_json::json!(10));

        assert_eq!(request.name, "search_ast");
        assert_eq!(request.arguments.get("node_type").unwrap(), "function");
        assert_eq!(request.arguments.get("limit").unwrap(), 10);
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("Hello, world!");
        assert!(result.success);
        assert!(!result.is_error);
        assert_eq!(result.content, "Hello, world!");
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("Something went wrong");
        assert!(!result.success);
        assert!(result.is_error);
        assert_eq!(result.content, "Something went wrong");
    }

    #[tokio::test]
    async fn test_noop_executor() {
        let executor = NoOpToolExecutor;
        let request = ToolRequest::new("test_tool");
        let result = executor.execute(request).await.unwrap();

        assert!(result.success);
        assert!(result.content.contains("test_tool"));
    }

    // --- ShellToolExecutor tests ---

    #[test]
    fn test_is_command_blocked() {
        assert!(is_command_blocked("rm"));
        assert!(is_command_blocked("/usr/bin/rm"));
        assert!(is_command_blocked("sudo apt install"));
        assert!(is_command_blocked("shutdown -h now"));
        assert!(!is_command_blocked("cargo build"));
        assert!(!is_command_blocked("npm test"));
        assert!(!is_command_blocked("echo hello"));
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("shell").with_arg(
            "command",
            serde_json::Value::String("echo hello world".to_string()),
        );
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_shell_exit_code() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("shell")
            .with_arg("command", serde_json::Value::String("exit 1".to_string()));
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
        assert!(result.is_error);
        assert!(result.content.contains("exited with code 1"));
    }

    #[tokio::test]
    async fn test_shell_blocked_command() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("shell")
            .with_arg("command", serde_json::Value::String("rm -rf /".to_string()));
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
        assert!(result.content.contains("blocked"));
    }

    #[tokio::test]
    async fn test_shell_missing_command() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("shell");
        let result = executor.execute(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_unknown_tool() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("nonexistent")
            .with_arg("command", serde_json::Value::String("echo hi".to_string()));
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
        assert!(result.content.contains("Unknown tool"));
    }

    #[test]
    fn test_shell_has_tool() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        assert!(executor.has_tool("shell"));
        assert!(!executor.has_tool("other"));
    }

    #[test]
    fn test_shell_list_tools() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let tools = executor.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "shell");
    }

    #[tokio::test]
    async fn test_shell_timeout() {
        let executor =
            ShellToolExecutor::new(std::env::temp_dir()).with_timeout(Duration::from_millis(100));
        let request = ToolRequest::new("shell")
            .with_arg("command", serde_json::Value::String("sleep 10".to_string()));
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
        assert!(result.content.contains("timed out"));
    }

    #[tokio::test]
    async fn test_shell_stderr_captured() {
        let executor = ShellToolExecutor::new(std::env::temp_dir());
        let request = ToolRequest::new("shell").with_arg(
            "command",
            serde_json::Value::String("echo err >&2".to_string()),
        );
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("[stderr]"));
        assert!(result.content.contains("err"));
    }
}
