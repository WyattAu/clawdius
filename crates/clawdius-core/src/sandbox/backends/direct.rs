//! Direct execution backend (no sandboxing)

use super::SandboxBackend;
use crate::error::Result;
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};

pub struct DirectBackend {
    _config: SandboxConfig,
}

impl DirectBackend {
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        Self { _config: config }
    }
}

impl SandboxBackend for DirectBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let output = Command::new(command).args(args).current_dir(cwd).output()?;
        Ok(output)
    }

    fn name(&self) -> &'static str {
        "direct"
    }
}
