//! Integration Tests for Clawdius
//!
//! Tests for LLM wiring, tools, streaming, and configuration.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    unsafe_code
)]

use clawdius_core::config::{
    Config, OutputConfig, RetryCondition, RetryConfig, ShellSandboxConfig,
};
use clawdius_core::llm::{create_provider, LlmConfig, LlmProvider};
use clawdius_core::output::{
    stream::{ChangeType, StreamWriter},
    OutputFormat, StreamEvent,
};
use clawdius_core::tools::file::{
    FileEditParams, FileListParams, FileReadParams, FileTool, FileWriteParams,
};
use clawdius_core::tools::git::{GitDiffParams, GitLogParams, GitTool};
use clawdius_core::tools::shell::{ShellParams, ShellTool};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// LLM Integration Tests
// ============================================================================

#[test]
fn test_llm_config_from_env_anthropic() {
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "test-anthropic-key");
    }
    let config = LlmConfig::from_env("anthropic").unwrap();
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-3-5-sonnet-20241022");
    assert_eq!(config.api_key, Some("test-anthropic-key".to_string()));
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
}

#[test]
fn test_llm_config_from_env_openai() {
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-openai-key");
    }
    let config = LlmConfig::from_env("openai").unwrap();
    assert_eq!(config.provider, "openai");
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(config.api_key, Some("test-openai-key".to_string()));
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}

#[test]
fn test_llm_config_from_env_ollama() {
    unsafe {
        std::env::set_var("OLLAMA_BASE_URL", "http://localhost:11434");
    }
    let config = LlmConfig::from_env("ollama").unwrap();
    assert_eq!(config.provider, "ollama");
    assert_eq!(config.model, "llama3.2");
    assert_eq!(config.base_url, Some("http://localhost:11434".to_string()));
    unsafe {
        std::env::remove_var("OLLAMA_BASE_URL");
    }
}

#[test]
fn test_llm_config_missing_key() {
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
    let result = LlmConfig::from_env("anthropic");
    assert!(result.is_err());
}

#[test]
fn test_create_provider_factory_anthropic() {
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    }
    let config = LlmConfig::from_env("anthropic").unwrap();
    let provider = create_provider(&config).unwrap();
    match provider {
        LlmProvider::Anthropic(_) => (),
        _ => panic!("Expected Anthropic provider"),
    }
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
}

#[test]
fn test_create_provider_factory_openai() {
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-key");
    }
    let config = LlmConfig::from_env("openai").unwrap();
    let provider = create_provider(&config).unwrap();
    match provider {
        LlmProvider::OpenAi(_) => (),
        _ => panic!("Expected OpenAI provider"),
    }
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}

#[test]
fn test_create_provider_factory_ollama() {
    let config = LlmConfig {
        provider: "ollama".to_string(),
        model: "llama3.2".to_string(),
        api_key: None,
        base_url: Some("http://localhost:11434".to_string()),
        max_tokens: 4096,
    };
    let provider = create_provider(&config).unwrap();
    match provider {
        LlmProvider::Ollama(_) => (),
        _ => panic!("Expected Ollama provider"),
    }
}

#[test]
fn test_create_provider_factory_unknown() {
    let config = LlmConfig {
        provider: "unknown".to_string(),
        model: "test".to_string(),
        api_key: None,
        base_url: None,
        max_tokens: 4096,
    };
    let result = create_provider(&config);
    assert!(result.is_err());
}

// ============================================================================
// File Tool Tests
// ============================================================================

#[test]
fn test_file_tool_write_and_read() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let path_str = file_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "Hello, World!".to_string(),
    })
    .unwrap();

    let content = tool
        .read(FileReadParams {
            path: path_str,
            offset: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(content, "Hello, World!");
}

#[test]
fn test_file_tool_read_with_offset() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let path_str = file_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "line1\nline2\nline3\nline4".to_string(),
    })
    .unwrap();

    let content = tool
        .read(FileReadParams {
            path: path_str,
            offset: Some(1),
            limit: Some(2),
        })
        .unwrap();

    assert_eq!(content, "line2\nline3");
}

