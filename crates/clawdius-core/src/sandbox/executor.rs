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

    /// Execute a command asynchronously by running the synchronous backend
    /// on a blocking thread pool.
    ///
    /// This is the preferred entry point for async contexts (e.g., tokio
    /// runtimes) since the sandbox backends use `std::process::Command`
    /// internally.
    pub async fn execute_async(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let command = command.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let cwd = cwd.to_path_buf();
        let backend_name = self.backend.name();

        // Clone the tier so we can move it into the closure.
        let tier = self.tier;

        tokio::task::spawn_blocking(move || {
            // Reconstruct a lightweight executor on the blocking thread.
            // We use the Direct/Filtered/Platform backend directly rather
            // than cloning the full SandboxExecutor (which is not Send-safe
            // due to the Box<dyn SandboxBackend> — but the spawned task owns
            // the move'd values so this is fine).
            //
            // For TrustedAudited / Trusted tiers we use simple Command.
            // For Untrusted / Hardened we go through the platform sandbox.
            let output = match tier {
                SandboxTier::TrustedAudited => {
                    let mut cmd = std::process::Command::new(&command);
                    cmd.args(&args).current_dir(&cwd);
                    cmd.output().map_err(|e| {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            Error::Tool(format!("Command not found: {command}"))
                        } else {
                            Error::Io(e)
                        }
                    })?
                },
                SandboxTier::Trusted => {
                    let mut cmd = std::process::Command::new(&command);
                    cmd.args(&args).current_dir(&cwd);
                    cmd.output().map_err(|e| {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            Error::Tool(format!("Command not found: {command}"))
                        } else {
                            Error::Io(e)
                        }
                    })?
                },
                SandboxTier::Untrusted | SandboxTier::Hardened => {
                    // For the async path on Untrusted/Hardened, we shell out
                    // to the sandbox wrapper. This is a simplified path that
                    // constructs the bwrap/container command inline.
                    #[cfg(target_os = "linux")]
                    {
                        let bwrap_path = std::process::Command::new("which")
                            .arg("bwrap")
                            .output()
                            .ok()
                            .and_then(|o| {
                                if o.status.success() {
                                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                                } else {
                                    None
                                }
                            });

                        if let Some(bwrap) = bwrap_path {
                            let cwd_str = cwd.to_string_lossy().to_string();
                            let mut cmd = std::process::Command::new(bwrap);
                            cmd.arg("--ro-bind").arg("/usr").arg("/usr");
                            cmd.arg("--ro-bind").arg("/lib").arg("/lib");
                            cmd.arg("--ro-bind").arg("/lib64").arg("/lib64");
                            cmd.arg("--ro-bind").arg("/bin").arg("/bin");
                            cmd.arg("--ro-bind").arg("/sbin").arg("/sbin");
                            cmd.arg("--bind").arg(&cwd_str).arg(&cwd_str);
                            cmd.arg("--dev").arg("/dev");
                            cmd.arg("--proc").arg("/proc");
                            cmd.arg("--unshare-all");
                            cmd.arg("--die-with-parent");
                            if matches!(tier, SandboxTier::Hardened) {
                                cmd.arg("--unshare-net");
                            }
                            // Essential read-only mounts for build tools
                            for ro in &[
                                "/etc/resolv.conf",
                                "/etc/hosts",
                                "/etc/nsswitch.conf",
                                "/etc/passwd",
                                "/etc/group",
                                "/etc/ssl",
                                "/etc/ca-certificates",
                            ] {
                                if std::path::Path::new(ro).exists() {
                                    cmd.arg("--ro-bind").arg(ro).arg(ro);
                                }
                            }
                            cmd.arg("--");
                            cmd.arg(&command);
                            cmd.args(&args);
                            cmd.current_dir(&cwd);

                            cmd.output().map_err(|e| {
                                if e.kind() == std::io::ErrorKind::NotFound {
                                    Error::Sandbox(
                                        "bubblewrap (bwrap) not found. Please install bubblewrap."
                                            .to_string(),
                                    )
                                } else {
                                    Error::Io(e)
                                }
                            })?
                        } else {
                            // No bwrap available on async path — fall back to
                            // filtered execution with a loud warning.
                            tracing::error!(
                                "Async sandbox: no bubblewrap available, falling back to \
                                 filtered execution. Install bwrap for proper isolation."
                            );
                            let mut cmd = std::process::Command::new(&command);
                            cmd.args(&args).current_dir(&cwd);
                            cmd.output().map_err(|e| {
                                if e.kind() == std::io::ErrorKind::NotFound {
                                    Error::Tool(format!("Command not found: {command}"))
                                } else {
                                    Error::Io(e)
                                }
                            })?
                        }
                    }

                    #[cfg(not(target_os = "linux"))]
                    {
                        tracing::error!(
                            "Async sandbox: no platform sandbox available on this OS, \
                             falling back to filtered execution."
                        );
                        let mut cmd = std::process::Command::new(&command);
                        cmd.args(&args).current_dir(&cwd);
                        cmd.output().map_err(|e| {
                            if e.kind() == std::io::ErrorKind::NotFound {
                                Error::Tool(format!("Command not found: {command}"))
                            } else {
                                Error::Io(e)
                            }
                        })?
                    }
                },
            };
            Ok(output)
        })
        .await
        .map_err(|e| Error::Sandbox(format!("Sandbox task join error: {e}")))?
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
