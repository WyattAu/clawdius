use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use clawdius_core::mcp::{handle_mcp_request, McpError, McpRequest, McpResponse};

pub async fn handle_mcp(body: Bytes) -> impl IntoResponse {
    let request: McpRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            let resp = McpResponse::error(0, McpError::parse_error(e.to_string()));
            return (
                StatusCode::OK,
                Json(serde_json::to_value(resp).unwrap_or_default()),
            )
                .into_response();
        },
    };

    let response = handle_mcp_request(&request);

    if response.is_notification() {
        return (StatusCode::OK, Json(serde_json::Value::Null)).into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_default()),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::mcp::protocol::MCP_VERSION;

    #[test]
    fn test_initialize_result() {
        let req = McpRequest::new(1, "initialize");
        let resp = handle_mcp_request(&req);
        let result = resp
            .result
            .as_ref()
            .expect("initialize should return result");
        assert_eq!(result["serverInfo"]["name"], "clawdius-server");
        assert_eq!(result["protocolVersion"], MCP_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list() {
        let req = McpRequest::new(1, "tools/list");
        let resp = handle_mcp_request(&req);
        let result = resp
            .result
            .as_ref()
            .expect("tools/list should return result");
        let tools = result["tools"]
            .as_array()
            .expect("tools should be an array");
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().expect("tool name should be string"))
            .collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"check_build"));
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn test_git_status_tool() {
        let req =
            McpRequest::new(1, "tools/call").with_params(serde_json::json!({"name": "git_status"}));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_git_log_tool() {
        let req = McpRequest::new(1, "tools/call")
            .with_params(serde_json::json!({"name": "git_log", "arguments": {"count": 3}}));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_git_diff_tool() {
        let req =
            McpRequest::new(1, "tools/call").with_params(serde_json::json!({"name": "git_diff"}));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_read_file_missing_param() {
        let req = McpRequest::new(1, "tools/call")
            .with_params(serde_json::json!({"name": "read_file", "arguments": {}}));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"]
            .as_bool()
            .expect("is_error should be bool"));
    }

    #[test]
    fn test_read_file_cargo_toml() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let path = format!("{cargo_path}/Cargo.toml");
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "read_file",
            "arguments": { "path": path }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"]
            .as_str()
            .expect("content text should be string");
        assert!(text.contains("clawdius-server"));
    }

    #[test]
    fn test_list_directory_missing_param() {
        let req = McpRequest::new(1, "tools/call")
            .with_params(serde_json::json!({"name": "list_directory", "arguments": {}}));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"]
            .as_bool()
            .expect("is_error should be bool"));
    }

    #[test]
    fn test_list_directory_src() {
        let src_path = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "list_directory",
            "arguments": { "path": format!("{src_path}/src") }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"]
            .as_str()
            .expect("content text should be string");
        assert!(text.contains("main.rs"));
    }

    #[test]
    fn test_notification_no_response_body() {
        let req = McpRequest::new(0, "notifications/initialized");
        let resp = handle_mcp_request(&req);
        assert!(resp.is_notification());
    }
}
