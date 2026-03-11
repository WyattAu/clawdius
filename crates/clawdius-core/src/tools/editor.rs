//! External editor support for editing prompts
//!
//! Provides functionality to open user's preferred $EDITOR for composing
//! long or complex prompts.
//!
//! # Features
//!
//! - Automatic editor detection from $EDITOR/$VISUAL environment variables
//! - Platform-specific defaults (vim on Unix, notepad on Windows)
//! - Custom file extensions for syntax highlighting
//! - Template-based editing with placeholder support
//! - Comment stripping for cleaner prompts
//!
//! # Example
//!
//! ```rust,no_run
//! use clawdius_core::tools::editor::{ExternalEditor, EditorConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = EditorConfig::default();
//!     let editor = ExternalEditor::new(config);
//!     
//!     let content = editor.edit("Initial draft content").await?;
//!     println!("Edited content: {}", content);
//!     Ok(())
//! }
//! ```

use std::env;
use std::process::Command;

use tempfile::NamedTempFile;
use tokio::fs;

use crate::{Error, Result};

/// External editor configuration
#[derive(Debug, Clone)]
pub struct EditorConfig {
    /// Editor command (e.g., "vim", "emacs", "code")
    pub command: String,

    /// Arguments to pass to editor
    pub args: Vec<String>,

    /// Whether to wait for editor to close
    pub wait: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: Self::detect_editor(),
            args: vec![],
            wait: true,
        }
    }
}

impl EditorConfig {
    /// Detect editor from environment
    ///
    /// Checks $EDITOR first, then $VISUAL, then attempts to find common editors
    /// in PATH, finally falling back to platform defaults.
    pub fn detect_editor() -> String {
        env::var("EDITOR")
            .or_else(|_| env::var("VISUAL"))
            .unwrap_or_else(|_| Self::find_common_editor())
    }

    /// Find a common editor in PATH
    ///
    /// Searches for common editors in order of preference.
    fn find_common_editor() -> String {
        if cfg!(windows) {
            // On Windows, try common editors in order
            let editors = ["code", "notepad++", "notepad"];
            for editor in editors {
                if Self::command_exists(editor) {
                    return editor.to_string();
                }
            }
            "notepad".to_string()
        } else {
            // On Unix-like systems, try common editors in order
            let editors = ["vim", "nano", "code", "emacs", "vi"];
            for editor in editors {
                if Self::command_exists(editor) {
                    return editor.to_string();
                }
            }
            "vim".to_string()
        }
    }

    /// Check if a command exists in PATH
    fn command_exists(cmd: &str) -> bool {
        which::which(cmd).is_ok()
    }

    /// Validate that the configured editor exists in PATH
    pub fn validate_editor(&self) -> Result<()> {
        if !Self::command_exists(&self.command) {
            return Err(Error::ToolExecution {
                tool: "editor".to_string(),
                reason: format!(
                    "Editor '{}' not found in PATH. Please install it or set $EDITOR to a valid editor.",
                    self.command
                ),
            });
        }
        Ok(())
    }

    /// Create config with specific editor
    pub fn with_editor(editor: impl Into<String>) -> Self {
        Self {
            command: editor.into(),
            args: vec![],
            wait: true,
        }
    }

    /// Add arguments to the editor command
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// External editor tool
#[derive(Debug)]
pub struct ExternalEditor {
    config: EditorConfig,
}

impl Default for ExternalEditor {
    fn default() -> Self {
        Self::new(EditorConfig::default())
    }
}

impl ExternalEditor {
    /// Create new external editor with configuration
    pub fn new(config: EditorConfig) -> Self {
        Self { config }
    }

    /// Create with default config
    pub fn default_editor() -> Self {
        Self::default()
    }

    /// Get the configured editor command
    pub fn editor(&self) -> &str {
        &self.config.command
    }

    /// Check if the configured editor exists in PATH
    pub fn editor_exists(&self) -> bool {
        EditorConfig::command_exists(&self.config.command)
    }

    /// Validate editor and return error if not found
    pub fn validate(&self) -> Result<()> {
        self.config.validate_editor()
    }

    /// Open editor with initial content and return edited content
    pub async fn edit(&self, initial_content: &str) -> Result<String> {
        self.edit_with_extension(initial_content, "md").await
    }

    /// Open editor with specific file extension
    ///
    /// The extension helps editors provide appropriate syntax highlighting.
    pub async fn edit_with_extension(
        &self,
        initial_content: &str,
        extension: &str,
    ) -> Result<String> {
        let temp_file = NamedTempFile::with_suffix(format!(".{}", extension)).map_err(Error::Io)?;

        let temp_path = temp_file.path();

        fs::write(temp_path, initial_content)
            .await
            .map_err(Error::Io)?;

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);
        cmd.arg(temp_path);

        let status = cmd.status().map_err(|e| Error::ToolExecution {
            tool: "editor".to_string(),
            reason: format!("Failed to launch editor '{}': {}", self.config.command, e),
        })?;

        if !status.success() {
            return Err(Error::ToolExecution {
                tool: "editor".to_string(),
                reason: format!("Editor exited with status: {}", status),
            });
        }

        let content = fs::read_to_string(temp_path).await.map_err(Error::Io)?;

        Ok(content)
    }

    /// Edit with template and placeholder detection
    ///
    /// Warns if the placeholder wasn't modified by the user.
    pub async fn edit_with_template(&self, template: &str, placeholder: &str) -> Result<String> {
        let content = self.edit(template).await?;

        if content.contains(placeholder) {
            tracing::warn!("Placeholder '{}' was not modified", placeholder);
        }

        Ok(content)
    }

