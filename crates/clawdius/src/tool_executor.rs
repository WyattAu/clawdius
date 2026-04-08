use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use clawdius_core::agentic::tool_executor::{
    ToolDefinition, ToolExecutor, ToolRequest, ToolResult,
};
use clawdius_core::config::ShellSandboxConfig;
use clawdius_core::tools::file::{
    FileEditParams, FileListParams, FileReadParams, FileTool, FileWriteParams,
};
use clawdius_core::tools::git::{GitDiffParams, GitLogParams, GitTool};
use clawdius_core::tools::shell::{ShellParams, ShellTool};

pub struct CliToolExecutor {
    file_tool: Arc<FileTool>,
    shell_tool: Arc<ShellTool>,
    git_tool: Arc<GitTool>,
}

impl CliToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        let sandbox_config = ShellSandboxConfig::default();
        let file_tool = Arc::new(FileTool::with_workspace_root(&workspace_root));
        let shell_tool = Arc::new(ShellTool::new(
            sandbox_config.clone(),
            workspace_root.clone(),
        ));
        let git_tool = Arc::new(GitTool::new(sandbox_config, workspace_root));

        Self {
            file_tool,
            shell_tool,
            git_tool,
        }
    }

    fn get_string_arg(
        args: &std::collections::HashMap<String, serde_json::Value>,
        key: &str,
    ) -> Option<String> {
        args.get(key).and_then(|v| v.as_str().map(String::from))
    }

    fn get_usize_arg(
        args: &std::collections::HashMap<String, serde_json::Value>,
        key: &str,
    ) -> Option<usize> {
        args.get(key).and_then(|v| v.as_u64().map(|n| n as usize))
    }

    fn get_bool_arg(
        args: &std::collections::HashMap<String, serde_json::Value>,
        key: &str,
    ) -> bool {
        args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
    }

    fn get_u64_arg(args: &std::collections::HashMap<String, serde_json::Value>, key: &str) -> u64 {
        args.get(key).and_then(|v| v.as_u64()).unwrap_or(120_000)
    }

    fn tool_definitions() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::new(
                "read_file",
                "Read file contents with optional offset and limit",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "offset": { "type": "integer", "description": "Line offset to start reading from" },
                    "limit": { "type": "integer", "description": "Max number of lines to read" }
                },
                "required": ["path"]
            })),
            ToolDefinition::new(
                "write_file",
                "Write content to a file, creating directories as needed",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            })),
            ToolDefinition::new(
                "edit_file",
                "Replace a substring in a file with new content",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path" },
                    "old_string": { "type": "string", "description": "Text to find" },
                    "new_string": { "type": "string", "description": "Replacement text" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences" }
                },
                "required": ["path", "old_string", "new_string"]
            })),
            ToolDefinition::new(
                "list_directory",
                "List files and directories in a given path",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path" }
                },
                "required": ["path"]
            })),
            ToolDefinition::new(
                "shell",
                "Execute a shell command with optional timeout and working directory",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in milliseconds (default 120000)" },
                    "cwd": { "type": "string", "description": "Working directory" }
                },
                "required": ["command"]
            })),
            ToolDefinition::new(
                "run_command",
                "Execute a shell command (alias for shell)",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" },
                    "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds" }
                },
                "required": ["command"]
            })),
            ToolDefinition::new(
                "git_status",
                "Show git working tree status",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "cwd": { "type": "string", "description": "Working directory" }
                }
            })),
            ToolDefinition::new(
                "git_diff",
                "Show git diff (staged or unstaged)",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "staged": { "type": "boolean", "description": "Show staged changes" },
                    "path": { "type": "string", "description": "Optional file path filter" },
                    "cwd": { "type": "string", "description": "Working directory" }
                }
            })),
            ToolDefinition::new(
                "git_log",
                "Show recent git commits",
            )
            .with_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "Number of commits (default 10)" },
                    "path": { "type": "string", "description": "Optional file path filter" },
                    "cwd": { "type": "string", "description": "Working directory" }
                }
            })),
        ]
    }
}

