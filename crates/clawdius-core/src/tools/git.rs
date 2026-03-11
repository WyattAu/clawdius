//! Git operations tool

use super::shell::{ShellParams, ShellResult, ShellTool};
use crate::config::ShellSandboxConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Git diff parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffParams {
    /// Staged or unstaged
    #[serde(default)]
    pub staged: bool,
    /// Optional file path
    #[serde(default)]
    pub path: Option<String>,
}

/// Git log parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogParams {
    /// Number of commits
    #[serde(default = "default_count")]
    pub count: usize,
    /// Optional file path
    #[serde(default)]
    pub path: Option<String>,
}

fn default_count() -> usize {
    10
}

/// Git tool implementation
pub struct GitTool {
    shell: ShellTool,
}

impl GitTool {
    #[must_use]
    pub fn new(config: ShellSandboxConfig, project_dir: PathBuf) -> Self {
        GitTool {
            shell: ShellTool::new(config, project_dir),
        }
    }

    pub fn status(&self, cwd: Option<&str>) -> crate::Result<String> {
        let result = self.run_git("status", &[], cwd)?;
        if result.exit_code != 0 {
            return Err(crate::Error::Tool(format!(
                "Git status failed: {}",
                result.stderr
            )));
        }
        Ok(result.stdout)
    }

    pub fn diff(&self, params: GitDiffParams, cwd: Option<&str>) -> crate::Result<String> {
        let mut args = vec!["diff"];

        if params.staged {
            args.push("--staged");
        }

        if let Some(path) = &params.path {
            args.push("--");
            args.push(path);
        }

        let result = self.run_git("diff", &args[1..], cwd)?;
        if result.exit_code != 0 {
            return Err(crate::Error::Tool(format!(
                "Git diff failed: {}",
                result.stderr
            )));
        }
        Ok(result.stdout)
    }

    pub fn log(&self, params: GitLogParams, cwd: Option<&str>) -> crate::Result<String> {
        let count_str = params.count.to_string();
        let mut args = vec!["log", "-n", &count_str, "--oneline"];

        if let Some(path) = &params.path {
            args.push("--");
            args.push(path);
        }

        let result = self.run_git("log", &args[1..], cwd)?;
        if result.exit_code != 0 {
            return Err(crate::Error::Tool(format!(
                "Git log failed: {}",
                result.stderr
            )));
        }
        Ok(result.stdout)
    }

    fn run_git(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&str>,
    ) -> crate::Result<ShellResult> {
        let full_command = if args.is_empty() {
            format!("git {command}")
        } else {
            format!("git {} {}", command, args.join(" "))
        };

        let shell_params = ShellParams {
            command: full_command,
            timeout: 30_000,
            cwd: cwd.map(std::string::ToString::to_string),
        };

        self.shell.execute(shell_params)
    }
}

impl Default for GitTool {
    fn default() -> Self {
        Self::new(
            ShellSandboxConfig::default(),
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        )
    }
}
