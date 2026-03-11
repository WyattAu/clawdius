use clawdius_core::config::ShellSandboxConfig;
use clawdius_core::tools::shell::{ShellParams, ShellTool};
use std::path::PathBuf;

#[test]
fn test_blocked_command() {
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, PathBuf::from("."));

    let params = ShellParams {
        command: "rm -rf /".to_string(),
        timeout: 5000,
        cwd: None,
    };

    let result = tool.execute(params);
    assert!(result.is_err());

    if let Err(clawdius_core::Error::Sandbox(msg)) = result {
        assert!(msg.contains("Blocked command"));
    } else {
        panic!("Expected Sandbox error");
    }
}

#[test]
fn test_safe_command() {
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, PathBuf::from("."));

    let params = ShellParams {
        command: "echo 'hello'".to_string(),
        timeout: 5000,
        cwd: None,
    };

    let result = tool.execute(params);
    assert!(result.is_ok());

    let shell_result = result.unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("hello"));
}

#[test]
fn test_output_truncation() {
    let mut config = ShellSandboxConfig::default();
    config.max_output_bytes = 10;
    let tool = ShellTool::new(config, PathBuf::from("."));

    let params = ShellParams {
        command: "echo 'this is a very long output string'".to_string(),
        timeout: 5000,
        cwd: None,
    };

    let result = tool.execute(params);
    assert!(result.is_ok());

    let shell_result = result.unwrap();
    assert!(shell_result.stdout.len() <= 10);
}

#[test]
fn test_working_directory_restriction() {
    let config = ShellSandboxConfig::default();
    let tool = ShellTool::new(config, PathBuf::from("."));

    let params = ShellParams {
        command: "echo 'test'".to_string(),
        timeout: 5000,
        cwd: Some("/tmp".to_string()),
    };

    let result = tool.execute(params);
    assert!(result.is_err());

    if let Err(clawdius_core::Error::Sandbox(msg)) = result {
        assert!(msg.contains("project directory"));
    } else {
        panic!("Expected Sandbox error");
    }
}

#[test]
fn test_working_directory_allowed() {
    let config = ShellSandboxConfig::default();
    let current_dir = std::env::current_dir().unwrap();
    let tool = ShellTool::new(config, current_dir.clone());

    let params = ShellParams {
        command: "echo 'test'".to_string(),
        timeout: 5000,
        cwd: Some(current_dir.to_string_lossy().to_string()),
    };

    let result = tool.execute(params);
    assert!(result.is_ok());
}
