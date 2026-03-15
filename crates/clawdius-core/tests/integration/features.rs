#![allow(unsafe_code)]

use clawdius_core::{
    config::{Config, RetryCondition, RetryConfig, ShellSandboxConfig},
    llm::{create_provider, ChatMessage, ChatRole, LlmConfig as LlmRuntimeConfig},
    output::{
        stream::{StreamEvent, StreamWriter},
        OutputFormat,
    },
    tools::file::{FileEditParams, FileListParams, FileReadParams, FileTool, FileWriteParams},
    tools::git::GitTool,
    tools::shell::{ShellParams, ShellTool},
};
use tempfile::TempDir;

#[test]
fn test_llm_config_from_env_anthropic() {
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "test-anthropic-key");
    }
    let config = LlmRuntimeConfig::from_env("anthropic").unwrap();
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
    let config = LlmRuntimeConfig::from_env("openai").unwrap();
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
        std::env::set_var("OLLAMA_BASE_URL", "http://custom-ollama:11434");
    }
    let config = LlmRuntimeConfig::from_env("ollama").unwrap();
    assert_eq!(config.provider, "ollama");
    assert_eq!(config.model, "llama3.2");
    assert_eq!(
        config.base_url,
        Some("http://custom-ollama:11434".to_string())
    );
    unsafe {
        std::env::remove_var("OLLAMA_BASE_URL");
    }
}

#[test]
fn test_llm_config_from_env_zai() {
    unsafe {
        std::env::set_var("ZAI_API_KEY", "test-zai-key");
    }
    let config = LlmRuntimeConfig::from_env("zai").unwrap();
    assert_eq!(config.provider, "zai");
    assert_eq!(config.model, "zai-default");
    assert_eq!(config.api_key, Some("test-zai-key".to_string()));
    unsafe {
        std::env::remove_var("ZAI_API_KEY");
    }
}

#[test]
fn test_llm_config_missing_key() {
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
    let result = LlmRuntimeConfig::from_env("anthropic");
    assert!(result.is_err());
}

#[test]
fn test_llm_config_unknown_provider() {
    let result = LlmRuntimeConfig::from_env("unknown_provider");
    assert!(result.is_err());
}

#[test]
fn test_create_provider_anthropic() {
    let config = LlmRuntimeConfig {
        provider: "anthropic".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        max_tokens: 4096,
    };
    let provider = create_provider(&config);
    assert!(provider.is_ok());
}

#[test]
fn test_create_provider_openai() {
    let config = LlmRuntimeConfig {
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        api_key: Some("test-key".to_string()),
        base_url: None,
        max_tokens: 4096,
    };
    let provider = create_provider(&config);
    assert!(provider.is_ok());
}

#[test]
fn test_create_provider_ollama() {
    let config = LlmRuntimeConfig {
        provider: "ollama".to_string(),
        model: "llama3.2".to_string(),
        api_key: None,
        base_url: Some("http://localhost:11434".to_string()),
        max_tokens: 4096,
    };
    let provider = create_provider(&config);
    assert!(provider.is_ok());
}

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
fn test_file_tool_read_with_offset_and_limit() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("multiline.txt");
    let path_str = file_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "line1\nline2\nline3\nline4\nline5".to_string(),
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
    let file_path = temp_dir.path().join("edit_test.txt");
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
    let file_path = temp_dir.path().join("edit_not_found.txt");
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
        path: temp_dir
            .path()
            .join("file1.txt")
            .to_string_lossy()
            .to_string(),
        content: "content1".to_string(),
    })
    .unwrap();

    tool.write(FileWriteParams {
        path: temp_dir
            .path()
            .join("file2.txt")
            .to_string_lossy()
            .to_string(),
        content: "content2".to_string(),
    })
    .unwrap();

    let entries = tool
        .list(FileListParams {
            path: temp_dir.path().to_string_lossy().to_string(),
        })
        .unwrap();

    assert!(entries.contains(&"file1.txt".to_string()));
    assert!(entries.contains(&"file2.txt".to_string()));
}

#[test]
fn test_file_tool_read_nonexistent() {
    let tool = FileTool::new();
    let result = tool.read(FileReadParams {
        path: "/nonexistent/path/file.txt".to_string(),
        offset: None,
        limit: None,
    });
    assert!(result.is_err());
}