#[test]
fn test_file_tool_edit() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let path_str = file_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "Hello, World!".to_string(),
    })
    .unwrap();

    let edited = tool
        .edit(FileEditParams {
            path: path_str.clone(),
            old_string: "World".to_string(),
            new_string: "Rust".to_string(),
            replace_all: false,
        })
        .unwrap();

    assert!(edited);

    let content = tool
        .read(FileReadParams {
            path: path_str,
            offset: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(content, "Hello, Rust!");
}

#[test]
fn test_file_tool_edit_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let path_str = file_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "Hello, World!".to_string(),
    })
    .unwrap();

    let edited = tool
        .edit(FileEditParams {
            path: path_str,
            old_string: "NotFound".to_string(),
            new_string: "Replaced".to_string(),
            replace_all: false,
        })
        .unwrap();

    assert!(!edited);
}

#[test]
fn test_file_tool_list() {
    let temp_dir = TempDir::new().unwrap();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: temp_dir.path().join("a.txt").to_string_lossy().to_string(),
        content: "a".to_string(),
    })
    .unwrap();

    tool.write(FileWriteParams {
        path: temp_dir.path().join("b.txt").to_string_lossy().to_string(),
        content: "b".to_string(),
    })
    .unwrap();

    let entries = tool
        .list(FileListParams {
            path: temp_dir.path().to_string_lossy().to_string(),
        })
        .unwrap();

    assert!(entries.contains(&"a.txt".to_string()));
    assert!(entries.contains(&"b.txt".to_string()));
}

#[test]
fn test_file_tool_read_nonexistent() {
    let tool = FileTool::new();
    let result = tool.read(FileReadParams {
        path: "/nonexistent/file.txt".to_string(),
        offset: None,
        limit: None,
    });
    assert!(result.is_err());
}

// ============================================================================
// Shell Tool Tests
// ============================================================================

#[test]
fn test_shell_tool_basic_command() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = ShellTool::new(config, current_dir);

    let result = tool
        .execute(ShellParams {
            command: "echo hello".to_string(),
            timeout: 5000,
            cwd: None,
        })
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello"));
    assert!(!result.timed_out);
}

#[test]
fn test_shell_tool_exit_code() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = ShellTool::new(config, current_dir);

    let result = tool
        .execute(ShellParams {
            command: "exit 42".to_string(),
            timeout: 5000,
            cwd: None,
        })
        .unwrap();

    assert_eq!(result.exit_code, 42);
}

#[test]
fn test_shell_tool_blocked_command() {
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, PathBuf::from("."));

    let result = tool.execute(ShellParams {
        command: "rm -rf /".to_string(),
        timeout: 5000,
        cwd: None,
    });

    assert!(result.is_err());
}

// ============================================================================
// Git Tool Tests
// ============================================================================

#[test]
fn test_git_tool_status() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = GitTool::new(config, current_dir);

    let result = tool.status(None);
    assert!(result.is_ok());

    let status = result.unwrap();
    assert!(
        !status.is_empty() || status.contains("On branch") || status.contains("nothing to commit")
    );
}

#[test]
fn test_git_tool_log() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = GitTool::new(config, current_dir);

    let result = tool.log(
        GitLogParams {
            count: 5,
            path: None,
        },
        None,
    );

    assert!(result.is_ok());
}

#[test]
fn test_git_tool_diff() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = GitTool::new(config, current_dir);

    let result = tool.diff(
        GitDiffParams {
            staged: false,
            path: None,
        },
        None,
    );

    assert!(result.is_ok());
}

// ============================================================================
// Config Loading Tests
// ============================================================================

#[test]
fn test_config_load_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let config_content = r#"
[project]
name = "test-project"
rigor_level = "high"
lifecycle_phase = "development"

[storage]
database_path = ".clawdius/graph/index.db"
vector_path = ".clawdius/graph/vectors.lance"
sessions_path = ".clawdius/sessions.db"

