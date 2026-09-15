//! Sandbox executor

use crate::error::{Error, Result};
use crate::sandbox::backends::ContainerBackend;
use crate::sandbox::backends::{DirectBackend, FilteredBackend, SandboxBackend};
use crate::sandbox::tiers::SandboxConfig;
use crate::sandbox::SandboxTier;
use std::path::Path;
use std::process::Output;
use std::sync::Arc;

#[cfg(target_os = "linux")]
use crate::sandbox::backends::BubblewrapBackend;

#[cfg(target_os = "macos")]
use crate::sandbox::backends::SandboxExecBackend;

/// Build the typed error returned when no real isolation backend is available
/// and unisolated execution was not explicitly opted into.
///
/// There is intentionally **no** warning-and-execute path behind this: the
/// command is never run.
fn sandbox_unavailable_error() -> Error {
    Error::SandboxUnavailable(
        "no sandbox isolation backend is available (tried gVisor, Firecracker, \
         Docker/Podman, bubblewrap/sandbox-exec); refusing to execute with the \
         unisolated `filtered` backend (command blocklist only — trivially bypassed)."
            .to_string()
            + "\n\n\
               Remediation (choose one):\n  \
               1. Install a real isolation backend: bubblewrap \
               (`apt install bubblewrap`), Docker, or Podman.\n  \
               2. Explicitly accept unisolated execution by setting in clawdius.toml:\n       \
               [shell_sandbox]\n       \
               allow_unisolated = true",
    )
}

pub struct SandboxExecutor {
    backend: Arc<dyn SandboxBackend>,
    tier: SandboxTier,
    allow_unisolated: bool,
}

impl std::fmt::Debug for SandboxExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxExecutor")
            .field("backend", &self.backend.name())
            .field("tier", &format!("{:?}", self.tier))
            .field("allow_unisolated", &self.allow_unisolated)
            .finish()
    }
}

impl SandboxExecutor {
    /// Create a sandbox executor for the given tier.
    ///
    /// Backend selection:
    ///
    /// - `TrustedAudited` — `direct` execution (zero isolation). The tier
    ///   itself is the explicit opt-in; a warning is logged.
    /// - `Trusted` — the unisolated `filtered` backend (blocklist only).
    ///   Because the blocklist is trivially bypassed, this backend is only
    ///   used when [`SandboxConfig::allow_unisolated`] is `true`; otherwise
    ///   construction fails with [`Error::SandboxUnavailable`].
    /// - `Untrusted` / `Hardened` — best available isolation backend
    ///   (container > bubblewrap/sandbox-exec). If none is available: the
    ///   `filtered` backend when `allow_unisolated` is `true`, otherwise
    ///   [`Error::SandboxUnavailable`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::SandboxUnavailable`] when no real isolation backend
    /// is available and `config.allow_unisolated` is `false`. The proposed
    /// command must not be executed in that case.
    pub fn new(tier: SandboxTier, config: SandboxConfig) -> Result<Self> {
        let allow_unisolated = config.allow_unisolated;
        let backend: Arc<dyn SandboxBackend> = match tier {
            SandboxTier::TrustedAudited => {
                tracing::warn!(
                    "TrustedAudited tier uses direct execution with NO sandboxing. \
                     Only use for fully trusted, audited code."
                );
                Arc::new(DirectBackend::new(config))
            },
            SandboxTier::Trusted => {
                if !allow_unisolated {
                    return Err(sandbox_unavailable_error());
                }
                tracing::warn!(
                    "Trusted tier uses filtered execution (command blocklist only). \
                     This is NOT a real sandbox — payloads can bypass the blocklist \
                     via interpreters, flag reordering, etc. Explicitly allowed via \
                     `allow_unisolated = true`."
                );
                Arc::new(FilteredBackend::new(config))
            },
            SandboxTier::Untrusted | SandboxTier::Hardened => {
                Self::platform_sandbox(config, allow_unisolated)?
            },
        };
        Ok(Self {
            backend,
            tier,
            allow_unisolated,
        })
    }

