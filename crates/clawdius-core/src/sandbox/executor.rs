//! Sandbox executor

use crate::error::{Error, Result};
use crate::sandbox::backends::{DirectBackend, FilteredBackend, SandboxBackend};
use crate::sandbox::tiers::SandboxConfig;
use crate::sandbox::SandboxTier;
use std::path::Path;
use std::process::Output;

#[cfg(target_os = "linux")]
use crate::sandbox::backends::BubblewrapBackend;

#[cfg(target_os = "macos")]
use crate::sandbox::backends::SandboxExecBackend;

pub struct SandboxExecutor {
    backend: Box<dyn SandboxBackend>,
    #[allow(dead_code)]
    tier: SandboxTier,
}

impl std::fmt::Debug for SandboxExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxExecutor")
            .field("backend", &self.backend.name())
            .field("tier", &format!("{:?}", self.tier))
            .finish()
    }
}

impl SandboxExecutor {
    pub fn new(tier: SandboxTier, config: SandboxConfig) -> Result<Self> {
        let backend: Box<dyn SandboxBackend> = match tier {
            SandboxTier::TrustedAudited => Box::new(DirectBackend::new(config)),
            SandboxTier::Trusted => Box::new(FilteredBackend::new(config)),
            SandboxTier::Untrusted | SandboxTier::Hardened => Self::platform_sandbox(config)?,
        };
        Ok(Self { backend, tier })
    }

    #[cfg(target_os = "linux")]
    fn platform_sandbox(config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        if BubblewrapBackend::is_available() {
            Ok(Box::new(BubblewrapBackend::new(config)))
        } else {
            Err(Error::Sandbox(
                "bubblewrap (bwrap) is required for Untrusted/Hardened sandbox tiers on Linux. \
                 Please install bubblewrap: apt install bubblewrap or dnf install bubblewrap"
                    .to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    fn platform_sandbox(config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        if SandboxExecBackend::is_available() {
            Ok(Box::new(SandboxExecBackend::new(config)))
        } else {
            Err(Error::Sandbox(
                "sandbox-exec is required for Untrusted/Hardened sandbox tiers on macOS"
                    .to_string(),
            ))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn platform_sandbox(_config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        Err(Error::Sandbox(
            "Platform sandboxing is only supported on Linux and macOS".to_string(),
        ))
    }

    pub fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        self.backend.execute(command, args, cwd)
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_trusted_audited() {
        let config = SandboxConfig {
            tier: SandboxTier::TrustedAudited,
            network: true,
            mounts: vec![],
        };

        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_executor_trusted() {
        let config = SandboxConfig {
            tier: SandboxTier::Trusted,
            network: true,
            mounts: vec![],
        };

        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        assert_eq!(executor.backend_name(), "filtered");
    }

    #[test]
    fn test_executor_untrusted_fallback() {
        let config = SandboxConfig {
            tier: SandboxTier::Untrusted,
            network: false,
            mounts: vec![],
        };

        let result = SandboxExecutor::new(SandboxTier::Untrusted, config);

        #[cfg(target_os = "linux")]
        {
            if BubblewrapBackend::is_available() {
                let executor = result.expect("Should create executor when bwrap is available");
                assert_eq!(executor.backend_name(), "bubblewrap");
            } else {
                result.expect_err("Should fail when bwrap is not available");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if SandboxExecBackend::is_available() {
                let executor =
                    result.expect("Should create executor when sandbox-exec is available");
                assert_eq!(executor.backend_name(), "sandbox-exec");
            } else {
                result.expect_err("Should fail when sandbox-exec is not available");
            }
        }
    }

    #[test]
    fn test_direct_execution() {
        let config = SandboxConfig {
            tier: SandboxTier::TrustedAudited,
            network: false,
            mounts: vec![],
        };

        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = std::env::current_dir().unwrap();

        let output = executor.execute("echo", &["test"], &cwd).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test"));
    }
}
