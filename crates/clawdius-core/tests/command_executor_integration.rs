//! Integration tests for Command Executor
//!
//! Tests variable substitution, file operations, shell execution,
//! git operations, and error handling.

use clawdius_core::commands::{
    CommandArgument, CommandExecutor, CommandResult, CommandTemplate, CustomCommand, TemplateStep,
};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn create_test_command() -> CustomCommand {
    CustomCommand {
        id: "test-id".to_string(),
        name: "test-command".to_string(),
        description: "Test command for integration tests".to_string(),
        arguments: vec![],
        template: CommandTemplate { steps: vec![] },
    }
}

fn create_test_command_with_args() -> CustomCommand {
    CustomCommand {
        id: "test-id".to_string(),
        name: "test-command".to_string(),
        description: "Test command for integration tests".to_string(),
        arguments: vec![
            CommandArgument {
                name: "path".to_string(),
                description: "File path".to_string(),
                required: true,
                default: None,
            },
            CommandArgument {
                name: "content".to_string(),
                description: "Content to write".to_string(),
                required: false,
                default: Some("default content".to_string()),
            },
        ],
        template: CommandTemplate { steps: vec![] },
    }
}

#[tokio::test]
async fn test_variable_substitution() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "shell".to_string(),
        template: "echo {{path}} {{content}}".to_string(),
        description: String::new(),
    }];

    let mut args = HashMap::new();
    args.insert("path".to_string(), "/tmp/test".to_string());
    args.insert("content".to_string(), "hello world".to_string());

    let results = executor.execute(&command, args).await.unwrap();

    assert!(!results.is_empty());
    assert!(results[0].success);
    assert!(results[0].output.contains("/tmp/test"));
    assert!(results[0].output.contains("hello world"));
}

#[tokio::test]
async fn test_missing_required_argument() {
    let executor = CommandExecutor;
    let command = create_test_command_with_args();

    let args = HashMap::new();

    let result = executor.execute(&command, args).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Missing required argument"));
}

#[tokio::test]
async fn test_file_read_operation() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_read.txt");
    fs::write(&test_file, "test content for reading").unwrap();

    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "file".to_string(),
        template: format!("read {}", test_file.display()),
        description: String::new(),
    }];

    let mut args = HashMap::new();
    args.insert("path".to_string(), test_file.display().to_string());

    let results = executor.execute(&command, args).await.unwrap();

    assert!(!results.is_empty());
    assert!(results[0].success);
    assert!(results[0].output.contains("test content for reading"));
}

#[tokio::test]
async fn test_file_write_operation() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_write.txt");

    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "file".to_string(),
        template: format!("write {} new content here", test_file.display()),
        description: String::new(),
    }];

    let mut args = HashMap::new();
    args.insert("path".to_string(), test_file.display().to_string());

    let results = executor.execute(&command, args).await.unwrap();

    assert!(!results.is_empty());
    assert!(results[0].success);
    assert!(results[0].output.contains("Wrote"));

    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "new content here");
}

#[tokio::test]
async fn test_file_read_nonexistent() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "file".to_string(),
        template: "read /nonexistent/path/file.txt".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_shell_execution() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "shell".to_string(),
        template: "echo test_output_12345".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_shell_execution_failure() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "shell".to_string(),
        template: "ls /nonexistent_directory_12345".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_git_status() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "git".to_string(),
        template: "status".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
}

#[tokio::test]
async fn test_step_execution_order() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![
        TemplateStep {
            tool: "shell".to_string(),
            template: "echo step1".to_string(),
            description: String::new(),
        },
        TemplateStep {
            tool: "shell".to_string(),
            template: "echo step2".to_string(),
            description: String::new(),
        },
        TemplateStep {
            tool: "shell".to_string(),
            template: "echo step3".to_string(),
            description: String::new(),
        },
    ];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_stop_on_failure() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![
        TemplateStep {
            tool: "shell".to_string(),
            template: "echo success".to_string(),
            description: String::new(),
        },
        TemplateStep {
            tool: "shell".to_string(),
            template: "ls /nonexistent_12345_xyz".to_string(),
            description: String::new(),
        },
        TemplateStep {
            tool: "shell".to_string(),
            template: "echo should_not_run".to_string(),
            description: String::new(),
        },
    ];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(results.len() >= 1);
    assert!(results.len() <= 2);
}

#[tokio::test]
async fn test_unresolved_variables() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "shell".to_string(),
        template: "echo {{unresolved_var}}".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let result = executor.execute(&command, args).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unresolved")
            || err_msg.contains("Unresolved")
            || err_msg.contains("variable")
            || err_msg.contains("Variable")
    );
}

#[tokio::test]
async fn test_empty_file_command() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "file".to_string(),
        template: "".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_unknown_file_action() {
    let executor = CommandExecutor;
    let mut command = create_test_command();

    command.template.steps = vec![TemplateStep {
        tool: "file".to_string(),
        template: "invalid_action /path".to_string(),
        description: String::new(),
    }];

    let args = HashMap::new();

    let results = executor.execute(&command, args).await;

    assert!(results.is_ok());
    let results = results.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_command_result_success() {
    let result = CommandResult::success("test_step", "output data".to_string());

    assert_eq!(result.step, "test_step");
    assert_eq!(result.output, "output data");
    assert!(result.success);
}

#[tokio::test]
async fn test_command_result_error() {
    let result = CommandResult::error("test_step", "error message".to_string());

    assert_eq!(result.step, "test_step");
    assert_eq!(result.output, "error message");
    assert!(!result.success);
}
