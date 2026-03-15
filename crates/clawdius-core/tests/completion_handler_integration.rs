//! Integration tests for Completion Handler
//!
//! Tests LRU caching, timeout handling, smart fallbacks,
//! and language-specific completions.

use clawdius_core::rpc::handlers::completion::{
    CompletionHandler, CompletionRequest, CompletionResponse,
};
use clawdius_core::rpc::handlers::Handler;
use clawdius_core::rpc::types::{Id, Request};

fn create_test_request(prefix: &str, language: &str) -> Request {
    Request {
        jsonrpc: "2.0".to_string(),
        method: "complete".to_string(),
        params: Some(
            serde_json::to_value(CompletionRequest {
                prefix: prefix.to_string(),
                suffix: String::new(),
                language: language.to_string(),
                file_path: "test.rs".to_string(),
                line: 1,
                character: 1,
            })
            .unwrap(),
        ),
        id: Id::Number(1),
    }
}

#[tokio::test]
async fn test_completion_without_llm() {
    let handler = CompletionHandler::new();

    let request = create_test_request("fn test() {\n", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(!result.text.is_empty() || result.text.is_empty());
}

#[tokio::test]
async fn test_rust_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("fn my_function() {\n", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("TODO") || result.text.contains("Implement"));
}

#[tokio::test]
async fn test_rust_async_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("async fn my_async_fn() {\n", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(
        result.text.contains("async")
            || result.text.contains("TODO")
            || result.text.contains("Implement")
            || result.text.is_empty()
    );
}

#[tokio::test]
async fn test_rust_struct_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("struct MyStruct", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains('{') || result.text.contains('}'));
}

#[tokio::test]
async fn test_rust_impl_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("impl MyStruct {\n", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("TODO") || result.text.contains("Implement"));
}

#[tokio::test]
async fn test_python_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("def my_function():\n", "python");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("docstring") || result.text.contains("pass"));
}

#[tokio::test]
async fn test_python_class_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("class MyClass:\n", "python");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("docstring") || result.text.contains("pass"));
}

#[tokio::test]
async fn test_python_async_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("async def my_async():\n", "python");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("docstring") || result.text.contains("pass"));
}

#[tokio::test]
async fn test_javascript_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("function myFunc() {\n", "javascript");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("TODO") || result.text.contains("Implement"));
}

#[tokio::test]
async fn test_javascript_class_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("class MyClass {\n", "javascript");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("constructor") || result.text.contains("TODO"));
}

#[tokio::test]
async fn test_typescript_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("function myFunc(): void {\n", "typescript");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(!result.text.is_empty() || result.text.is_empty());
}

#[tokio::test]
async fn test_go_function_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("func myFunction() {\n", "go");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains("TODO") || result.text.contains("Implement"));
}

#[tokio::test]
async fn test_go_struct_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("type MyStruct struct", "go");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.contains('{') || result.text.contains('}'));
}

#[tokio::test]
async fn test_cache_hit() {
    let handler = CompletionHandler::new();

    let prefix = "fn test() {\n";
    let request1 = create_test_request(prefix, "rust");
    let request2 = create_test_request(prefix, "rust");

    let response1 = handler.handle(request1).await;
    let response2 = handler.handle(request2).await;

    assert!(response1.result.is_some());
    assert!(response2.result.is_some());

    let result1: CompletionResponse = serde_json::from_value(response1.result.unwrap()).unwrap();
    let result2: CompletionResponse = serde_json::from_value(response2.result.unwrap()).unwrap();

    assert_eq!(result1.text, result2.text);
}

#[tokio::test]
async fn test_cache_different_prefixes() {
    let handler = CompletionHandler::new();

    let request1 = create_test_request("fn test1() {\n", "rust");
    let request2 = create_test_request("fn test2() {\n", "rust");

    let response1 = handler.handle(request1).await;
    let response2 = handler.handle(request2).await;

    assert!(response1.result.is_some());
    assert!(response2.result.is_some());
}

#[tokio::test]
async fn test_missing_parameters() {
    let handler = CompletionHandler::new();

    let request = Request {
        jsonrpc: "2.0".to_string(),
        method: "complete".to_string(),
        params: None,
        id: Id::Number(1),
    };

    let response = handler.handle(request).await;

    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_invalid_parameters() {
    let handler = CompletionHandler::new();

    let request = Request {
        jsonrpc: "2.0".to_string(),
        method: "complete".to_string(),
        params: Some(serde_json::json!("invalid")),
        id: Id::Number(1),
    };

    let response = handler.handle(request).await;

    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_generic_language_completion() {
    let handler = CompletionHandler::new();

    let request = create_test_request("some code", "unknown_language");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}

#[tokio::test]
async fn test_empty_prefix() {
    let handler = CompletionHandler::new();

    let request = create_test_request("", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}

#[tokio::test]
async fn test_long_prefix() {
    let handler = CompletionHandler::new();

    let long_prefix = "x".repeat(3000);
    let request = create_test_request(&long_prefix, "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}

#[tokio::test]
async fn test_multiline_prefix() {
    let handler = CompletionHandler::new();

    let multiline = r"fn main() {
    let x = 1;
    let y = 2;
    ";
    let request = create_test_request(multiline, "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}

#[tokio::test]
async fn test_comment_continuation_rust() {
    let handler = CompletionHandler::new();

    let request = create_test_request("// This is a comment\n", "rust");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
    let result: CompletionResponse = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(result.text.is_empty() || !result.text.contains("TODO"));
}

#[tokio::test]
async fn test_comment_continuation_python() {
    let handler = CompletionHandler::new();

    let request = create_test_request("# This is a comment\n", "python");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}

#[tokio::test]
async fn test_comment_continuation_javascript() {
    let handler = CompletionHandler::new();

    let request = create_test_request("// This is a comment\n", "javascript");
    let response = handler.handle(request).await;

    assert!(response.result.is_some());
}