[llm]
default_provider = "anthropic"
max_tokens = 2048
"#;

    std::fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();

    assert_eq!(config.project.name, "test-project");
    assert_eq!(config.project.rigor_level, "high");
    assert_eq!(config.llm.default_provider, Some("anthropic".to_string()));
    assert_eq!(config.llm.max_tokens, 2048);
}

#[test]
fn test_config_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let mut config = Config::default();
    config.project.name = "saved-project".to_string();
    config.project.rigor_level = "medium".to_string();

    config.save(&config_path).unwrap();

    let loaded = Config::load(&config_path).unwrap();

    assert_eq!(loaded.project.name, "saved-project");
    assert_eq!(loaded.project.rigor_level, "medium");
}

#[test]
fn test_config_defaults() {
    let config = Config::default();

    assert_eq!(config.project.name, "clawdius");
    assert_eq!(config.project.rigor_level, "high");
    assert_eq!(config.project.lifecycle_phase, "context_discovery");
    assert!(config.session.auto_save);
}

#[test]
fn test_config_retry_config() {
    let retry = RetryConfig::default();

    assert_eq!(retry.max_retries, 3);
    assert_eq!(retry.initial_delay_ms, 1000);
    assert_eq!(retry.max_delay_ms, 30000);
    assert_eq!(retry.exponential_base, 2.0);
    assert!(retry.retry_on.contains(&RetryCondition::RateLimit));
    assert!(retry.retry_on.contains(&RetryCondition::Timeout));
    assert!(retry.retry_on.contains(&RetryCondition::ServerError));
    assert!(retry.retry_on.contains(&RetryCondition::NetworkError));
}

#[test]
fn test_config_env_override() {
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "env-override-key");
    }

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let config_content = r#"
[project]
name = "test"
rigor_level = "high"
lifecycle_phase = "development"

[storage]
database_path = ".clawdius/graph/index.db"
vector_path = ".clawdius/graph/vectors.lance"
sessions_path = ".clawdius/sessions.db"

[llm]
default_provider = "openai"
max_tokens = 4096
"#;

    std::fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();
    let llm_config = LlmConfig::from_config(&config.llm, "openai").unwrap();

    assert_eq!(llm_config.api_key, Some("env-override-key".to_string()));

    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}

// ============================================================================
// Streaming Tests
// ============================================================================

#[test]
fn test_streaming_event_creation() {
    let event = StreamEvent::start("session-123", Some("gpt-4o".to_string()));
    match event {
        StreamEvent::Start {
            session_id, model, ..
        } => {
            assert_eq!(session_id, "session-123");
            assert_eq!(model, Some("gpt-4o".to_string()));
        }
        _ => panic!("Expected Start event"),
    }
}

#[test]
fn test_streaming_token_event() {
    let event = StreamEvent::token("Hello, world!");
    match event {
        StreamEvent::Token { content } => {
            assert_eq!(content, "Hello, world!");
        }
        _ => panic!("Expected Token event"),
    }
}

#[test]
fn test_streaming_tool_call_event() {
    let args = serde_json::json!({"path": "/test.txt"});
    let event = StreamEvent::tool_call("read_file", args.clone());
    match event {
        StreamEvent::ToolCall { name, arguments } => {
            assert_eq!(name, "read_file");
            assert_eq!(arguments, args);
        }
        _ => panic!("Expected ToolCall event"),
    }
}

#[test]
fn test_streaming_complete_event() {
    let event = StreamEvent::complete(100, 50, 1500);
    match event {
        StreamEvent::Complete { usage, duration_ms } => {
            assert_eq!(usage.input, 100);
            assert_eq!(usage.output, 50);
            assert_eq!(usage.total, 150);
            assert_eq!(duration_ms, 1500);
        }
        _ => panic!("Expected Complete event"),
    }
}

#[test]
fn test_streaming_error_event() {
    let event = StreamEvent::error("Something went wrong", "ERR_001");
    match event {
        StreamEvent::Error {
            message,
            code,
            recoverable,
        } => {
            assert_eq!(message, "Something went wrong");
            assert_eq!(code, "ERR_001");
            assert!(!recoverable);
        }
        _ => panic!("Expected Error event"),
    }
}

