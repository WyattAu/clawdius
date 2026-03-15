use clawdius_core::rpc::{Id, Request, Response, RpcError};

#[tokio::test]
async fn test_rpc_request_response_cycle() {
    let request = Request::new(1, "session/create").with_params(serde_json::json!({
        "title": "Test Session",
        "provider": "anthropic"
    }));

    let json = serde_json::to_string(&request).expect("Failed to serialize request");
    assert!(json.contains("session/create"));
    assert!(json.contains("Test Session"));

    let parsed: Request = Request::from_json(&json).expect("Failed to parse request");
    assert_eq!(parsed.method, "session/create");
    assert_eq!(parsed.id, Id::Number(1));

    let response = Response::success(
        Id::Number(1),
        serde_json::json!({
            "sessionId": "550e8400-e29b-41d4-a716-446655440000",
            "status": "created"
        }),
    );

    let response_json = response.to_json().expect("Failed to serialize response");
    assert!(response_json.contains("sessionId"));

    let parsed_response = Response::from_json(&response_json).expect("Failed to parse response");
    assert!(parsed_response.result.is_some());
    assert!(parsed_response.error.is_none());
}

#[tokio::test]
async fn test_rpc_error_handling() {
    let malformed = "{ not valid json }";
    let result = Request::from_json(malformed);
    assert!(result.is_err());

    let response = Response::error(Id::Number(1), RpcError::parse_error("Invalid JSON"));
    assert!(response.result.is_none());
    assert!(response.error.is_some());
    let err = response.error.unwrap();
    assert_eq!(err.code, -32700);

    let response = Response::method_not_found(Id::Number(2), "unknown/method");
    let err = response.error.as_ref().expect("Should have error");
    assert_eq!(err.code, -32601);

    let response = Response::invalid_params(Id::Number(3), "Missing required field: title");
    let err = response.error.as_ref().expect("Should have error");
    assert_eq!(err.code, -32602);

    let response = Response::internal_error(Id::Number(4), "Database connection failed");
    let err = response.error.as_ref().expect("Should have error");
    assert_eq!(err.code, -32603);
}

#[tokio::test]
async fn test_rpc_id_types() {
    let num_id = Id::number(42);
    let str_id = Id::string("request-abc-123");
    let null_id = Id::null();

    let req1 = Request::new(num_id.clone(), "test/numeric");
    assert_eq!(req1.id, Id::Number(42));

    let req2 = Request::new(str_id.clone(), "test/string");
    assert_eq!(req2.id, Id::String("request-abc-123".to_string()));

    assert!(null_id.is_null());
    assert!(!num_id.is_null());
}

#[tokio::test]
async fn test_rpc_request_with_complex_params() {
    let params = serde_json::json!({
        "session": {
            "id": "session-123",
            "title": "Complex Session",
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"}
            ]
        },
        "options": {
            "stream": true,
            "maxTokens": 4096,
            "temperature": 0.7
        }
    });

    let request = Request::new("complex-request", "session/update").with_params(params);

    let json = serde_json::to_string(&request).expect("Failed to serialize");
    let parsed = Request::from_json(&json).expect("Failed to parse");

    let parsed_params = parsed.params.expect("Should have params");
    assert_eq!(parsed_params["session"]["title"], "Complex Session");
    assert_eq!(parsed_params["options"]["maxTokens"], 4096);
}

#[tokio::test]
async fn test_rpc_response_serialization() {
    let response = Response::success(
        Id::String("test-123".to_string()),
        serde_json::json!({
            "data": {
                "items": [1, 2, 3],
                "nested": {"key": "value"}
            }
        }),
    );

    let json = response.to_json_pretty().expect("Failed to serialize");
    assert!(json.contains("\"jsonrpc\": \"2.0\""));
    assert!(json.contains("\"id\": \"test-123\""));

    let parsed = Response::from_json(&json).expect("Failed to parse");
    let result = parsed.result.expect("Should have result");
    assert_eq!(result["data"]["items"], serde_json::json!([1, 2, 3]));
}

#[tokio::test]
async fn test_rpc_error_with_data() {
    let error = RpcError::invalid_params("Validation failed").with_data(serde_json::json!({
        "field": "email",
        "error": "Invalid email format"
    }));

    let response = Response::error(Id::Number(1), error);

    let err = response.error.expect("Should have error");
    assert_eq!(err.code, -32602);
    assert!(err.data.is_some());

    let data = err.data.unwrap();
    assert_eq!(data["field"], "email");
}

#[tokio::test]
async fn test_rpc_multiple_requests() {
    let requests = [
        Request::new(1, "session/list"),
        Request::new(2, "session/get").with_params(serde_json::json!({"id": "session-1"})),
        Request::new(3, "session/delete").with_params(serde_json::json!({"id": "session-1"})),
    ];

    let responses: Vec<Response> = requests
        .iter()
        .map(|req| match req.method.as_str() {
            "session/list" => Response::success(req.id.clone(), serde_json::json!([])),
            "session/get" => Response::success(
                req.id.clone(),
                serde_json::json!({"id": "session-1", "title": "Test"}),
            ),
            "session/delete" => {
                Response::success(req.id.clone(), serde_json::json!({"deleted": true}))
            }
            _ => Response::method_not_found(req.id.clone(), &req.method),
        })
        .collect();

    assert_eq!(responses.len(), 3);
    assert!(responses.iter().all(|r| r.error.is_none()));
}
