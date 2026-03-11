//! Integration tests for external editor support

use std::process::Command;
use std::env;

fn clawdius_binary() -> String {
    env::var("CLAWDIUS_BIN")
        .unwrap_or_else(|_| "./target/debug/clawdius".to_string())
}

#[test]
fn test_edit_command_help() {
    let output = Command::new(clawdius_binary())
        .args(["edit", "--help"])
        .output()
        .expect("Failed to execute clawdius");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Edit a long prompt"));
    assert!(stdout.contains("--initial"));
    assert!(stdout.contains("--editor"));
    assert!(stdout.contains("--extension"));
}

#[test]
fn test_edit_with_echo_editor() {
    let output = Command::new(clawdius_binary())
        .args(["edit", "--editor", "echo", "--initial", "Test content"])
        .output()
        .expect("Failed to execute clawdius");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Test content"));
}

#[test]
fn test_edit_with_extension() {
    let output = Command::new(clawdius_binary())
        .args(["edit", "--editor", "echo", "-x", "rs", "--initial", "fn main() {}"])
        .output()
        .expect("Failed to execute clawdius");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fn main()"));
}

#[test]
fn test_edit_json_output() {
    let output = Command::new(clawdius_binary())
        .args(["edit", "--editor", "echo", "--initial", "Test", "--output-format", "json"])
        .output()
        .expect("Failed to execute clawdius");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"content\""));
    assert!(stdout.contains("\"length\""));
}

#[test]
fn test_chat_editor_flag_help() {
    let output = Command::new(clawdius_binary())
        .args(["chat", "--help"])
        .output()
        .expect("Failed to execute clawdius");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--editor") || stdout.contains("-e"));
    assert!(stdout.contains("external editor"));
}

#[cfg(test)]
mod editor_config_tests {
    use clawdius_core::tools::editor::{EditorConfig, ExternalEditor};
    
    #[test]
    fn test_editor_config_default() {
        let config = EditorConfig::default();
        assert!(!config.command.is_empty());
        assert!(config.wait);
        assert!(config.args.is_empty());
    }
    
    #[test]
    fn test_editor_config_with_editor() {
        let config = EditorConfig::with_editor("nano");
        assert_eq!(config.command, "nano");
    }
    
    #[test]
    fn test_editor_config_with_args() {
        let config = EditorConfig::with_editor("code")
            .with_args(vec!["--wait".to_string(), "--new-window".to_string()]);
        assert_eq!(config.command, "code");
        assert_eq!(config.args.len(), 2);
    }
    
    #[test]
    fn test_external_editor_creation() {
        let config = EditorConfig::with_editor("vim");
        let editor = ExternalEditor::new(config);
        assert_eq!(editor.editor(), "vim");
    }
    
    #[test]
    fn test_external_editor_default() {
        let editor = ExternalEditor::default_editor();
        assert!(!editor.editor().is_empty());
    }
    
    #[test]
    fn test_external_editor_sync_edit() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let result = editor.open_and_edit("Hello, World!");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, World!");
    }
    
    #[tokio::test]
    async fn test_external_editor_async_edit() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let result = editor.edit("Async content").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Async content");
    }
    
    #[tokio::test]
    async fn test_external_editor_edit_with_extension() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let result = editor.edit_with_extension("fn test() {}", "rs").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "fn test() {}");
    }
    
    #[tokio::test]
    async fn test_external_editor_edit_prompt_strips_comments() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let input = "# Comment\nActual content\n# Another comment";
        let result = editor.edit_prompt(Some(input)).await;
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.contains("Comment"));
        assert!(output.contains("Actual content"));
    }
    
    #[tokio::test]
    async fn test_external_editor_edit_prompt_trims() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let input = "  \n  Content  \n  ";
        let result = editor.edit_prompt(Some(input)).await;
        
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output, "Content");
    }
    
    #[tokio::test]
    async fn test_external_editor_edit_template() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        
        let template = "Hello {{NAME}}!";
        let result = editor.edit_with_template(template, "{{NAME}}").await;
        
        assert!(result.is_ok());
        // Template wasn't modified (echo just returns the content)
        assert!(result.unwrap().contains("{{NAME}}"));
    }
}

#[cfg(test)]
mod editor_error_handling {
    use clawdius_core::tools::editor::{EditorConfig, ExternalEditor};
    
    #[test]
    fn test_nonexistent_editor() {
        let config = EditorConfig::with_editor("nonexistent_editor_12345");
        let editor = ExternalEditor::new(config);
        
        let result = editor.open_and_edit("test");
        assert!(result.is_err());
    }
}