    /// Create a sandbox executor using the best available backend for the
    /// given tier, with cascading fallback.
    ///
    /// Cascade order for `Untrusted`/`Hardened`: Container (Docker/Podman) >
    /// Bubblewrap/Sandbox-exec. **The cascade never silently degrades to the
    /// unisolated `filtered` backend**: if no isolation backend is available,
    /// construction fails with [`Error::SandboxUnavailable`] unless
    /// [`SandboxConfig::allow_unisolated`] is explicitly `true`.
    ///
    /// # Errors
    ///
    /// Same as [`SandboxExecutor::new`].
    pub fn new_with_fallback(tier: SandboxTier, config: SandboxConfig) -> Result<Self> {
        Self::new(tier, config)
    }

    /// Select the best available isolation backend for `Untrusted`/`Hardened`
    /// tiers, degrading to `filtered` **only** when explicitly allowed.
    ///
    /// Priority: Container (Docker/Podman with `--rm --network=none`) >
    /// Bubblewrap (Linux) / sandbox-exec (macOS).
    ///
    /// The `direct` backend is never used as a fallback, and the `filtered`
    /// backend is only used when `allow_unisolated` is `true` — otherwise
    /// this returns [`Error::SandboxUnavailable`] and the caller must refuse
    /// to execute.
    fn platform_sandbox(
        config: SandboxConfig,
        allow_unisolated: bool,
    ) -> Result<Arc<dyn SandboxBackend>> {
        // Priority 1: Container (Docker/Podman with --rm --network=none)
        if ContainerBackend::is_available() {
            return Ok(Arc::new(ContainerBackend::with_defaults()));
        }

        // Priority 2: Platform sandbox (Bubblewrap on Linux, sandbox-exec on macOS)
        #[cfg(target_os = "linux")]
        if BubblewrapBackend::is_available() {
            return Ok(Arc::new(BubblewrapBackend::new(config)));
        }

        #[cfg(target_os = "macos")]
        if SandboxExecBackend::is_available() {
            return Ok(Arc::new(SandboxExecBackend::new(config)));
        }

        // No real isolation backend. Degrade to the (weak) filtered backend
        // only when the user explicitly opted in; otherwise this is a hard stop.
        if allow_unisolated {
            tracing::warn!(
                "No sandbox isolation backend available. \
                 Degrading to filtered execution (command blocklist only) \
                 as explicitly allowed by `allow_unisolated = true`."
            );
            return Ok(Arc::new(FilteredBackend::new(config)));
        }

        Err(sandbox_unavailable_error())
    }

