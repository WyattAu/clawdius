use super::protocol::*;

/// Handle a single MCP JSON-RPC request and return the response.
/// This is transport-agnostic — both HTTP and stdio transports call this.
pub fn handle_mcp_request(request: &McpRequest) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "notifications/initialized" => McpResponse::notification(),
        "ping" => McpResponse::success(request.id, serde_json::json!({})),
        "tools/list" => handle_tools_list(request),
        "tools/call" => handle_tools_call(request),
        "resources/list" => handle_resources_list(request),
        "prompts/list" => handle_prompts_list(request),
        _ => McpResponse::error(request.id, McpError::method_not_found(&request.method)),
    }
}

fn handle_initialize(request: &McpRequest) -> McpResponse {
    let result = serde_json::json!({
        "protocolVersion": MCP_VERSION,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "listChanged": false, "subscribe": false },
            "prompts": { "listChanged": false }
        },
        "serverInfo": {
            "name": "clawdius-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    });
    McpResponse::success(request.id, result)
}

fn handle_tools_list(request: &McpRequest) -> McpResponse {
    let tools = vec![
        McpTool::new("read_file", "Read the contents of a file").with_string_param(
            "path",
            "Absolute path to the file",
            true,
        ),
        McpTool::new(
            "list_directory",
            "List files and directories at a given path",
        )
        .with_string_param("path", "Directory path to list", true),
        McpTool::new("git_status", "Show short git status output"),
        McpTool::new("git_log", "Show recent git commits").with_string_param(
            "count",
            "Number of commits to show (default 10)",
            false,
        ),
        McpTool::new("git_diff", "Show git diff output"),
        McpTool::new("check_build", "Run cargo check and return the result"),
        McpTool::new("write_file", "Write content to a file")
            .with_string_param("path", "File path relative to workspace root", true)
            .with_string_param("content", "Content to write", true),
        McpTool::new(
            "edit_file",
            "Replace first occurrence of old_string with new_string in a file",
        )
        .with_string_param("path", "File path relative to workspace root", true)
        .with_string_param("old_string", "Text to find and replace", true)
        .with_string_param("new_string", "Replacement text", true),
    ];
    McpResponse::success(request.id, serde_json::json!({ "tools": tools }))
}

fn handle_tools_call(request: &McpRequest) -> McpResponse {
    let params = request.params.as_ref().unwrap_or(&serde_json::Value::Null);

    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return McpResponse::error(request.id, McpError::invalid_params("missing tool name"));
        },
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match name {
        "read_file" => tool_read_file(&arguments),
        "list_directory" => tool_list_directory(&arguments),
        "git_status" => tool_git_status(),
        "git_log" => tool_git_log(&arguments),
        "git_diff" => tool_git_diff(),
        "check_build" => tool_check_build(),
        "write_file" => tool_write_file(&arguments),
        "edit_file" => tool_edit_file(&arguments),
        _ => {
            return McpResponse::error(
                request.id,
                McpError::invalid_params(format!("unknown tool: {name}")),
            );
        },
    };

    McpResponse::success(request.id, serde_json::to_value(result).unwrap_or_default())
}

fn handle_resources_list(request: &McpRequest) -> McpResponse {
    McpResponse::success(request.id, serde_json::json!({ "resources": [] }))
}

fn handle_prompts_list(request: &McpRequest) -> McpResponse {
    McpResponse::success(request.id, serde_json::json!({ "prompts": [] }))
}

fn tool_read_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    match std::fs::read_to_string(path) {
        Ok(content) => McpToolResult::text(content),
        Err(e) => McpToolResult::error(format!("failed to read file: {e}")),
    }
}

fn tool_list_directory(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut lines = Vec::new();
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let kind = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "dir"
                } else {
                    "file"
                };
                lines.push(format!("{kind} {file_name}"));
            }
            McpToolResult::text(lines.join("\n"))
        },
        Err(e) => McpToolResult::error(format!("failed to list directory: {e}")),
    }
}

fn tool_git_status() -> McpToolResult {
    match std::process::Command::new("git")
        .args(["status", "--short"])
        .output()
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.is_empty() {
                McpToolResult::text("(clean working tree)")
            } else {
                McpToolResult::text(text)
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run git status: {e}")),
    }
}

fn tool_git_log(args: &serde_json::Value) -> McpToolResult {
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10);

    match std::process::Command::new("git")
        .args(["log", &format!("-{count}"), "--oneline"])
        .output()
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            McpToolResult::text(text)
        },
        Err(e) => McpToolResult::error(format!("failed to run git log: {e}")),
    }
}

