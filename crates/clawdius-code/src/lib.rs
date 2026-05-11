//! Testable core logic for the JSON-RPC server (clawdius-code).

use clawdius_core::rpc::types::{Error as RpcError, Request, Response};

/// Parse a raw JSON string into a JSON-RPC request.
///
/// # Errors
///
/// Returns `RpcError::parse_error` if the input is not valid JSON or does not
/// conform to the JSON-RPC 2.0 request schema.
pub fn parse_request(raw: &str) -> Result<Request, RpcError> {
    serde_json::from_str(raw).map_err(|e| RpcError::parse_error(e.to_string()))
}

/// Format a JSON-RPC response into a JSON string.
///
/// Falls back to an empty string if serialization fails.
#[must_use]
pub fn format_response(response: &Response) -> String {
    response.to_json().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::rpc::types::Id;

    #[test]
    fn test_parse_valid_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"session/create","params":{"name":"test"}}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::Number(1));
        assert_eq!(req.method, "session/create");
        assert!(req.params.is_some());
    }

    #[test]
    fn test_parse_valid_request_no_params() {
        let json = r#"{"jsonrpc":"2.0","id":2,"method":"session/list"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::Number(2));
        assert_eq!(req.method, "session/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_parse_malformed_json() {
        let result = parse_request("{broken");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_request("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_method_not_found_response() {
        let resp = Response::method_not_found(Id::Number(1), "unknown/method");
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32601);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown/method"));
    }

    #[test]
    fn test_invalid_params_response() {
        let resp = Response::invalid_params(Id::Number(1), "missing field: name");
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32602);
    }

    #[test]
    fn test_success_response_format() {
        let resp = Response::success(Id::Number(5), serde_json::json!({"ok": true}));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 5);
        assert_eq!(parsed["result"]["ok"], true);
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_notification_parsing() {
        let json = r#"{"jsonrpc":"2.0","id":null,"method":"initialized"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "initialized");
        assert!(req.id.is_null());
        assert!(req.params.is_none());
    }

    #[test]
    fn test_string_id_request() {
        let json = r#"{"jsonrpc":"2.0","id":"abc","method":"chat/send"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::String("abc".to_string()));
        assert_eq!(req.method, "chat/send");
    }
}