    pub fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        self.backend.execute(command, args, cwd)
    }

    /// Execute a command asynchronously by running the synchronous backend
    /// on a blocking thread pool.
    ///
    /// The configured backend is the single source of truth for execution:
    /// there is no per-call fallback. If the executor was constructed at all,
    /// a real isolation backend (or an explicitly allowed `filtered` backend)
    /// is in use.
    ///
    /// # Errors
    ///
    /// Propagates backend execution errors and task join errors.
    pub async fn execute_async(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let backend = Arc::clone(&self.backend);
        let command = command.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let cwd = cwd.to_path_buf();

        let _ = self.tier; // tier is exercised during construction; backend owns execution

        tokio::task::spawn_blocking(move || {
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            backend.execute(&command, &arg_refs, &cwd)
        })
        .await
        .map_err(|e| Error::Sandbox(format!("Sandbox task join error: {e}")))?
    }

    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    /// Whether this executor may fall back to unisolated (`filtered`)
    /// execution when no real isolation backend is available.
    #[must_use]
    pub fn allows_unisolated(&self) -> bool {
        self.allow_unisolated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(tier: SandboxTier, allow_unisolated: bool) -> SandboxConfig {
        SandboxConfig {
            tier,
            network: false,
            mounts: vec![],
            allow_unisolated,
        }
    }

    #[test]
    fn test_executor_trusted_audited() {
        let config = make_config(SandboxTier::TrustedAudited, false);

        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_executor_trusted_without_opt_in_refuses() {
        let config = make_config(SandboxTier::Trusted, false);

        let result = SandboxExecutor::new(SandboxTier::Trusted, config);
        let err = result.expect_err("Trusted tier must refuse without allow_unisolated");
        assert!(matches!(err, Error::SandboxUnavailable(_)));
        let msg = err.to_string();
        assert!(msg.contains("Sandbox unavailable"));
        assert!(msg.contains("allow_unisolated"));
        assert!(msg.contains("bubblewrap"));
    }

    #[test]
    fn test_executor_trusted_opt_in_uses_filtered() {
        let config = make_config(SandboxTier::Trusted, true);

        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        assert_eq!(executor.backend_name(), "filtered");
        assert!(executor.allows_unisolated());
    }

    #[test]
    fn test_executor_untrusted_no_opt_in_refuses_or_uses_real_backend() {
        let config = make_config(SandboxTier::Untrusted, false);

        match SandboxExecutor::new_with_fallback(SandboxTier::Untrusted, config) {
            Ok(executor) => {
                // A real isolation backend exists on this host.
                let name = executor.backend_name();
                assert_ne!(name, "filtered");
                assert_ne!(name, "direct");
            },
            Err(err) => {
                assert!(matches!(err, Error::SandboxUnavailable(_)));
                let msg = err.to_string();
                assert!(
                    msg.contains("Docker") || msg.contains("bubblewrap"),
                    "error should mention remediation backends: {msg}"
                );
            },
        }
    }

    #[test]
    fn test_executor_untrusted_opt_in_never_fails() {
        let config = make_config(SandboxTier::Untrusted, true);

        let executor = SandboxExecutor::new_with_fallback(SandboxTier::Untrusted, config)
            .expect("opted-in fallback must always succeed");
        let _name = executor.backend_name();
    }

    #[test]
    fn test_executor_new_with_fallback_trusted_audited() {
        let config = make_config(SandboxTier::TrustedAudited, false);

        let executor = SandboxExecutor::new_with_fallback(SandboxTier::TrustedAudited, config)
            .expect("TrustedAudited never refuses");
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_executor_hardened_without_opt_in_matches_untrusted() {
        let config = make_config(SandboxTier::Hardened, false);

        match SandboxExecutor::new_with_fallback(SandboxTier::Hardened, config) {
            Ok(executor) => {
                let name = executor.backend_name();
                assert_ne!(name, "filtered");
                assert_ne!(name, "direct");
            },
            Err(err) => {
                assert!(matches!(err, Error::SandboxUnavailable(_)));
            },
        }
    }

    #[test]
    fn test_direct_execution() {
        let config = make_config(SandboxTier::TrustedAudited, false);

        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = std::env::current_dir().unwrap();

        let output = executor.execute("echo", &["test"], &cwd).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_execute_async_uses_configured_backend() {
        let config = make_config(SandboxTier::Trusted, true);

        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = std::env::current_dir().unwrap();

        let output = executor
            .execute_async("echo", &["async-ok"], &cwd)
            .await
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("async-ok"));
    }

    #[tokio::test]
    async fn test_execute_async_trusted_applies_blocklist() {
        let config = make_config(SandboxTier::Trusted, true);

        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = std::env::current_dir().unwrap();

        let result = executor.execute_async("rm", &["-rf", "/"], &cwd).await;
        assert!(result.is_err(), "blocklist must apply on the async path");
    }

    #[tokio::test]
    async fn test_execute_async_trusted_without_opt_in_refuses() {
        let config = make_config(SandboxTier::Trusted, false);

        let result = SandboxExecutor::new(SandboxTier::Trusted, config);
        assert!(
            result.is_err(),
            "construction must refuse before any async execution"
        );
    }

    #[test]
    fn test_sandbox_unavailable_error_message_is_actionable() {
        let err = sandbox_unavailable_error();
        let msg = err.to_string();
        assert!(msg.contains("Sandbox unavailable"));
        assert!(msg.contains("filtered"));
        assert!(msg.contains("allow_unisolated"));
        assert!(msg.contains("bubblewrap"));
        assert!(msg.contains("Docker"));
        assert!(msg.contains("gVisor"));
    }
}
