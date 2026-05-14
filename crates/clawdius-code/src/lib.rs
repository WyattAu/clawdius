//! Testable core logic for the JSON-RPC server (clawdius-code).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

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

    #[test]
    fn test_format_internal_error_response() {
        let resp = Response::internal_error(Id::Number(1), "internal failure");
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32603);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("internal failure"));
    }

    #[test]
    fn test_format_parse_error_response() {
        let resp = Response::error(Id::Number(0), RpcError::parse_error("bad json"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32700);
    }

    #[test]
    fn test_format_invalid_request_response() {
        let resp = Response::error(Id::Number(0), RpcError::invalid_request("bad request"));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32600);
    }

    #[test]
    fn test_parse_request_with_extra_fields() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","extra":true,"more":42}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "test");
        assert_eq!(req.id, Id::Number(1));
    }

    #[test]
    fn test_parse_request_with_whitespace_only() {
        let result = parse_request("   \t\n  ");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_success_response_with_null_result() {
        let resp = Response::success(Id::Number(1), serde_json::Value::Null);
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["result"], serde_json::Value::Null);
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_success_response_empty_object() {
        let resp = Response::success(Id::Number(5), serde_json::json!({}));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["result"], serde_json::json!({}));
    }

    #[test]
    fn test_format_server_error_response() {
        let resp = Response::error(
            Id::Number(1),
            RpcError::server_error(-32000, "custom server error"),
        );
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"]["code"], -32000);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("custom server error"));
    }

    #[test]
    fn test_parse_request_without_jsonrpc_field() {
        let json = r#"{"id":1,"method":"test"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_without_method_field() {
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_with_large_id() {
        let json = r#"{"jsonrpc":"2.0","id":999999999,"method":"test"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::Number(999_999_999));
    }

    #[test]
    fn test_success_response_with_array_result() {
        let resp = Response::success(Id::Number(1), serde_json::json!([1, 2, 3]));
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["result"].is_array());
        assert_eq!(parsed["result"].as_array().unwrap().len(), 3);
    }
}

#[cfg(test)]
mod error_path_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn test_parse_json_array_instead_of_object() {
        let result = parse_request(r"[1, 2, 3]");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32700);
    }

    #[test]
    fn test_parse_json_number_instead_of_object() {
        let result = parse_request("42");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_boolean_instead_of_object() {
        let result = parse_request("true");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_null_instead_of_object() {
        let result = parse_request("null");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_valid_json_but_wrong_version_still_parses() {
        let json = r#"{"jsonrpc":"1.0","id":1,"method":"test"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.jsonrpc, "1.0");
        assert_eq!(req.method, "test");
    }

    #[test]
    fn test_parse_method_is_number_instead_of_string() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":42}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_id_is_array_rejected() {
        let json = r#"{"jsonrpc":"2.0","id":[1],"method":"test"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_id_is_object_rejected() {
        let json = r#"{"jsonrpc":"2.0","id":{"x":1},"method":"test"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_params_is_string_accepted() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":"raw"}"#;
        let req = parse_request(json).unwrap();
        assert!(req.params.is_some());
    }

    #[test]
    fn test_parse_error_message_contains_detail() {
        let result = parse_request("{invalid");
        let err = result.unwrap_err();
        assert!(!err.message.is_empty());
    }
}

#[cfg(test)]
mod edge_case_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use clawdius_core::rpc::types::Id;

    #[test]
    fn test_parse_unicode_method_name() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"日本語/テスト"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "日本語/テスト");
    }

    #[test]
    fn test_parse_unicode_params() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"emoji":"🎉🚀"}}"#;
        let req = parse_request(json).unwrap();
        let params = req.params.unwrap();
        assert_eq!(params["emoji"], "🎉🚀");
    }

    #[test]
    fn test_parse_zero_id() {
        let json = r#"{"jsonrpc":"2.0","id":0,"method":"test"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::Number(0));
    }

    #[test]
    fn test_parse_negative_id() {
        let json = r#"{"jsonrpc":"2.0","id":-1,"method":"test"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::Number(-1));
    }

    #[test]
    fn test_parse_empty_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"","method":"test"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.id, Id::String(String::new()));
    }

    #[test]
    fn test_parse_empty_method_name() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":""}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "");
    }

    #[test]
    fn test_parse_empty_params_object() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        let req = parse_request(json).unwrap();
        assert!(req.params.is_some());
        assert_eq!(req.params.unwrap(), serde_json::json!({}));
    }

    #[test]
    fn test_format_response_with_deeply_nested_result() {
        let nested = serde_json::json!({"a":{"b":{"c":{"d":"deep"}}}});
        let resp = Response::success(Id::Number(1), nested);
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["result"]["a"]["b"]["c"]["d"], "deep");
    }

    #[test]
    fn test_format_response_with_special_chars_in_error_message() {
        let resp = Response::internal_error(Id::Number(1), "error with \"quotes\" and \n newlines");
        let json = format_response(&resp);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["error"]["message"].as_str().unwrap().contains("quotes"));
    }
}

