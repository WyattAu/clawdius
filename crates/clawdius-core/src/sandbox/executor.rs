//! Sandbox executor

use crate::error::{Error, Result};
use crate::sandbox::backends::ContainerBackend;
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
            SandboxTier::TrustedAudited => {
                tracing::warn!(
                    "TrustedAudited tier uses direct execution with NO sandboxing. \
                     Only use for fully trusted, audited code."
                );
                Box::new(DirectBackend::new(config))
            },
            SandboxTier::Trusted => {
                tracing::warn!(
                    "Trusted tier uses filtered execution (command blocklist only). \
                     This is NOT a real sandbox — payloads can bypass the blocklist \
                     via interpreters, flag reordering, etc."
                );
                Box::new(FilteredBackend::new(config))
            },
            SandboxTier::Untrusted | SandboxTier::Hardened => Self::platform_sandbox(config)?,
        };
        Ok(Self { backend, tier })
    }

    /// Create a sandbox executor that uses the best available backend
    /// for the given tier, with cascading fallback.
    ///
    /// For `TrustedAudited` and `Trusted` tiers, this always uses `direct` and
    /// `filtered` respectively — these tiers intentionally opt out of real
    /// isolation. See [`SandboxTier`] documentation for security implications.
    pub fn new_with_fallback(tier: SandboxTier, config: SandboxConfig) -> Self {
        let backend: Box<dyn SandboxBackend> = match tier {
            SandboxTier::TrustedAudited => {
                tracing::warn!(
                    "TrustedAudited tier uses direct execution with NO sandboxing. \
                     Only use for fully trusted, audited code."
                );
                Box::new(DirectBackend::new(config))
            },
            SandboxTier::Trusted => {
                tracing::warn!(
                    "Trusted tier uses filtered execution (command blocklist only). \
                     This is NOT a real sandbox — payloads can bypass the blocklist \
                     via interpreters, flag reordering, etc."
                );
                Box::new(FilteredBackend::new(config))
            },
            SandboxTier::Untrusted | SandboxTier::Hardened => Self::best_available_sandbox(config),
        };
        Self { backend, tier }
    }

    /// Select the best available sandbox backend with cascading priority:
    ///
    /// gVisor (kernel intercept) > Container (process isolation) >
    /// Bubblewrap/Sandbox-exec (namespace/seatbelt) > Filtered (degraded, no real isolation)
    ///
    /// **The `direct` backend is never used as a fallback** — if all isolation
    /// backends are unavailable, we degrade to `filtered` (blocklist) rather
    /// than running with zero protection.
    fn best_available_sandbox(config: SandboxConfig) -> Box<dyn SandboxBackend> {
        // Priority 1: Container (Docker/Podman with --rm --network=none)
        if ContainerBackend::is_available() {
            return Box::new(ContainerBackend::with_defaults());
        }

        // Priority 3: Platform sandbox (Bubblewrap on Linux, sandbox-exec on macOS)
        #[cfg(target_os = "linux")]
        if BubblewrapBackend::is_available() {
            return Box::new(BubblewrapBackend::new(config));
        }

        #[cfg(target_os = "macos")]
        if SandboxExecBackend::is_available() {
            return Box::new(SandboxExecBackend::new(config));
        }

        // Priority 4: Filtered (degraded — command blocklist only, NO real isolation).
        // We intentionally do NOT fall back to `direct` here. A blocklist is weak
        // but still better than nothing.
        tracing::error!(
            "No sandbox isolation backend available. \
             Falling back to filtered execution (command blocklist only). \
             Install Docker/Podman or bubblewrap for proper isolation."
        );
        Box::new(FilteredBackend::new(config))
    }

    #[cfg(target_os = "linux")]
    fn platform_sandbox(config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        // Try Container backend (Docker/Podman)
        if ContainerBackend::is_available() {
            return Ok(Box::new(ContainerBackend::with_defaults()));
        }

        // Fall back to Bubblewrap
        if BubblewrapBackend::is_available() {
            return Ok(Box::new(BubblewrapBackend::new(config)));
        }

        Err(Error::Sandbox(
            "No sandbox backend available for Untrusted/Hardened tiers. \
             Install one of: gVisor (runsc), Docker/Podman, or bubblewrap (bwrap)."
                .to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    fn platform_sandbox(config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        // Try Container backend first (Docker/Podman)
        if ContainerBackend::is_available() {
            return Ok(Box::new(ContainerBackend::with_defaults()));
        }

        if SandboxExecBackend::is_available() {
            return Ok(Box::new(SandboxExecBackend::new(config)));
        }

        Err(Error::Sandbox(
            "No sandbox backend available for Untrusted/Hardened tiers on macOS. \
             Install Docker/Podman or sandbox-exec."
                .to_string(),
        ))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn platform_sandbox(_config: SandboxConfig) -> Result<Box<dyn SandboxBackend>> {
        if ContainerBackend::is_available() {
            return Ok(Box::new(ContainerBackend::with_defaults()));
        }

        Err(Error::Sandbox(
            "Platform sandboxing requires Docker/Podman on this platform".to_string(),
        ))
    }

    pub fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        self.backend.execute(command, args, cwd)
    }

    #[must_use]
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

        // new_with_fallback always succeeds by cascading through backends.
        let executor = SandboxExecutor::new_with_fallback(SandboxTier::Untrusted, config);
        // The backend name depends on what's installed; just verify it doesn't panic.
        let _name = executor.backend_name();
    }

    #[test]
    fn test_executor_new_with_fallback_trusted_audited() {
        let config = SandboxConfig {
            tier: SandboxTier::TrustedAudited,
            network: true,
            mounts: vec![],
        };

        let executor = SandboxExecutor::new_with_fallback(SandboxTier::TrustedAudited, config);
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_executor_hardened_same_as_untrusted() {
        let config = SandboxConfig {
            tier: SandboxTier::Hardened,
            network: false,
            mounts: vec![],
        };

        // Hardened and Untrusted use the same backend selection.
        let executor = SandboxExecutor::new_with_fallback(SandboxTier::Hardened, config);
        let _name = executor.backend_name();
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
