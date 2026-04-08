//! Direct execution backend (NO ISOLATION — for trusted code only).
//!
//! This backend runs commands on the host with zero sandboxing. It provides
//! no filesystem isolation, no network restrictions, no resource limits, and
//! no command filtering. It should **only** be used for code that is fully
//! trusted and audited.

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
        tracing::warn!(
            "Using direct execution backend with no sandboxing. \
             Ensure the caller is fully trusted."
        );
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
