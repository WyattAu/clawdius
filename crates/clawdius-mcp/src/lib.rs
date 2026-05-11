//! Testable core logic for the MCP stdio server.

use clawdius_core::mcp::protocol::{McpError, McpRequest, McpResponse};

/// Parse a raw JSON string into an MCP request.
///
/// # Errors
///
/// Returns `McpError::parse_error` if the input is not valid JSON or does not
/// conform to the JSON-RPC 2.0 request schema.
pub fn parse_request(raw: &str) -> Result<McpRequest, McpError> {
    serde_json::from_str(raw).map_err(|e| McpError::parse_error(e.to_string()))
}

/// Format an MCP response into a JSON string.
///
/// Falls back to an empty string if serialization fails.
#[must_use]
pub fn format_response(response: &McpResponse) -> String {
    serde_json::to_string(response).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());
    }

    #[test]
    fn test_parse_valid_request_no_params() {
        let json = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, 2);
        assert_eq!(req.method, "tools/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        let result = parse_request("not json at all");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32700);
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_request("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_parse_invalid_params_type() {
        let json = r#"{"jsonrpc":"2.0","id":3,"method":"test","params":"should_be_object"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "test");
        assert!(req.params.is_some());
    }

    #[test]
    fn test_format_success_response() {
        let resp = McpResponse::success(1, serde_json::json!({"status": "ok"}));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["status"], "ok");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_format_error_response() {
        let resp = McpResponse::error(1, McpError::method_not_found("foo/bar"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32601);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("foo/bar"));
    }

    #[test]
    fn test_format_notification_skipped() {
        let resp = McpResponse::notification();
        assert!(resp.is_notification());
    }

    #[test]
    fn test_roundtrip_valid_request() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"/tmp/test"}}}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, 42);
        assert_eq!(req.method, "tools/call");
        let params = req.params.unwrap();
        assert_eq!(params["name"], "read_file");
    }
}