#[async_trait]
impl ToolExecutor for CliToolExecutor {
    async fn execute(&self, request: ToolRequest) -> clawdius_core::error::Result<ToolResult> {
        let result = match request.name.as_str() {
            "read_file" => {
                let path = match Self::get_string_arg(&request.arguments, "path") {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing 'path' argument")),
                };
                let offset = Self::get_usize_arg(&request.arguments, "offset");
                let limit = Self::get_usize_arg(&request.arguments, "limit");

                match self.file_tool.read(FileReadParams {
                    path,
                    offset,
                    limit,
                }) {
                    Ok(content) => ToolResult::success(content),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "write_file" => {
                let path = match Self::get_string_arg(&request.arguments, "path") {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing 'path' argument")),
                };
                let content = match Self::get_string_arg(&request.arguments, "content") {
                    Some(c) => c,
                    None => return Ok(ToolResult::error("Missing 'content' argument")),
                };

                match self.file_tool.write(FileWriteParams { path, content }) {
                    Ok(()) => ToolResult::success("File written successfully"),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "edit_file" => {
                let path = match Self::get_string_arg(&request.arguments, "path") {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing 'path' argument")),
                };
                let old_string = match Self::get_string_arg(&request.arguments, "old_string") {
                    Some(s) => s,
                    None => return Ok(ToolResult::error("Missing 'old_string' argument")),
                };
                let new_string = match Self::get_string_arg(&request.arguments, "new_string") {
                    Some(s) => s,
                    None => return Ok(ToolResult::error("Missing 'new_string' argument")),
                };
                let replace_all = Self::get_bool_arg(&request.arguments, "replace_all");

                match self.file_tool.edit(FileEditParams {
                    path,
                    old_string,
                    new_string,
                    replace_all,
                }) {
                    Ok(true) => ToolResult::success("Edit applied successfully"),
                    Ok(false) => ToolResult::error("old_string not found in file"),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "list_directory" => {
                let path = match Self::get_string_arg(&request.arguments, "path") {
                    Some(p) => p,
                    None => return Ok(ToolResult::error("Missing 'path' argument")),
                };

                match self.file_tool.list(FileListParams { path }) {
                    Ok(entries) => ToolResult::success(entries.join("\n")),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "shell" | "run_command" => {
                let command = match Self::get_string_arg(&request.arguments, "command") {
                    Some(c) => c,
                    None => return Ok(ToolResult::error("Missing 'command' argument")),
                };
                let cwd = Self::get_string_arg(&request.arguments, "cwd");
                let timeout = if request.name == "run_command" {
                    Self::get_u64_arg(&request.arguments, "timeout_ms")
                } else {
                    Self::get_u64_arg(&request.arguments, "timeout")
                };

                match self.shell_tool.execute(ShellParams {
                    command,
                    timeout,
                    cwd,
                }) {
                    Ok(result) => {
                        let mut output = result.stdout;
                        if !result.stderr.is_empty() {
                            output.push_str("\n[stderr]\n");
                            output.push_str(&result.stderr);
                        }
                        if result.timed_out {
                            output.push_str("\n[TIMEOUT]");
                        }
                        if result.exit_code != 0 {
                            ToolResult {
                                success: false,
                                content: format!("exit code {}: {}", result.exit_code, output),
                                is_error: true,
                            }
                        } else {
                            ToolResult::success(output)
                        }
                    },
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "git_status" => {
                let cwd = Self::get_string_arg(&request.arguments, "cwd").map(String::from);

                match self.git_tool.status(cwd.as_deref()) {
                    Ok(output) => ToolResult::success(output),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "git_diff" => {
                let cwd = Self::get_string_arg(&request.arguments, "cwd").map(String::from);
                let staged = Self::get_bool_arg(&request.arguments, "staged");
                let path = Self::get_string_arg(&request.arguments, "path");

                match self
                    .git_tool
                    .diff(GitDiffParams { staged, path }, cwd.as_deref())
                {
                    Ok(output) => ToolResult::success(output),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            "git_log" => {
                let cwd = Self::get_string_arg(&request.arguments, "cwd").map(String::from);
                let count = Self::get_usize_arg(&request.arguments, "count").unwrap_or(10);
                let path = Self::get_string_arg(&request.arguments, "path");

                match self
                    .git_tool
                    .log(GitLogParams { count, path }, cwd.as_deref())
                {
                    Ok(output) => ToolResult::success(output),
                    Err(e) => ToolResult::error(e.to_string()),
                }
            },

            _ => ToolResult::error(format!("Unknown tool: {}", request.name)),
        };

        Ok(result)
    }

    fn has_tool(&self, name: &str) -> bool {
        matches!(
            name,
            "read_file"
                | "write_file"
                | "edit_file"
                | "list_directory"
                | "shell"
                | "run_command"
                | "git_status"
                | "git_diff"
                | "git_log"
        )
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        Self::tool_definitions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_unknown_tool() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());
        let request = ToolRequest::new("nonexistent_tool");
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
        assert!(result.is_error);
        assert!(result.content.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_has_tool() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());
        assert!(executor.has_tool("read_file"));
        assert!(executor.has_tool("shell"));
        assert!(executor.has_tool("git_status"));
        assert!(!executor.has_tool("nonexistent"));
    }

    #[tokio::test]
    async fn test_list_tools() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());
        let tools = executor.list_tools();
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"shell"));
        assert!(names.contains(&"git_diff"));
    }

    #[tokio::test]
    async fn test_read_missing_arg() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());
        let request = ToolRequest::new("read_file");
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());
        let request = ToolRequest::new("read_file").with_arg("path", serde_json::json!("nope.txt"));
        let result = executor.execute(request).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());

        let file_path = dir.path().join("test.txt");
        let path_str = file_path.to_str().unwrap().to_string();

        let write_request = ToolRequest::new("write_file")
            .with_arg("path", serde_json::json!(path_str))
            .with_arg("content", serde_json::json!("hello world"));
        let write_result = executor.execute(write_request).await.unwrap();
        assert!(write_result.success);

        let read_request =
            ToolRequest::new("read_file").with_arg("path", serde_json::json!(path_str));
        let read_result = executor.execute(read_request).await.unwrap();
        assert!(read_result.success);
        assert_eq!(read_result.content, "hello world");
    }

    #[tokio::test]
    async fn test_edit_file() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());

        let file_path = dir.path().join("edit.txt");
        let path_str = file_path.to_str().unwrap().to_string();

        fs::write(&file_path, "foo bar baz").unwrap();

        let request = ToolRequest::new("edit_file")
            .with_arg("path", serde_json::json!(path_str))
            .with_arg("old_string", serde_json::json!("bar"))
            .with_arg("new_string", serde_json::json!("qux"));
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "foo qux baz");
    }

    #[tokio::test]
    async fn test_list_directory() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());

        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();

        let request = ToolRequest::new("list_directory")
            .with_arg("path", serde_json::json!(dir.path().to_str().unwrap()));
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("a.txt"));
        assert!(result.content.contains("b.txt"));
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());

        let request =
            ToolRequest::new("shell").with_arg("command", serde_json::json!("echo hello"));
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_run_command_alias() {
        let dir = tempfile::tempdir().unwrap();
        let executor = CliToolExecutor::new(dir.path().to_path_buf());

        let request =
            ToolRequest::new("run_command").with_arg("command", serde_json::json!("printf hi"));
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hi"));
    }
}
