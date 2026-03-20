//! Integration tests for tool execution flow through the agentic system.

use async_trait::async_trait;
use clawdius_core::agentic::tool_executor::{
    NoOpToolExecutor, ToolDefinition, ToolExecutor, ToolRequest, ToolResult,
};
use clawdius_core::agentic::{AgenticSystem, ApplyWorkflow, GenerationMode, TestExecutionStrategy};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A mock tool executor that records calls and returns configurable results.
struct MockToolExecutor {
    calls: RwLock<Vec<ToolRequest>>,
    responses: RwLock<HashMap<String, ToolResult>>,
}

impl MockToolExecutor {
    fn new() -> Self {
        Self {
            calls: RwLock::new(Vec::new()),
            responses: RwLock::new(HashMap::new()),
        }
    }

    fn set_response(&self, tool_name: &str, result: ToolResult) {
        let mut responses = self.responses.write().unwrap();
        responses.insert(tool_name.to_string(), result);
    }

    fn get_calls(&self) -> Vec<ToolRequest> {
        self.calls.read().unwrap().clone()
    }

    fn clear_calls(&self) {
        let mut calls = self.calls.write().unwrap();
        calls.clear();
    }
}

#[async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute(&self, request: ToolRequest) -> clawdius_core::error::Result<ToolResult> {
        // Record the call
        {
            let mut calls = self.calls.write().unwrap();
            calls.push(request.clone());
        }

        // Return configured response or default
        let responses = self.responses.read().unwrap();
        Ok(responses
            .get(&request.name)
            .cloned()
            .unwrap_or_else(|| ToolResult::success(format!("Executed: {}", request.name))))
    }

    fn has_tool(&self, name: &str) -> bool {
        matches!(name, "search_ast" | "semantic_search" | "get_call_graph")
    }

    fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition::new("search_ast", "Search AST nodes"),
            ToolDefinition::new("semantic_search", "Semantic code search"),
            ToolDefinition::new("get_call_graph", "Get function call graph"),
        ]
    }
}

#[tokio::test]
async fn test_agentic_system_with_noop_executor() {
    let executor = Arc::new(NoOpToolExecutor);

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_tool_executor(executor);

    // Verify tool executor is set
    assert!(system.tool_executor().is_some());

    // Verify it has no tools (NoOp returns false for has_tool)
    let tool_executor = system.tool_executor().unwrap();
    assert!(!tool_executor.has_tool("any_tool"));
    assert!(tool_executor.list_tools().is_empty());
}

#[tokio::test]
async fn test_agentic_system_with_mock_executor() {
    let executor = Arc::new(MockToolExecutor::new());

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_tool_executor(executor);

    // Verify tool executor is set
    assert!(system.tool_executor().is_some());

    // Verify tools are available
    let tool_executor = system.tool_executor().unwrap();
    assert!(tool_executor.has_tool("search_ast"));
    assert!(tool_executor.has_tool("semantic_search"));
    assert!(!tool_executor.has_tool("nonexistent"));

    let tools = tool_executor.list_tools();
    assert_eq!(tools.len(), 3);
}

#[tokio::test]
async fn test_tool_executor_direct_execution() {
    let executor = Arc::new(MockToolExecutor::new());

    // Configure a response
    executor.set_response(
        "search_ast",
        ToolResult::success(r#"{"results": [{"name": "main", "type": "function"}]}"#),
    );

    // Execute the tool
    let request = ToolRequest::new("search_ast")
        .with_arg("node_type", serde_json::json!("function"))
        .with_arg("limit", serde_json::json!(10));

    let result = executor.execute(request).await.unwrap();

    assert!(result.success);
    assert!(result.content.contains("results"));

    // Verify the call was recorded
    let calls = executor.get_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "search_ast");
}

#[tokio::test]
async fn test_tool_executor_error_handling() {
    let executor = Arc::new(MockToolExecutor::new());

    // Configure an error response
    executor.set_response(
        "failing_tool",
        ToolResult::error("Tool execution failed: invalid arguments"),
    );

    let request = ToolRequest::new("failing_tool");
    let result = executor.execute(request).await.unwrap();

    assert!(!result.success);
    assert!(result.is_error);
    assert!(result.content.contains("failed"));
}