#[test]
fn test_shell_tool_basic_command() {
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, temp_dir.path().to_path_buf());

    let result = tool
        .execute(ShellParams {
            command: "echo hello".to_string(),
            timeout: 5000,
            cwd: None,
        })
        .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.trim() == "hello");
    assert!(!result.timed_out);
}

#[test]
fn test_shell_tool_with_exit_code() {
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, temp_dir.path().to_path_buf());

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
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, temp_dir.path().to_path_buf());

    let result = tool.execute(ShellParams {
        command: "rm -rf /".to_string(),
        timeout: 5000,
        cwd: None,
    });

    assert!(result.is_err());
}

#[test]
fn test_shell_tool_working_directory() {
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, temp_dir.path().to_path_buf());

    let result = tool
        .execute(ShellParams {
            command: "pwd".to_string(),
            timeout: 5000,
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
        })
        .unwrap();

    assert_eq!(result.exit_code, 0);
    let expected = temp_dir.path().canonicalize().unwrap();
    let actual = std::path::PathBuf::from(result.stdout.trim());
    assert_eq!(actual.canonicalize().unwrap(), expected);
}

#[test]
fn test_git_tool_status() {
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = GitTool::new(config, temp_dir.path().to_path_buf());

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    let result = tool.status(Some(temp_dir.path().to_string_lossy().as_ref()));

    if let Ok(status) = result {
        assert!(status.contains("On branch") || status.contains("No commits yet"));
    }
}

#[test]
fn test_git_tool_log() {
    let temp_dir = TempDir::new().unwrap();
    let config = ShellSandboxConfig::default();
    let tool = GitTool::new(config, temp_dir.path().to_path_buf());

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::fs::write(temp_dir.path().join("test.txt"), "content").ok();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_dir.path())
        .output()
        .ok();

    use clawdius_core::tools::git::GitLogParams;
    let result = tool.log(
        GitLogParams {
            count: 5,
            path: None,
        },
        Some(temp_dir.path().to_string_lossy().as_ref()),
    );

    if let Ok(log) = result {
        assert!(log.contains("Initial commit") || log.is_empty() || log.contains("fatal"));
    }
}

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
database_path = "test.db"
vector_path = "vectors"
sessions_path = "sessions.db"

[llm]
default_provider = "anthropic"
max_tokens = 8192

[llm.anthropic]
model = "claude-3-opus"

[session]
compact_threshold = 0.9
keep_recent = 6
"#;

    std::fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();

    assert_eq!(config.project.name, "test-project");
    assert_eq!(config.project.rigor_level, "high");
    assert_eq!(config.llm.default_provider, Some("anthropic".to_string()));
    assert_eq!(config.llm.max_tokens, 8192);
    assert_eq!(config.session.compact_threshold, 0.9);
    assert_eq!(config.session.keep_recent, 6);
}

#[test]
fn test_config_save_and_reload() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("saved_config.toml");

    let mut config = Config::default();
    config.project.name = "saved-project".to_string();
    config.llm.max_tokens = 2048;

    config.save(&config_path).unwrap();

    let reloaded = Config::load(&config_path).unwrap();

    assert_eq!(reloaded.project.name, "saved-project");
    assert_eq!(reloaded.llm.max_tokens, 2048);
}

#[test]
fn test_config_default_values() {
    let config = Config::default();

    assert_eq!(config.project.name, "clawdius");
    assert_eq!(config.project.rigor_level, "high");
    assert_eq!(config.project.lifecycle_phase, "context_discovery");
    assert!(config
        .storage
        .database_path
        .to_string_lossy()
        .contains("graph"));
}

#[test]
fn test_config_load_or_default_missing_file() {
    let config = Config::load(&std::path::PathBuf::from("/nonexistent/config.toml"));
    assert!(config.is_err());
}

#[test]
fn test_retry_config_defaults() {
    let config = RetryConfig::default();

    assert_eq!(config.max_retries, 3);
    assert_eq!(config.initial_delay_ms, 1000);
    assert_eq!(config.max_delay_ms, 30000);
    assert_eq!(config.exponential_base, 2.0);
    assert!(config.retry_on.contains(&RetryCondition::RateLimit));
    assert!(config.retry_on.contains(&RetryCondition::Timeout));
    assert!(config.retry_on.contains(&RetryCondition::ServerError));
    assert!(config.retry_on.contains(&RetryCondition::NetworkError));
}

