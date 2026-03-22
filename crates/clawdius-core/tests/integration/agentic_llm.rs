//! Integration tests for agentic system with LLM integration.
//!
//! This test file verifies that the agentic system works correctly
//! when an LLM client is configured, and uses stubs when not.

use async_trait::async_trait;
use clawdius_core::agentic::tool_executor::NoOpToolExecutor;
use clawdius_core::agentic::{
    AgenticSystem, ApplyWorkflow, GenerationMode, TaskContext, TaskRequest, TestExecutionStrategy,
    TrustLevel,
};
use clawdius_core::error::Result;
use clawdius_core::llm::{ChatMessage, LlmClient};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Mock LLM client for testing agentic system.
struct MockLlmClient {
    responses: Vec<String>,
    call_count: AtomicUsize,
}

impl MockLlmClient {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            call_count: AtomicUsize::new(0),
        }
    }

    fn single(response: &str) -> Self {
        Self::new(vec![response.to_string()])
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count < self.responses.len() {
            Ok(self.responses[count].clone())
        } else if !self.responses.is_empty() {
            Ok("// No response configured".to_string())
        } else {
            Ok(self.responses.last().unwrap().clone())
        }
    }

    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        let response = self.chat(messages).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let _ = tx.send(response).await;
        Ok(rx)
    }

    fn count_tokens(&self, _text: &str) -> usize {
        100
    }
}

#[tokio::test]
async fn test_agentic_system_with_llm_client() {
    let llm_client = Arc::new(MockLlmClient::single(
        "fn hello_world() { println!(\"Hello, World!\"); }",
    ));

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client.clone());

    assert!(system.llm_client().is_some());
}

#[tokio::test]
async fn test_agentic_system_without_llm_client() {
    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    );

    assert!(system.llm_client().is_none());
}

#[tokio::test]
async fn test_executor_agent_with_llm() {
    use clawdius_core::agentic::ExecutorAgent;

    let llm_client = Arc::new(MockLlmClient::single(
        "fn generated_function() -> i32 { 42 }",
    ));

    let executor = ExecutorAgent::new().with_llm_client(llm_client, "test-model");

    assert!(executor.has_llm_client());
    assert!(!executor.has_tool_executor());
}

#[tokio::test]
async fn test_code_generator_with_llm() {
    let llm_client = Arc::new(MockLlmClient::single(
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    ));

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client.clone());

    let generator = system.code_generator();
    assert!(generator.is_some());
}

#[tokio::test]
async fn test_generate_code_with_llm() {
    let expected_code = r#"
pub fn calculate_sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}
"#;

    let llm_client = Arc::new(MockLlmClient::single(expected_code));

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client.clone());

    let result = system
        .generate_code("Write a function to sum numbers", Some("src/math.rs"), None)
        .await;

    assert!(result.is_ok());
    let generated = result.unwrap();
    assert!(generated.is_some());
    let code = generated.unwrap();
    assert!(code.content.contains("calculate_sum"));
}

#[tokio::test]
async fn test_generate_code_without_llm_returns_none() {
    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    );

    let result = system
        .generate_code("Write a function", Some("src/lib.rs"), None)
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_agentic_system_with_both_executors() {
    let llm_client = Arc::new(MockLlmClient::single("generated code"));
    let tool_executor = Arc::new(NoOpToolExecutor);

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client)
    .with_tool_executor(tool_executor);

    assert!(system.llm_client().is_some());
    assert!(system.tool_executor().is_some());
}