#[test]
fn test_streaming_recoverable_error_event() {
    let event = StreamEvent::recoverable_error("Rate limited", "ERR_RATE");
    match event {
        StreamEvent::Error {
            message,
            code,
            recoverable,
        } => {
            assert_eq!(message, "Rate limited");
            assert_eq!(code, "ERR_RATE");
            assert!(recoverable);
        }
        _ => panic!("Expected Error event"),
    }
}

#[test]
fn test_streaming_file_change_event() {
    let event = StreamEvent::file_change("/src/main.rs", ChangeType::Modified);
    match event {
        StreamEvent::FileChange { path, change_type } => {
            assert_eq!(path, "/src/main.rs");
            assert!(matches!(change_type, ChangeType::Modified));
        }
        _ => panic!("Expected FileChange event"),
    }
}

#[test]
fn test_streaming_progress_event() {
    let event = StreamEvent::progress("Processing files", 5, 10);
    match event {
        StreamEvent::Progress {
            message,
            current,
            total,
        } => {
            assert_eq!(message, "Processing files");
            assert_eq!(current, 5);
            assert_eq!(total, 10);
        }
        _ => panic!("Expected Progress event"),
    }
}

#[test]
fn test_streaming_event_to_json() {
    let event = StreamEvent::token("test content");
    let json = event.to_json_line().unwrap();

    assert!(json.contains(r#""type":"token"#));
    assert!(json.contains(r#""content":"test content"#));
    assert!(json.ends_with('\n'));
}

#[test]
fn test_streaming_writer_json_format() {
    let mut writer = StreamWriter::in_memory(OutputFormat::StreamJson);

    writer
        .write_event(&StreamEvent::start("session-abc", None))
        .unwrap();
    writer.write_event(&StreamEvent::token("Hello")).unwrap();
    writer
        .write_event(&StreamEvent::complete(10, 5, 500))
        .unwrap();

    let output = writer.into_string();

    assert!(output.contains("session-abc"));
    assert!(output.contains("Hello"));
    assert!(output.contains("complete"));
}

#[test]
fn test_streaming_writer_text_format() {
    let mut writer = StreamWriter::in_memory(OutputFormat::Text);

    writer.write_event(&StreamEvent::token("Hello")).unwrap();
    writer
        .write_event(&StreamEvent::tool_call("test_tool", serde_json::json!({})))
        .unwrap();

    let output = writer.into_string();

    assert!(output.contains("Hello"));
    assert!(output.contains("test_tool"));
}

// ============================================================================
// Tool Result Tests
// ============================================================================

#[test]
fn test_tool_result_serialization() {
    use clawdius_core::tools::ToolResult;

    let result = ToolResult {
        success: true,
        output: "File written successfully".to_string(),
        metadata: Some(serde_json::json!({"bytes": 100})),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("File written successfully"));

    let deserialized: ToolResult = serde_json::from_str(&json).unwrap();
    assert!(deserialized.success);
    assert_eq!(deserialized.output, "File written successfully");
}

#[test]
fn test_tool_result_without_metadata() {
    use clawdius_core::tools::ToolResult;

    let result = ToolResult {
        success: false,
        output: "Error: file not found".to_string(),
        metadata: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":false"));
    assert!(!json.contains("metadata"));
}

// ============================================================================
// Additional Configuration Tests
// ============================================================================

#[test]
fn test_shell_sandbox_config_defaults() {
    let config = ShellSandboxConfig::default();

    assert!(!config.blocked_commands.is_empty());
    assert!(config.blocked_commands.contains(&"rm -rf /".to_string()));
    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_output_bytes, 1_048_576);
    assert!(config.restrict_to_cwd);
}

#[test]
fn test_output_config_defaults() {
    let config = OutputConfig::default();

    // Note: Default trait uses type defaults (false for bool),
    // but serde deserialization uses default_true() for show_progress
    assert!(!config.show_progress);
}

#[test]
fn test_llm_config_serialization() {
    let config = LlmConfig {
        provider: "anthropic".to_string(),
        model: "claude-3-5-sonnet-20241022".to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        max_tokens: 4096,
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("anthropic"));
    assert!(json.contains("claude-3-5-sonnet-20241022"));

    let deserialized: LlmConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.provider, "anthropic");
}

// ============================================================================
// Generate Command Tests
// ============================================================================

mod generate_tests {
    use clawdius_core::agentic::{
        apply_workflow::{ApplyWorkflow, TrustLevel},
        generation_mode::GenerationMode,
        test_execution::TestExecutionStrategy,
        TaskContext, TaskRequest,
    };

    /// Test that TaskRequest can be created with all required fields
    #[test]
    fn test_task_request_creation() {
        let request = TaskRequest {
            id: "test-123".to_string(),
            description: "Add a hello world function".to_string(),
            target_files: vec!["src/main.rs".to_string()],
            mode: GenerationMode::SinglePass,
            test_strategy: TestExecutionStrategy::Skip,
            apply_workflow: ApplyWorkflow::trust_based_with_level(TrustLevel::Medium, true),
            context: TaskContext::default(),
            trust_level: TrustLevel::Medium,
        };

        assert_eq!(request.id, "test-123");
        assert_eq!(request.description, "Add a hello world function");
        assert_eq!(request.target_files.len(), 1);
    }

    /// Test generation mode parsing
    #[test]
    fn test_generation_mode_single_pass() {
        let mode = GenerationMode::SinglePass;
        assert!(matches!(mode, GenerationMode::SinglePass));
    }

    #[test]
    fn test_generation_mode_iterative() {
        let mode = GenerationMode::Iterative { max_iterations: 5 };
        if let GenerationMode::Iterative { max_iterations } = mode {
            assert_eq!(max_iterations, 5);
        } else {
            panic!("Expected Iterative mode");
        }
    }

    #[test]
    fn test_generation_mode_agent() {
        let mode = GenerationMode::AgentBased {
            max_steps: 10,
            autonomous: true,
        };
        if let GenerationMode::AgentBased {
            max_steps,
            autonomous,
        } = mode
        {
            assert_eq!(max_steps, 10);
            assert!(autonomous);
        } else {
            panic!("Expected AgentBased mode");
        }
    }

    /// Test trust level parsing
    #[test]
    fn test_trust_level_high() {
        let workflow = ApplyWorkflow::trust_based_with_level(TrustLevel::High, false);
        if let ApplyWorkflow::TrustBased { level, .. } = workflow {
            assert!(matches!(level, TrustLevel::High));
        } else {
            panic!("Expected TrustBased workflow");
        }
    }

    #[test]
    fn test_trust_level_medium() {
        let workflow = ApplyWorkflow::trust_based_with_level(TrustLevel::Medium, true);
        if let ApplyWorkflow::TrustBased { level, .. } = workflow {
            assert!(matches!(level, TrustLevel::Medium));
        } else {
            panic!("Expected TrustBased workflow");
        }
    }

    /// Test that low trust level has expected behavior
    #[test]
    fn test_low_trust_level() {
        let workflow = ApplyWorkflow::trust_based_with_level(TrustLevel::Low, true);
        if let ApplyWorkflow::TrustBased { level, .. } = workflow {
            assert!(matches!(level, TrustLevel::Low));
        } else {
            panic!("Expected TrustBased workflow");
        }
    }

    /// Test test execution strategies
    #[test]
    fn test_execution_strategy_skip() {
        let strategy = TestExecutionStrategy::Skip;
        assert!(matches!(strategy, TestExecutionStrategy::Skip));
    }

    #[test]
    fn test_execution_strategy_sandboxed() {
        let strategy = TestExecutionStrategy::sandboxed();
        assert!(matches!(strategy, TestExecutionStrategy::Sandboxed { .. }));
    }

    #[test]
    fn test_execution_strategy_direct() {
        let strategy = TestExecutionStrategy::direct_with_rollback();
        assert!(matches!(
            strategy,
            TestExecutionStrategy::DirectWithRollback { .. }
        ));
    }
}