#[tokio::test]
async fn test_tool_result_json_parsing() {
    let result = ToolResult::success(r#"{"count": 42, "items": ["a", "b"]}"#);

    #[derive(serde::Deserialize)]
    struct TestResponse {
        count: usize,
        items: Vec<String>,
    }

    let parsed: TestResponse = result.parse_json().unwrap();
    assert_eq!(parsed.count, 42);
    assert_eq!(parsed.items.len(), 2);
}

#[tokio::test]
async fn test_tool_result_json_parsing_error() {
    let result = ToolResult::success("not valid json");

    #[derive(serde::Deserialize)]
    struct TestResponse {
        count: usize,
    }

    let parsed = result.parse_json::<TestResponse>();
    assert!(parsed.is_err());
}

#[tokio::test]
async fn test_multiple_tool_executions() {
    let executor = Arc::new(MockToolExecutor::new());

    // Execute multiple tools
    let tools = ["search_ast", "semantic_search", "get_call_graph"];

    for tool in &tools {
        let request = ToolRequest::new(*tool);
        let result = executor.execute(request).await.unwrap();
        assert!(result.success);
    }

    // Verify all calls were recorded
    let calls = executor.get_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].name, "search_ast");
    assert_eq!(calls[1].name, "semantic_search");
    assert_eq!(calls[2].name, "get_call_graph");
}

#[tokio::test]
async fn test_tool_executor_with_complex_arguments() {
    let executor = Arc::new(MockToolExecutor::new());

    let request = ToolRequest::new("search_ast")
        .with_arg("node_type", serde_json::json!("function"))
        .with_arg("name_pattern", serde_json::json!("handle_*"))
        .with_arg("file_path", serde_json::json!("/src/main.rs"))
        .with_arg("limit", serde_json::json!(25))
        .with_arg(
            "options",
            serde_json::json!({
                "case_sensitive": false,
                "include_private": true
            }),
        );

    let result = executor.execute(request).await.unwrap();
    assert!(result.success);

    // Verify the complex arguments were recorded
    let calls = executor.get_calls();
    assert_eq!(calls.len(), 1);

    let args = &calls[0].arguments;
    assert_eq!(args.get("node_type").unwrap(), "function");
    assert_eq!(args.get("name_pattern").unwrap(), "handle_*");
    assert_eq!(args.get("limit").unwrap(), 25);

    let options = args.get("options").unwrap().as_object().unwrap();
    assert_eq!(options.get("case_sensitive").unwrap(), false);
    assert_eq!(options.get("include_private").unwrap(), true);
}

#[tokio::test]
async fn test_tool_request_with_string_args() {
    let mut string_args = HashMap::new();
    string_args.insert("path".to_string(), "/src/lib.rs".to_string());
    string_args.insert("mode".to_string(), "read".to_string());

    let request = ToolRequest::with_string_args("file_operation", string_args);

    assert_eq!(request.name, "file_operation");
    assert_eq!(
        request.arguments.get("path").unwrap(),
        &serde_json::json!("/src/lib.rs")
    );
    assert_eq!(
        request.arguments.get("mode").unwrap(),
        &serde_json::json!("read")
    );
}

#[tokio::test]
async fn test_tool_definition_with_schema() {
    let definition =
        ToolDefinition::new("test_tool", "A test tool").with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100
                }
            },
            "required": ["query"]
        }));

    assert_eq!(definition.name, "test_tool");
    assert_eq!(definition.description, "A test tool");

    let schema = definition.input_schema.as_object().unwrap();
    assert!(schema.contains_key("properties"));
    assert!(schema.contains_key("required"));
}

#[tokio::test]
async fn test_agentic_system_without_tool_executor() {
    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    );

    // Verify no tool executor is set
    assert!(system.tool_executor().is_none());
}

#[tokio::test]
async fn test_concurrent_tool_execution() {
    let executor = Arc::new(MockToolExecutor::new());

    // Execute tools concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let exec = Arc::clone(&executor);
            tokio::spawn(async move {
                let request = ToolRequest::new("search_ast").with_arg("id", serde_json::json!(i));
                exec.execute(request).await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        assert!(result.success);
    }

    // Verify all calls were recorded (thread-safe)
    let calls = executor.get_calls();
    assert_eq!(calls.len(), 10);
}