#[cfg(test)]
mod roundtrip_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use clawdius_core::rpc::types::{Id, Request};

    #[test]
    fn test_request_parse_roundtrip() {
        let original = r#"{"jsonrpc":"2.0","id":42,"method":"session/create","params":{"name":"my-session"}}"#;
        let req = parse_request(original).unwrap();
        let serialized = serde_json::to_string(&req).unwrap();
        let req2 = parse_request(&serialized).unwrap();
        assert_eq!(req.id, req2.id);
        assert_eq!(req.method, req2.method);
        assert_eq!(req.params, req2.params);
    }

    #[test]
    fn test_response_format_roundtrip() {
        let resp = Response::success(Id::Number(7), serde_json::json!({"key": "value", "num": 123}));
        let json = format_response(&resp);
        let reparsed = Response::from_json(&json).unwrap();
        assert_eq!(reparsed.id, resp.id);
        assert_eq!(reparsed.result, resp.result);
        assert!(reparsed.error.is_none());
    }

    #[test]
    fn test_error_response_format_roundtrip() {
        let resp = Response::method_not_found(Id::String("req-1".into()), "missing/method");
        let json = format_response(&resp);
        let reparsed = Response::from_json(&json).unwrap();
        assert_eq!(reparsed.id, Id::String("req-1".to_string()));
        assert!(reparsed.error.is_some());
        assert_eq!(reparsed.error.unwrap().code, -32601);
    }

    #[test]
    fn test_request_to_response_id_preservation() {
        let json = r#"{"jsonrpc":"2.0","id":"unique-str-id","method":"chat/send","params":{"msg":"hi"}}"#;
        let req = parse_request(json).unwrap();
        let resp = Response::success(req.id, serde_json::json!({"ok": true}));
        let resp_json = format_response(&resp);
        let resp_parsed: serde_json::Value = serde_json::from_str(&resp_json).unwrap();
        assert_eq!(resp_parsed["id"], "unique-str-id");
    }

    #[test]
    fn test_parse_request_builder_roundtrip() {
        let req = Request::new("req-99", "test/method")
            .with_params(serde_json::json!({"foo": [1, 2, 3]}));
        let json = serde_json::to_string(&req).unwrap();
        let parsed = parse_request(&json).unwrap();
        assert_eq!(parsed.id, Id::String("req-99".to_string()));
        assert_eq!(parsed.method, "test/method");
        let params = parsed.params.unwrap();
        assert_eq!(params["foo"].as_array().unwrap().len(), 3);
    }
}

#[cfg(test)]
mod concurrent_access_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_parse_request_from_multiple_threads() {
        let json = Arc::new(r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"k":"v"}}"#.to_string());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let j = Arc::clone(&json);
                thread::spawn(move || {
                    let req = parse_request(&j).unwrap();
                    assert_eq!(req.method, "test");
                    req
                })
            })
            .collect();
        for h in handles {
            let req = h.join().unwrap();
            assert_eq!(req.method, "test");
        }
    }

    #[test]
    fn test_format_response_from_multiple_threads() {
        let resp = Arc::new(Response::success(
            clawdius_core::rpc::types::Id::Number(1),
            serde_json::json!({"status": "ok"}),
        ));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let r = Arc::clone(&resp);
                thread::spawn(move || {
                    let json = format_response(&r);
                    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                    assert_eq!(parsed["result"]["status"], "ok");
                    json
                })
            })
            .collect();
        for h in handles {
            let json = h.join().unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["jsonrpc"], "2.0");
        }
    }

    #[test]
    fn test_mixed_parse_and_format_threads() {
        let json_input = r#"{"jsonrpc":"2.0","id":99,"method":"concurrent/test"}"#;
        let json = Arc::new(json_input.to_string());
        let mut handles = Vec::new();

        for i in 0..4 {
            let j = Arc::clone(&json);
            handles.push(thread::spawn(move || {
                let req = parse_request(&j).unwrap();
                assert_eq!(req.id, clawdius_core::rpc::types::Id::Number(99));
                format!("parse-{}: {}", i, req.method)
            }));
        }

        for i in 0..4 {
            handles.push(thread::spawn(move || {
                let resp = Response::success(
                    clawdius_core::rpc::types::Id::Number(i),
                    serde_json::json!({"thread": i}),
                );
                let json = format_response(&resp);
                assert!(json.contains(r#""id":"#));
                format!("format-{i}: ok")
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
