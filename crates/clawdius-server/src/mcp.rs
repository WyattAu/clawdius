use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use clawdius_core::mcp::*;

pub async fn handle_mcp(body: Bytes) -> impl IntoResponse {
    let req: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(
                    serde_json::to_value(McpResponse::error(
                        0,
                        McpError::parse_error(e.to_string()),
                    ))
                    .unwrap(),
                ),
            )
                .into_response();
        },
    };

    let id = req.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = req.get("params").cloned();

    let result = match method {
        "initialize" => handle_initialize(params),
        "notifications/initialized" => {
            return (StatusCode::OK, Json(serde_json::Value::Null)).into_response();
        },
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(params).await,
        "resources/list" => Ok(serde_json::json!({ "resources": [] })),
        "prompts/list" => Ok(serde_json::json!({ "prompts": [] })),
        _ => Err(McpError::method_not_found(method)),
    };

    let resp = match result {
        Ok(data) => McpResponse::success(id, data),
        Err(err) => McpResponse::error(id, err),
    };

    (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response()
}

fn handle_initialize(_params: Option<serde_json::Value>) -> Result<serde_json::Value, McpError> {
    let result = serde_json::json!({
        "protocolVersion": protocol::MCP_VERSION,
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
    Ok(result)
}

fn handle_tools_list() -> Result<serde_json::Value, McpError> {
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
    ];

    Ok(serde_json::json!({ "tools": tools }))
}

async fn handle_tools_call(
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, McpError> {
    let args = params
        .and_then(|p| p.get("arguments").cloned())
        .unwrap_or(serde_json::Value::Null);

    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing tool name"))?;

    let tool_args = args
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match name {
        "read_file" => tool_read_file(&tool_args).await,
        "list_directory" => tool_list_directory(&tool_args).await,
        "git_status" => tool_git_status().await,
        "git_log" => tool_git_log(&tool_args).await,
        "git_diff" => tool_git_diff().await,
        "check_build" => tool_check_build().await,
        _ => return Err(McpError::invalid_params(format!("unknown tool: {name}"))),
    };

    Ok(serde_json::to_value(result).unwrap())
}

async fn tool_read_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    match tokio::fs::read_to_string(path).await {
        Ok(content) => McpToolResult::text(content),
        Err(e) => McpToolResult::error(format!("failed to read file: {e}")),
    }
}

async fn tool_list_directory(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    let mut entries = tokio::fs::read_dir(path).await;
    match &mut entries {
        Ok(dir) => {
            let mut lines = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let kind = if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
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

async fn tool_git_status() -> McpToolResult {
    match tokio::process::Command::new("git")
        .args(["status", "--short"])
        .output()
        .await
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

async fn tool_git_log(args: &serde_json::Value) -> McpToolResult {
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10);

    match tokio::process::Command::new("git")
        .args(["log", &format!("-{count}"), "--oneline"])
        .output()
        .await
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            McpToolResult::text(text)
        },
        Err(e) => McpToolResult::error(format!("failed to run git log: {e}")),
    }
}

async fn tool_git_diff() -> McpToolResult {
    match tokio::process::Command::new("git")
        .arg("diff")
        .output()
        .await
    {
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

async fn tool_check_build() -> McpToolResult {
    match tokio::process::Command::new("cargo")
        .arg("check")
        .output()
        .await
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_result() {
        let result = handle_initialize(None).unwrap();
        assert_eq!(result["serverInfo"]["name"], "clawdius-server");
        assert_eq!(result["protocolVersion"], protocol::MCP_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list() {
        let result = handle_tools_list().unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"check_build"));
        assert_eq!(tools.len(), 6);
    }

    #[tokio::test]
    async fn test_git_status_tool() {
        let result = tool_git_status().await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_git_log_tool() {
        let result = tool_git_log(&serde_json::json!({"count": 3})).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_git_diff_tool() {
        let result = tool_git_diff().await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_read_file_missing_param() {
        let result = tool_read_file(&serde_json::json!({})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_read_file_cargo_toml() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = format!("{cargo_path}/Cargo.toml");
        let result = tool_read_file(&serde_json::json!({"path": path})).await;
        assert!(!result.is_error);
        let McpContent::Text { text } = &result.content[0] else {
            panic!("expected text content");
        };
        assert!(text.contains("clawdius-server"));
    }

    #[tokio::test]
    async fn test_list_directory_missing_param() {
        let result = tool_list_directory(&serde_json::json!({})).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_list_directory_src() {
        let src_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let result =
            tool_list_directory(&serde_json::json!({"path": format!("{src_path}/src")})).await;
        assert!(!result.is_error);
        let McpContent::Text { text } = &result.content[0] else {
            panic!("expected text content");
        };
        assert!(text.contains("main.rs"));
    }
}
