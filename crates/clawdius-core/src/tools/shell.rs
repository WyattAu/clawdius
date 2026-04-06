//! Shell command execution

use crate::config::ShellSandboxConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const BLOCKED_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "mkfs",
    "dd",
    "shred",
    "wipe",
    "chmod",
    "chown",
    "chgrp",
    "sudo",
    "su",
    "doas",
    "run0",
    "kill",
    "killall",
    "pkill",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "passwd",
    "useradd",
    "userdel",
    "usermod",
    "crontab",
    "at",
    "batch",
    "iptables",
    "nft",
    "ufw",
    "firewalld",
    "mount",
    "umount",
    "nc",
    "ncat",
    "socat",
];

fn is_command_blocked(command: &str) -> bool {
    let base = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    BLOCKED_COMMANDS.contains(&base)
}

static SHELL_CALL_COUNT: AtomicU64 = AtomicU64::new(0);
static SHELL_WINDOW_START: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

fn check_shell_rate_limit() -> crate::Result<()> {
    let now = Instant::now();
    let mut window_start = SHELL_WINDOW_START.lock().unwrap();

    match *window_start {
        Some(start) if now.duration_since(start) < Duration::from_secs(60) => {
            let count = SHELL_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            if count > 10 {
                return Err(crate::Error::Tool(
                    "Shell tool rate limit exceeded (max 10 calls per 60 seconds)".to_string(),
                ));
            }
        },
        _ => {
            SHELL_CALL_COUNT.store(1, Ordering::Relaxed);
            *window_start = Some(now);
        },
    }
    Ok(())
}

/// Shell command parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellParams {
    /// Command to execute
    pub command: String,
    /// Optional timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Working directory
    #[serde(default)]
    pub cwd: Option<String>,
}

fn default_timeout() -> u64 {
    120_000
}

/// Shell execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellResult {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether the command timed out
    pub timed_out: bool,
}

/// Shell tool implementation
pub struct ShellTool {
    config: ShellSandboxConfig,
    project_dir: std::path::PathBuf,
}

impl ShellTool {
    #[must_use]
    pub fn new(config: ShellSandboxConfig, project_dir: std::path::PathBuf) -> Self {
        ShellTool {
            config,
            project_dir,
        }
    }

    fn validate_command(&self, cmd: &str) -> crate::Result<()> {
        use crate::error::ErrorHelpers;

        let cmd_lower = cmd.to_lowercase();

        for blocked in &self.config.blocked_commands {
            if cmd_lower.starts_with(&blocked.to_lowercase()) {
                return Err(crate::Error::Sandbox(
                    ErrorHelpers::sandbox_violation(blocked).to_string(),
                ));
            }
        }

        Ok(())
    }

    fn validate_working_directory(&self, cwd: &Path) -> crate::Result<()> {
        use crate::error::ErrorHelpers;

        if !self.config.restrict_to_cwd {
            return Ok(());
        }

        let canonical_cwd = cwd.canonicalize().map_err(|e| {
            crate::Error::Sandbox(
                ErrorHelpers::invalid_config("working_directory", &e.to_string()).to_string(),
            )
        })?;

        let canonical_project = self.project_dir.canonicalize().map_err(|e| {
            crate::Error::Sandbox(
                ErrorHelpers::invalid_config("project_directory", &e.to_string()).to_string(),
            )
        })?;

        if !canonical_cwd.starts_with(&canonical_project) {
            return Err(crate::Error::Sandbox(
                ErrorHelpers::sandbox_violation("directory traversal").to_string(),
            ));
        }

        Ok(())
    }

    fn truncate_output(&self, output: &str) -> String {
        let bytes = output.as_bytes();
        if bytes.len() > self.config.max_output_bytes {
            let truncated_len = self.config.max_output_bytes;
            String::from_utf8_lossy(&bytes[..truncated_len]).to_string()
        } else {
            output.to_string()
        }
    }

    pub fn execute(&self, params: ShellParams) -> crate::Result<ShellResult> {
        check_shell_rate_limit()?;

        if is_command_blocked(&params.command) {
            return Err(crate::Error::Tool(format!(
                "Command '{}' is blocked for safety. Use a sandbox backend for restricted operations.",
                params.command
            )));
        }

        self.validate_command(&params.command)?;

        let shell = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        };
        let flag = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        let mut command = Command::new(shell);
        command.arg(flag).arg(&params.command);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let cwd = if let Some(cwd) = &params.cwd {
            let cwd_path = Path::new(cwd);
            self.validate_working_directory(cwd_path)?;
            cwd_path.to_path_buf()
        } else {
            self.project_dir.clone()
        };

        command.current_dir(&cwd);

        let start = Instant::now();
        let timeout = Duration::from_millis(params.timeout);

        let child = command.spawn()?;

        let result = child.wait_with_output()?;
        let elapsed = start.elapsed();

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let exit_code = result.status.code().unwrap_or(-1);
        let timed_out = elapsed > timeout;

        let stdout = self.truncate_output(&stdout);
        let stderr = self.truncate_output(&stderr);

        Ok(ShellResult {
            exit_code,
            stdout,
            stderr,
            timed_out,
        })
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new(
            ShellSandboxConfig::default(),
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        )
    }
}