    /// Edit long prompt with convenience features
    ///
    /// - Provides a helpful template for new prompts
    /// - Strips comment lines (starting with #)
    /// - Trims whitespace
    pub async fn edit_prompt(&self, initial: Option<&str>) -> Result<String> {
        let template = initial.unwrap_or(
            "# Enter your prompt below\n\
             # Lines starting with # are comments and will be removed\n\
             # Save and quit to submit\n\n",
        );

        let content = self.edit_with_extension(template, "md").await?;

        let cleaned: String = content
            .lines()
            .filter(|line| !line.trim().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(cleaned.trim().to_string())
    }

    /// Synchronous version for non-async contexts
    ///
    /// Opens the editor with initial content and returns the edited content.
    /// This is a blocking operation.
    pub fn open_and_edit(&self, initial_content: &str) -> Result<String> {
        let temp_file = NamedTempFile::with_suffix(".md").map_err(Error::Io)?;

        let temp_path = temp_file.path();

        std::fs::write(temp_path, initial_content).map_err(Error::Io)?;

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);
        cmd.arg(temp_path);

        let status = cmd.status().map_err(|e| Error::ToolExecution {
            tool: "editor".to_string(),
            reason: format!("Failed to launch editor '{}': {}", self.config.command, e),
        })?;

        if !status.success() {
            return Err(Error::ToolExecution {
                tool: "editor".to_string(),
                reason: format!("Editor exited with status: {}", status),
            });
        }

        let content = std::fs::read_to_string(temp_path).map_err(Error::Io)?;

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_detection() {
        let editor = EditorConfig::detect_editor();
        assert!(!editor.is_empty());
    }

    #[test]
    fn test_default_config() {
        let config = EditorConfig::default();
        assert!(!config.command.is_empty());
        assert!(config.wait);
    }

    #[test]
    fn test_custom_editor() {
        let config = EditorConfig::with_editor("nano");
        assert_eq!(config.command, "nano");
    }

    #[test]
    fn test_config_with_args() {
        let config = EditorConfig::with_editor("code").with_args(vec!["--wait".to_string()]);
        assert_eq!(config.command, "code");
        assert_eq!(config.args, vec!["--wait"]);
    }

    #[test]
    fn test_editor_creation() {
        let config = EditorConfig::with_editor("vim");
        let editor = ExternalEditor::new(config);
        assert_eq!(editor.editor(), "vim");
    }

    #[test]
    fn test_default_editor() {
        let editor = ExternalEditor::default_editor();
        assert!(!editor.editor().is_empty());
    }

    #[test]
    fn test_sync_edit() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);

        let content = "Test content";
        let result = editor.open_and_edit(content);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[tokio::test]
    async fn test_async_edit() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);

        let content = "Test async content";
        let result = editor.edit(content).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[tokio::test]
    async fn test_edit_with_extension() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);

        let content = "fn main() {}";
        let result = editor.edit_with_extension(content, "rs").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[tokio::test]
    async fn test_edit_prompt_strips_comments() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);

        let input = "# This is a comment\nActual content\n# Another comment";
        let result = editor.edit_prompt(Some(input)).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.contains("comment"));
        assert!(output.contains("Actual content"));
    }

    #[tokio::test]
    async fn test_edit_prompt_trims_whitespace() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);

        let input = "  \n  Content here  \n  ";
        let result = editor.edit_prompt(Some(input)).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.starts_with('\n'));
        assert!(!output.ends_with('\n'));
    }

    #[test]
    fn test_find_common_editor() {
        let editor = EditorConfig::find_common_editor();
        assert!(!editor.is_empty());

        // Should find at least vim or nano on Unix systems
        #[cfg(unix)]
        assert!(
            editor == "vim"
                || editor == "nano"
                || editor == "code"
                || editor == "emacs"
                || editor == "vi"
        );

        // Should find at least notepad on Windows
        #[cfg(windows)]
        assert!(editor == "code" || editor == "notepad++" || editor == "notepad");
    }

    #[test]
    fn test_command_exists() {
        // Test with a command that should exist on most systems
        #[cfg(unix)]
        {
            // ls should exist on Unix systems
            assert!(EditorConfig::command_exists("ls"));
            // This command shouldn't exist
            assert!(!EditorConfig::command_exists("nonexistent_command_12345"));
        }

        #[cfg(windows)]
        {
            // cmd should exist on Windows
            assert!(EditorConfig::command_exists("cmd"));
            // This command shouldn't exist
            assert!(!EditorConfig::command_exists("nonexistent_command_12345"));
        }
    }

    #[test]
    fn test_editor_exists() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        assert!(editor.editor_exists());

        let config = EditorConfig::with_editor("nonexistent_editor_12345");
        let editor = ExternalEditor::new(config);
        assert!(!editor.editor_exists());
    }

    #[test]
    fn test_validate_editor_success() {
        let config = EditorConfig::with_editor("echo");
        assert!(config.validate_editor().is_ok());
    }

    #[test]
    fn test_validate_editor_failure() {
        let config = EditorConfig::with_editor("nonexistent_editor_12345");
        let result = config.validate_editor();
        assert!(result.is_err());

        if let Err(Error::ToolExecution { tool, reason }) = result {
            assert_eq!(tool, "editor");
            assert!(reason.contains("nonexistent_editor_12345"));
            assert!(reason.contains("not found in PATH"));
        } else {
            panic!("Expected ToolExecution error");
        }
    }

    #[test]
    fn test_editor_validate_method() {
        let config = EditorConfig::with_editor("echo");
        let editor = ExternalEditor::new(config);
        assert!(editor.validate().is_ok());

        let config = EditorConfig::with_editor("nonexistent_editor_12345");
        let editor = ExternalEditor::new(config);
        assert!(editor.validate().is_err());
    }
}
