//! sandbox-exec backend for macOS

use super::SandboxBackend;
use crate::error::{Error, Result};
use crate::sandbox::tiers::SandboxConfig;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

pub struct SandboxExecBackend {
    config: SandboxConfig,
}

impl SandboxExecBackend {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    pub fn is_available() -> bool {
        Command::new("sandbox-exec")
            .arg("-h")
            .output()
            .map(|_| true)
            .unwrap_or(false)
    }

    fn generate_profile(&self, cwd: &Path) -> String {
        let cwd_str = cwd.to_string_lossy();

        let network_rule = if self.config.network {
            "(allow network*)"
        } else {
            "(deny network*)"
        };

        let mut mounts_rules = String::new();
        for mount in &self.config.mounts {
            if mount.read_only {
                mounts_rules.push_str(&format!(
                    "(allow file-read* (subpath \"{}\"))\n",
                    mount.destination
                ));
            } else {
                mounts_rules.push_str(&format!(
                    "(allow file* (subpath \"{}\"))\n",
                    mount.destination
                ));
            }
        }

        format!(
            r#"(version 1)
(deny default)
(allow process*)
(allow file* (subpath "{}"))
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/System"))
(allow file-read* (subpath "/Library"))
(allow file-read* (subpath "/bin"))
(allow file-read* (subpath "/sbin"))
{}
{}
"#,
            cwd_str, network_rule, mounts_rules
        )
    }
}

impl SandboxBackend for SandboxExecBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let profile = self.generate_profile(cwd);

        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f")
            .arg("/dev/stdin")
            .arg(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Sandbox("sandbox-exec not found".to_string())
            } else {
                Error::Io(e)
            }
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(profile.as_bytes()).map_err(Error::Io)?;
        }

        let output = child.wait_with_output().map_err(Error::Io)?;
        Ok(output)
    }

    fn name(&self) -> &'static str {
        "sandbox-exec"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxTier;

    #[test]
    fn test_sandbox_exec_available() {
        let available = SandboxExecBackend::is_available();
        println!("sandbox-exec available: {}", available);
    }

    #[test]
    fn test_generate_profile() {
        let config = SandboxConfig {
            tier: SandboxTier::Untrusted,
            network: false,
            mounts: vec![],
        };

        let backend = SandboxExecBackend::new(config);
        let cwd = Path::new("/tmp/test");

        let profile = backend.generate_profile(cwd);
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("/tmp/test"));
        assert!(profile.contains("(deny network*)"));
    }

    #[test]
    fn test_generate_profile_with_network() {
        let config = SandboxConfig {
            tier: SandboxTier::Untrusted,
            network: true,
            mounts: vec![],
        };

        let backend = SandboxExecBackend::new(config);
        let cwd = Path::new("/tmp/test");

        let profile = backend.generate_profile(cwd);
        assert!(profile.contains("(allow network*)"));
    }
}
