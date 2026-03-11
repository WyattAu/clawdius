//! Bubblewrap sandbox backend for Linux

use super::SandboxBackend;
use crate::error::{Error, Result};
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};

pub struct BubblewrapBackend {
    config: SandboxConfig,
}

impl BubblewrapBackend {
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn is_available() -> bool {
        Command::new("bwrap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl SandboxBackend for BubblewrapBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let cwd_str = cwd.to_string_lossy();

        let mut cmd = Command::new("bwrap");

        cmd.arg("--ro-bind").arg("/usr").arg("/usr");
        cmd.arg("--ro-bind").arg("/lib").arg("/lib");
        cmd.arg("--ro-bind").arg("/lib64").arg("/lib64");
        cmd.arg("--ro-bind").arg("/bin").arg("/bin");
        cmd.arg("--ro-bind").arg("/sbin").arg("/sbin");

        cmd.arg("--bind")
            .arg(cwd_str.as_ref())
            .arg(cwd_str.as_ref());

        cmd.arg("--dev").arg("/dev");
        cmd.arg("--proc").arg("/proc");

        cmd.arg("--unshare-all");
        cmd.arg("--die-with-parent");

        if !self.config.network {
            cmd.arg("--unshare-net");
        }

        for mount in &self.config.mounts {
            if mount.read_only {
                cmd.arg("--ro-bind")
                    .arg(&mount.source)
                    .arg(&mount.destination);
            } else {
                cmd.arg("--bind").arg(&mount.source).arg(&mount.destination);
            }
        }

        cmd.arg("--");
        cmd.arg(command);
        cmd.args(args);

        cmd.current_dir(cwd);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Sandbox(
                    "bubblewrap (bwrap) not found. Please install bubblewrap.".to_string(),
                )
            } else {
                Error::Io(e)
            }
        })?;

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "bubblewrap"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxTier;

    #[test]
    fn test_bubblewrap_available() {
        let available = BubblewrapBackend::is_available();
        println!("Bubblewrap available: {}", available);
    }

    #[test]
    fn test_bubblewrap_execute() {
        if !BubblewrapBackend::is_available() {
            println!("Skipping test: bubblewrap not available");
            return;
        }

        let config = SandboxConfig {
            tier: SandboxTier::Untrusted,
            network: false,
            mounts: vec![],
        };

        let backend = BubblewrapBackend::new(config);
        let cwd = std::env::current_dir().unwrap();

        let result = backend.execute("echo", &["hello"], &cwd);
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(stdout.contains("hello"));
            }
            Err(e) => {
                println!("Error (expected if no bwrap): {}", e);
            }
        }
    }
}
