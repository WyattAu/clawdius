//! Filtered execution backend with basic command filtering

use super::SandboxBackend;
use crate::error::{Error, Result};
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};

const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf /",
    "mkfs",
    "dd if=/dev/zero",
    "dd if=/dev/urandom",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "chown -R",
    "> /dev/sda",
    "mv /* /dev/null",
];

pub struct FilteredBackend {
    _config: SandboxConfig,
}

impl FilteredBackend {
    pub fn new(config: SandboxConfig) -> Self {
        Self { _config: config }
    }

    fn validate_command(&self, command: &str) -> Result<()> {
        let cmd_lower = command.to_lowercase();
        for pattern in BLOCKED_PATTERNS {
            if cmd_lower.contains(&pattern.to_lowercase()) {
                return Err(Error::Sandbox(format!(
                    "Blocked command pattern detected: {}",
                    pattern
                )));
            }
        }
        Ok(())
    }
}

impl SandboxBackend for FilteredBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let full_cmd = format!("{} {}", command, args.join(" "));
        self.validate_command(&full_cmd)?;

        let output = Command::new(command).args(args).current_dir(cwd).output()?;
        Ok(output)
    }

    fn name(&self) -> &'static str {
        "filtered"
    }
}