fn tool_git_diff() -> McpToolResult {
    match std::process::Command::new("git").arg("diff").output() {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.is_empty() {
                McpToolResult::text("(no diff)")
            } else {
                McpToolResult::text(text)
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run git diff: {e}")),
    }
}

fn tool_check_build() -> McpToolResult {
    match std::process::Command::new("cargo").arg("check").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stdout.is_empty() { stderr } else { stdout };
            if output.status.success() {
                McpToolResult::text(format!("cargo check passed:\n{combined}"))
            } else {
                McpToolResult::error(format!("cargo check failed:\n{combined}"))
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run cargo check: {e}")),
    }
}

fn validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    if path.contains("..") {
        return Err("path traversal ('..') is not allowed".to_string());
    }
    let workspace_root =
        std::env::current_dir().map_err(|e| format!("failed to get workspace root: {e}"))?;
    let resolved = workspace_root.join(path);
    let resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !resolved.starts_with(&workspace_root) {
        return Err("path is outside the workspace root".to_string());
    }
    Ok(resolved)
}

fn tool_write_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };
    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return McpToolResult::error("missing 'content' parameter"),
    };

    let resolved = match validate_path(path) {
        Ok(p) => p,
        Err(e) => return McpToolResult::error(e),
    };

    if let Some(parent) = resolved.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return McpToolResult::error(format!("failed to create directories: {e}"));
        }
    }

    match std::fs::write(&resolved, content) {
        Ok(()) => {
            let bytes = content.len();
            McpToolResult::text(format!("wrote {bytes} bytes to {path}"))
        },
        Err(e) => McpToolResult::error(format!("failed to write file: {e}")),
    }
}

fn tool_edit_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };
    let old_string = match args.get("old_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return McpToolResult::error("missing 'old_string' parameter"),
    };
    let new_string = match args.get("new_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return McpToolResult::error("missing 'new_string' parameter"),
    };

    let resolved = match validate_path(path) {
        Ok(p) => p,
        Err(e) => return McpToolResult::error(e),
    };

    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(e) => return McpToolResult::error(format!("failed to read file: {e}")),
    };

    if !content.contains(old_string) {
        return McpToolResult::error("old_string not found in file");
    }

    let new_content = content.replacen(old_string, new_string, 1);

    match std::fs::write(&resolved, new_content) {
        Ok(()) => McpToolResult::text(format!("edited {path} successfully")),
        Err(e) => McpToolResult::error(format!("failed to write file: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        let req = McpRequest::new(1, "initialize");
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "clawdius-server");
        assert_eq!(result["protocolVersion"], MCP_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_notification() {
        let req = McpRequest::new(0, "notifications/initialized");
        let resp = handle_mcp_request(&req);
        assert!(resp.is_notification());
        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_ping() {
        let req = McpRequest::new(1, "ping");
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn test_tools_list() {
        let req = McpRequest::new(1, "tools/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        let tools = result["tools"]
            .as_array()
            .expect("tools should be an array");
        assert_eq!(tools.len(), 8);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"check_build"));
    }

    #[test]
    fn test_tools_call_fixed_params() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "git_status"
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_tools_call_missing_name() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn test_tools_call_unknown_tool() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "nonexistent_tool"
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn test_method_not_found() {
        let req = McpRequest::new(1, "foo/bar");
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_resources_list() {
        let req = McpRequest::new(1, "resources/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["resources"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_prompts_list() {
        let req = McpRequest::new(1, "prompts/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["prompts"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_read_file() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = format!("{cargo_path}/Cargo.toml");
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "read_file",
            "arguments": { "path": path }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("clawdius-core"));
    }

    #[test]
    fn test_read_file_missing_param() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "read_file",
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
    }

    #[test]
    fn test_list_directory() {
        let src_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "list_directory",
            "arguments": { "path": format!("{src_path}/src") }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("lib.rs") || text.contains("mcp"));
    }

    #[test]
    fn test_notification_request_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: McpRequest =
            serde_json::from_str(json).expect("should parse notification without id");
        assert_eq!(req.id, 0);
        let resp = handle_mcp_request(&req);
        assert!(resp.is_notification());
    }

    #[test]
    fn test_input_schema_camelcase() {
        let tool = McpTool::new("test", "Test");
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("inputSchema"));
        assert!(!json.contains("input_schema"));
    }

    #[test]
    fn test_write_file_creates_file() {
        let dir = std::env::temp_dir().join("clawdius_test_write");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("sub").join("test.txt");
        let relative = format!("sub/test.txt");

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "write_file",
            "arguments": { "path": relative, "content": "hello world" }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("11 bytes"));
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_file_rejects_traversal() {
        let dir = std::env::temp_dir().join("clawdius_test_traversal");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "write_file",
            "arguments": { "path": "../etc/passwd", "content": "hack" }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("traversal"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_edit_file_replaces_content() {
        let dir = std::env::temp_dir().join("clawdius_test_edit");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("edit_me.txt"), "foo bar baz").unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "edit_file",
            "arguments": {
                "path": "edit_me.txt",
                "old_string": "bar",
                "new_string": "qux"
            }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.join("edit_me.txt")).unwrap(),
            "foo qux baz"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_edit_file_old_string_not_found() {
        let dir = std::env::temp_dir().join("clawdius_test_edit_nf");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("missing.txt"), "hello").unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "edit_file",
            "arguments": {
                "path": "missing.txt",
                "old_string": "goodbye",
                "new_string": "world"
            }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
