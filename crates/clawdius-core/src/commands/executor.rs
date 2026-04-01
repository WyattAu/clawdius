//! Command executor

use super::super::commands::{CustomCommand, TemplateStep};
use crate::error::{Error, Result};
use crate::tools::file::{FileReadParams, FileTool, FileWriteParams};
use crate::tools::git::{GitDiffParams, GitLogParams, GitTool};
use crate::tools::shell::{ShellParams, ShellTool};
use std::collections::HashMap;

/// Command execution result
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Step name
    pub step: String,
    /// Output from step
    pub output: String,
    /// Whether step succeeded
    pub success: bool,
}

impl CommandResult {
    pub fn success(step: impl Into<String>, output: String) -> Self {
        Self {
            step: step.into(),
            output,
            success: true,
        }
    }
    pub fn error(step: impl Into<String>, output: String) -> Self {
        Self {
            step: step.into(),
            output,
            success: false,
        }
    }
}

/// Execute custom commands
pub struct CommandExecutor;

impl CommandExecutor {
    /// Execute a custom command
    pub async fn execute(
        &self,
        command: &CustomCommand,
        args: HashMap<String, String>,
    ) -> Result<Vec<CommandResult>> {
        let mut results = Vec::new();

        // Validate required arguments
        for arg in &command.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(Error::InvalidInput(format!(
                    "Missing required argument: {}",
                    arg.name
                )));
            }
        }

        // Execute each step in the template
        for (step_index, step) in command.template.steps.iter().enumerate() {
            let result = self.execute_step(step, step_index, &args).await?;

            // Stop on first failure
            if !result.success {
                results.push(result);
                break;
            }

            results.push(result);
        }

        Ok(results)
    }

    async fn execute_step(
        &self,
        step: &TemplateStep,
        step_index: usize,
        args: &HashMap<String, String>,
    ) -> Result<CommandResult> {
        let step_name = format!("step_{}", step_index + 1);

        // Substitute variables in the step template
        let rendered = self.substitute_variables(&step.template, args)?;

        match step.tool.as_str() {
            "file" => self.execute_file_step(&step_name, &rendered).await,
            "shell" => self.execute_shell_step(&step_name, &rendered).await,
            "git" => self.execute_git_step(&step_name, &rendered).await,
            _ => Ok(CommandResult::success(&step_name, rendered)),
        }
    }

    fn substitute_variables(
        &self,
        template: &str,
        args: &HashMap<String, String>,
    ) -> Result<String> {
        let mut result = template.to_string();

        // Replace {{variable}} with values from args
        for (key, value) in args {
            let pattern = format!("{{{{{key}}}}}");
            result = result.replace(&pattern, value);
        }

        // Check for unresolved variables
        if result.contains("{{") && result.contains("}}") {
            return Err(Error::InvalidInput(
                "Template contains unresolved variables".to_string(),
            ));
        }

        Ok(result)
    }

    async fn execute_file_step(&self, step_name: &str, command: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(CommandResult::error(
                step_name.to_string(),
                "Empty file command".to_string(),
            ));
        }

        let action = parts[0];
        let file_tool = FileTool::new();

        match action {
            "read" => {
                if parts.len() < 2 {
                    return Ok(CommandResult::error(
                        step_name.to_string(),
                        "read requires file path".to_string(),
                    ));
                }
                let params = FileReadParams {
                    path: parts[1].to_string(),
                    offset: None,
                    limit: None,
                };
                match file_tool.read(params) {
                    Ok(content) => Ok(CommandResult::success(step_name.to_string(), content)),
                    Err(e) => Ok(CommandResult::error(step_name.to_string(), e.to_string())),
                }
            },
            "write" => {
                if parts.len() < 3 {
                    return Ok(CommandResult::error(
                        step_name.to_string(),
                        "write requires path and content".to_string(),
                    ));
                }
                let content = parts[2..].join(" ");
                let content_len = content.len();
                let params = FileWriteParams {
                    path: parts[1].to_string(),
                    content,
                };
                match file_tool.write(params) {
                    Ok(()) => Ok(CommandResult::success(
                        step_name.to_string(),
                        format!("Wrote {content_len} bytes"),
                    )),
                    Err(e) => Ok(CommandResult::error(step_name.to_string(), e.to_string())),
                }
            },
            _ => Ok(CommandResult::error(
                step_name.to_string(),
                format!("Unknown file action: {action}"),
            )),
        }
    }

    async fn execute_shell_step(&self, step_name: &str, command: &str) -> Result<CommandResult> {
        let shell_tool = ShellTool::default();
        let params = ShellParams {
            command: command.to_string(),
            timeout: 120_000,
            cwd: None,
        };

        match shell_tool.execute(params) {
            Ok(result) if result.exit_code == 0 => {
                Ok(CommandResult::success(step_name.to_string(), result.stdout))
            },
            Ok(result) => Ok(CommandResult::error(step_name.to_string(), result.stderr)),
            Err(e) => Ok(CommandResult::error(step_name.to_string(), e.to_string())),
        }
    }

    async fn execute_git_step(&self, step_name: &str, command: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(CommandResult::error(
                step_name.to_string(),
                "Empty git command".to_string(),
            ));
        }

        let action = parts[0];
        let git_tool = GitTool::default();

        let result = match action {
            "status" => {
                let cwd = parts.get(1).map(std::string::ToString::to_string);
                git_tool.status(cwd.as_deref())
            },
            "diff" => {
                let params = GitDiffParams {
                    staged: false,
                    path: parts.get(1).map(std::string::ToString::to_string),
                };
                git_tool.diff(params, None)
            },
            "log" => {
                let count = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                let params = GitLogParams {
                    count,
                    path: parts.get(2).map(std::string::ToString::to_string),
                };
                git_tool.log(params, None)
            },
            _ => {
                return Ok(CommandResult::error(
                    step_name.to_string(),
                    format!("Unknown git action: {action}"),
                ));
            },
        };

        match result {
            Ok(output) => Ok(CommandResult::success(step_name.to_string(), output)),
            Err(e) => Ok(CommandResult::error(step_name.to_string(), e.to_string())),
        }
    }
}