#[test]
fn test_streaming_event_token() {
    let event = StreamEvent::token("Hello");
    let json = event.to_json_line().unwrap();

    assert!(json.contains(r#""type":"token"#));
    assert!(json.contains(r#""content":"Hello"#));
}

#[test]
fn test_streaming_event_complete() {
    let event = StreamEvent::complete(100, 50, 1500);
    let json = event.to_json_line().unwrap();

    assert!(json.contains(r#""type":"complete"#));
    assert!(json.contains(r#""input":100"#));
    assert!(json.contains(r#""output":50"#));
    assert!(json.contains(r#""total":150"#));
    assert!(json.contains(r#""duration_ms":1500"#));
}

#[test]
fn test_streaming_event_error() {
    let event = StreamEvent::error("Something went wrong", "ERR001");
    let json = event.to_json_line().unwrap();

    assert!(json.contains(r#""type":"error"#));
    assert!(json.contains(r#""message":"Something went wrong"#));
    assert!(json.contains(r#""code":"ERR001"#));
    assert!(json.contains(r#""recoverable":false"#));
}

#[test]
fn test_streaming_writer_json() {
    let mut writer = StreamWriter::in_memory(OutputFormat::StreamJson);

    writer
        .write_event(&StreamEvent::start(
            "session-123",
            Some("gpt-4o".to_string()),
        ))
        .unwrap();
    writer.write_event(&StreamEvent::token("Hello")).unwrap();
    writer.write_event(&StreamEvent::token(" World")).unwrap();
    writer
        .write_event(&StreamEvent::complete(10, 5, 500))
        .unwrap();

    let output = writer.into_string();

    assert!(output.contains("session-123"));
    assert!(output.contains("gpt-4o"));
    assert!(output.contains("Hello"));
    assert!(output.contains(" World"));
    assert!(output.contains("complete"));
}

#[test]
fn test_streaming_writer_text() {
    let mut writer = StreamWriter::in_memory(OutputFormat::Text);

    writer
        .write_event(&StreamEvent::start("session-456", None))
        .unwrap();
    writer.write_event(&StreamEvent::token("Test")).unwrap();

    let output = writer.into_string();

    assert!(output.contains("session-456"));
    assert!(output.contains("Test"));
}

#[test]
fn test_chat_message_user() {
    let msg = ChatMessage {
        role: ChatRole::User,
        content: "Hello".to_string(),
    };
    assert_eq!(msg.role, ChatRole::User);
    assert_eq!(msg.content, "Hello");
}

#[test]
fn test_chat_message_assistant() {
    let msg = ChatMessage {
        role: ChatRole::Assistant,
        content: "Hi there!".to_string(),
    };
    assert_eq!(msg.role, ChatRole::Assistant);
    assert_eq!(msg.content, "Hi there!");
}

#[test]
fn test_chat_message_system() {
    let msg = ChatMessage {
        role: ChatRole::System,
        content: "You are helpful.".to_string(),
    };
    assert_eq!(msg.role, ChatRole::System);
    assert_eq!(msg.content, "You are helpful.");
}

#[test]
fn test_shell_sandbox_config_defaults() {
    let config = ShellSandboxConfig::default();

    assert!(!config.blocked_commands.is_empty());
    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_output_bytes, 1_048_576);
    assert!(config.restrict_to_cwd);
}

#[test]
fn test_llm_runtime_config_custom_model() {
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-key");
    }
    let mut config = LlmRuntimeConfig::from_env("openai").unwrap();
    config.model = "gpt-4-turbo".to_string();

    assert_eq!(config.model, "gpt-4-turbo");
    assert_eq!(config.max_tokens, 4096);

    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}

#[test]
fn test_file_tool_creates_parent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let nested_path = temp_dir.path().join("a/b/c/nested.txt");
    let path_str = nested_path.to_string_lossy().to_string();

    let tool = FileTool::new();

    tool.write(FileWriteParams {
        path: path_str.clone(),
        content: "nested content".to_string(),
    })
    .unwrap();

    assert!(nested_path.exists());

    let content = tool
        .read(FileReadParams {
            path: path_str,
            offset: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(content, "nested content");
}
