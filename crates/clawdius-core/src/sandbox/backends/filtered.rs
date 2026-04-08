//! Filter-based command blocking (WEAK — easily bypassed, use only as defense-in-depth).
//!
//! This backend checks commands against a hardcoded blocklist of dangerous
//! patterns. It is **not** a real sandbox — there are many trivial bypasses:
//!
//! - `rm -r -f /` bypasses the `rm -rf /` check (reordered flags).
//! - `sudo rm -rf /` is only caught if `sudo` appears before the blocked
//!   substring, but `sudo` itself is never blocked.
//! - `python3 -c "import os; os.system('rm -rf /')"` bypasses all filters
//!   because the dangerous payload is hidden inside a string argument.
//! - Any language runtime (perl, ruby, node, etc.) can execute arbitrary
//!   system calls through eval or equivalent mechanisms.
//!
//! This backend should only be used as a defense-in-depth measure alongside
//! a proper isolation mechanism (container, namespace, VM). Never rely on it
//! as the sole security boundary.

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
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        tracing::warn!(
            "Using filtered execution backend with no real isolation. \
             Command blocklist is trivially bypassable and should not be \
             relied upon as a security boundary."
        );
        Self { _config: config }
    }

    fn validate_command(&self, command: &str) -> Result<()> {
        // This check uses naive substring matching against a static blocklist.
        // It does NOT provide meaningful security because:
        //   - Flag reordering (rm -r -f vs rm -rf) evades the filter.
        //   - Any interpreter (python, perl, node, sh -c) can embed
        //     dangerous payloads as string arguments.
        //   - Environment variables, aliases, and shell features can
        //     alter command semantics after this check passes.
        // This is defense-in-depth at best.
        let cmd_lower = command.to_lowercase();
        for pattern in BLOCKED_PATTERNS {
            if cmd_lower.contains(&pattern.to_lowercase()) {
                return Err(Error::Sandbox(format!(
                    "Blocked command pattern detected: {pattern}"
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
