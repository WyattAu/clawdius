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

    #[test]
    fn test_format_notification_serializes_to_json() {
        let resp = McpResponse::notification();
        assert!(resp.is_notification());
        // Notification serialization should produce valid JSON
        let json = format_response(&resp);
        assert!(!json.is_empty(), "notification should serialize to non-empty JSON");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
    }

    #[test]
    fn test_parse_request_with_extra_fields() {
        // serde default behavior: unknown fields ignored
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"foo","extra":42,"more":"data"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "foo");
    }

    #[test]
    fn test_parse_request_with_null_id_rejected() {
        // MCP protocol uses u64 id; null id is rejected
        let json = r#"{"jsonrpc":"2.0","id":null,"method":"initialized"}"#;
        let result = parse_request(json);
        assert!(result.is_err(), "null id should be rejected for MCP u64 id");
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_parse_request_with_whitespace_only() {
        let result = parse_request("   \t\n  ");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_format_response_internal_error() {
        let resp = McpResponse::error(1, McpError::internal_error("internal failure"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32603);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("internal failure"));
    }

    #[test]
    fn test_format_response_invalid_request() {
        let resp = McpResponse::error(0, McpError::invalid_request("bad request"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32600);
    }

    #[test]
    fn test_parse_request_with_array_params() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":[1,2,3]}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "test");
        let params = req.params.unwrap();
        assert!(params.is_array());
        assert_eq!(params.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_format_success_response_large_payload() {
        let large_data = "x".repeat(100_000);
        let resp = McpResponse::success(1, serde_json::json!({"data": large_data}));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["data"].as_str().unwrap().len(), 100_000);
    }

    #[test]
    fn test_format_response_parse_error() {
        let resp = McpResponse::error(0, McpError::parse_error("parse failure"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32700);
    }

    #[test]
    fn test_format_response_invalid_params() {
        let resp = McpResponse::error(1, McpError::invalid_params("missing field"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32602);
    }

    #[test]
    fn test_parse_request_without_jsonrpc_field() {
        let json = r#"{"id":1,"method":"test"}"#;
        let result = parse_request(json);
        // Missing jsonrpc field is a schema violation
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_without_method_field() {
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result = parse_request(json);
        // Missing method field is a schema violation
        assert!(result.is_err());
    }

    #[test]
    fn test_format_success_response_empty_result() {
        let resp = McpResponse::success(1, serde_json::json!({}));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["result"], serde_json::json!({}));
        assert!(parsed.get("error").is_none());
    }
}