#[tokio::test]
async fn test_task_execution_with_llm() {
    let llm_response = r#"
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;

    let llm_client = Arc::new(MockLlmClient::single(llm_response));

    let mut system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client);

    let request = TaskRequest {
        id: "test-1".to_string(),
        description: "Add a greet function".to_string(),
        target_files: vec!["src/lib.rs".to_string()],
        mode: GenerationMode::single_pass(),
        test_strategy: TestExecutionStrategy::skip(),
        apply_workflow: ApplyWorkflow::trust_based(),
        context: TaskContext::default(),
        trust_level: TrustLevel::Medium,
    };

    let result = system.execute(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_iterative_generation_with_llm() {
    let responses = vec![
        "fn first_draft() {}".to_string(),
        "fn second_draft() {}".to_string(),
        "fn final_version() {}".to_string(),
    ];

    let llm_client = Arc::new(MockLlmClient::new(responses));

    let mut system = AgenticSystem::new(
        GenerationMode::iterative(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client);

    let request = TaskRequest {
        id: "test-iterative".to_string(),
        description: "Create a function with iterations".to_string(),
        target_files: vec![],
        mode: GenerationMode::iterative(),
        test_strategy: TestExecutionStrategy::skip(),
        apply_workflow: ApplyWorkflow::trust_based(),
        context: TaskContext::default(),
        trust_level: TrustLevel::Medium,
    };

    let result = system.execute(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agent_based_generation_with_llm() {
    let responses = vec![
        "Analysis: The code needs a helper function.".to_string(),
        "Design: Create a utility module.".to_string(),
        "fn helper() {}".to_string(),
        "Verification: All tests pass.".to_string(),
    ];

    let llm_client = Arc::new(MockLlmClient::new(responses));

    let mut system = AgenticSystem::new(
        GenerationMode::agent_based(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client);

    let request = TaskRequest {
        id: "test-agent".to_string(),
        description: "Implement a feature autonomously".to_string(),
        target_files: vec!["src/lib.rs".to_string()],
        mode: GenerationMode::agent_based(),
        test_strategy: TestExecutionStrategy::skip(),
        apply_workflow: ApplyWorkflow::trust_based(),
        context: TaskContext::default(),
        trust_level: TrustLevel::Medium,
    };

    let result = system.execute(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_llm_client_call_tracking() {
    let llm_client = Arc::new(MockLlmClient::new(vec![
        "first response".to_string(),
        "second response".to_string(),
    ]));

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(Arc::clone(&llm_client) as Arc<dyn LlmClient>);

    let _ = system.generate_code("test prompt", None, None).await;

    assert_eq!(llm_client.call_count(), 1);
}

#[tokio::test]
async fn test_agentic_system_handles_empty_llm_response() {
    let llm_client = Arc::new(MockLlmClient::single(""));

    let system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client);

    let result = system.generate_code("test", Some("src/lib.rs"), None).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_agentic_system_with_context() {
    let llm_client = Arc::new(MockLlmClient::single("// Context-aware code generation"));

    let mut system = AgenticSystem::new(
        GenerationMode::single_pass(),
        TestExecutionStrategy::skip(),
        ApplyWorkflow::trust_based(),
    )
    .with_llm_client(llm_client);

    let context = TaskContext {
        related_files: vec!["src/types.rs".to_string()],
        conversation_history: vec!["Previous discussion about types".to_string()],
        project_language: Some("rust".to_string()),
        project_framework: None,
        constraints: vec!["Must be async".to_string()],
    };

    let request = TaskRequest {
        id: "test-context".to_string(),
        description: "Generate async function".to_string(),
        target_files: vec!["src/lib.rs".to_string()],
        mode: GenerationMode::single_pass(),
        test_strategy: TestExecutionStrategy::skip(),
        apply_workflow: ApplyWorkflow::trust_based(),
        context,
        trust_level: TrustLevel::Medium,
    };

    let result = system.execute(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_different_trust_levels() {
    let llm_client = Arc::new(MockLlmClient::single("fn test() {}"));

    for trust_level in [TrustLevel::Low, TrustLevel::Medium, TrustLevel::High] {
        let mut system = AgenticSystem::new(
            GenerationMode::single_pass(),
            TestExecutionStrategy::skip(),
            ApplyWorkflow::trust_based_with_level(trust_level, trust_level != TrustLevel::High),
        )
        .with_llm_client(Arc::clone(&llm_client) as Arc<dyn LlmClient>);

        let request = TaskRequest {
            id: format!("test-trust-{:?}", trust_level),
            description: "Test task".to_string(),
            target_files: vec![],
            mode: GenerationMode::single_pass(),
            test_strategy: TestExecutionStrategy::skip(),
            apply_workflow: ApplyWorkflow::trust_based_with_level(
                trust_level,
                trust_level != TrustLevel::High,
            ),
            context: TaskContext::default(),
            trust_level,
        };

        let result = system.execute(request).await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_different_test_strategies() {
    let llm_client = Arc::new(MockLlmClient::single("fn test() {}"));

    for test_strategy in [
        TestExecutionStrategy::skip(),
        TestExecutionStrategy::sandboxed(),
        TestExecutionStrategy::direct_with_rollback(),
    ] {
        let mut system = AgenticSystem::new(
            GenerationMode::single_pass(),
            test_strategy.clone(),
            ApplyWorkflow::trust_based(),
        )
        .with_llm_client(Arc::clone(&llm_client) as Arc<dyn LlmClient>);

        let request = TaskRequest {
            id: format!("test-strategy-{:?}", test_strategy),
            description: "Test task".to_string(),
            target_files: vec![],
            mode: GenerationMode::single_pass(),
            test_strategy: test_strategy.clone(),
            apply_workflow: ApplyWorkflow::trust_based(),
            context: TaskContext::default(),
            trust_level: TrustLevel::Medium,
        };

        let result = system.execute(request).await;
        assert!(result.is_ok());
    }
}
