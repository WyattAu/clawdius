//! Sandbox Escape Test Suite
//!
//! Verifies that each sandbox backend properly isolates code and prevents
//! access to resources outside the permitted scope.

use clawdius_core::sandbox::backends::SandboxBackend;
use clawdius_core::sandbox::executor::SandboxExecutor;
use clawdius_core::sandbox::tiers::SandboxConfig;
use clawdius_core::sandbox::{backends, SandboxTier};

fn default_config(tier: SandboxTier) -> SandboxConfig {
    SandboxConfig {
        tier,
        network: false,
        mounts: vec![],
    }
}

fn cwd() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
}

// ---------------------------------------------------------------------------
// Filtered backend
// ---------------------------------------------------------------------------

mod filtered {
    use super::*;

    #[test]
    fn blocks_rm_rf_root() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("rm", &["-rf", "/"], &cwd());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Blocked command pattern"));
    }

    #[test]
    fn blocks_mkfs() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("mkfs.ext4", &["/dev/sda1"], &cwd());
        assert!(result.is_err());
    }

    #[test]
    fn blocks_dd_dev_zero() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("dd", &["if=/dev/zero", "of=/dev/sda"], &cwd());
        assert!(result.is_err());
    }

    #[test]
    fn blocks_fork_bomb_pattern() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute(":()", &[":|:& };:"], &cwd());
        assert!(result.is_err());
    }

    #[test]
    fn blocks_chmod_recursive_root() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("chmod", &["-R", "777", "/"], &cwd());
        assert!(result.is_err());
    }

    #[test]
    fn blocks_chown_recursive() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("chown", &["-R", "nobody:nobody", "/etc"], &cwd());
        assert!(result.is_err());
    }

    #[test]
    fn allows_echo() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("echo", &["hello"], &cwd());
        assert!(result.is_ok());
        let output = result.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn allows_ls() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("ls", &[], &cwd());
        assert!(result.is_ok());
    }

    #[test]
    fn name_is_filtered() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        assert_eq!(backend.name(), "filtered");
    }

    #[test]
    fn blocks_mv_to_dev_null() {
        let backend = backends::FilteredBackend::new(default_config(SandboxTier::Trusted));
        let result = backend.execute("mv", &["/*", "/dev/null"], &cwd());
        assert!(result.is_err());
    }
}

// ---------------------------------------------------------------------------
// Direct backend (no isolation — should allow everything)
// ---------------------------------------------------------------------------

mod direct {
    use super::*;

    #[test]
    fn allows_echo() {
        let backend = backends::DirectBackend::new(default_config(SandboxTier::TrustedAudited));
        let result = backend.execute("echo", &["hello"], &cwd());
        assert!(result.is_ok());
    }

    #[test]
    fn name_is_direct() {
        let backend = backends::DirectBackend::new(default_config(SandboxTier::TrustedAudited));
        assert_eq!(backend.name(), "direct");
    }
}

// ---------------------------------------------------------------------------
// Firecracker backend — sync execute must always refuse
// ---------------------------------------------------------------------------

mod firecracker {
    use super::*;

    #[test]
    fn refuses_sync_execution() {
        let backend = backends::FirecrackerBackend::with_defaults();
        let result = backend.execute("echo", &["hello"], &cwd());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("async") || msg.contains("execute_async"),
            "Expected async-related error, got: {msg}"
        );
    }

    #[test]
    fn name_is_firecracker() {
        let backend = backends::FirecrackerBackend::with_defaults();
        assert_eq!(backend.name(), "firecracker");
    }

    #[test]
    fn config_defaults() {
        let backend = backends::FirecrackerBackend::with_defaults();
        // Just ensure construction succeeds
        let _ = &backend;
    }
}

// ---------------------------------------------------------------------------
// Container backend — only run when runtime is available
// ---------------------------------------------------------------------------

mod container {
    use super::*;

    #[test]
    fn config_defaults_no_network() {
        let config = backends::ContainerConfig::default();
        assert!(!config.network);
        assert_eq!(config.base_image, "alpine:latest");
        assert!(config.memory_limit > 0);
    }

    #[test]
    fn mount_read_only() {
        let mount = backends::ContainerMount::new("/host", "/container").read_only();
        assert!(mount.read_only);
    }

    #[test]
    fn runtime_detect_does_not_panic() {
        let _ = backends::ContainerRuntime::detect();
    }

    #[test]
    fn is_available_does_not_panic() {
        let _ = backends::ContainerBackend::is_available();
    }

    #[test]
    fn sync_execute_requires_runtime() {
        if !backends::ContainerBackend::is_available() {
            return;
        }
        let backend = backends::ContainerBackend::with_defaults();
        let result = backend.execute("echo", &["hello"], &cwd());
        // Without a pulled image this will likely fail, but it shouldn't panic
        let _ = result;
    }
}

// ---------------------------------------------------------------------------
// gVisor backend
// ---------------------------------------------------------------------------

mod gvisor {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = backends::GVisorConfig::default();
        assert!(config.rootless);
        assert!(!config.network);
    }

    #[test]
    fn is_available_does_not_panic() {
        let _ = backends::GVisorBackend::is_available();
    }

    #[test]
    fn construction_succeeds() {
        let _ = backends::GVisorBackend::with_defaults();
    }
}

// ---------------------------------------------------------------------------
// SandboxExecutor — tier-based routing and fallback
// ---------------------------------------------------------------------------

mod executor {
    use super::*;

    #[test]
    fn trusted_audited_uses_direct() {
        let config = default_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn trusted_uses_filtered() {
        let config = default_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        assert_eq!(executor.backend_name(), "filtered");
    }

    #[test]
    fn fallback_always_succeeds() {
        let config = default_config(SandboxTier::Hardened);
        let executor = SandboxExecutor::new_with_fallback(SandboxTier::Hardened, config);
        // Should never panic — degrades to filtered at worst
        let _name = executor.backend_name();
    }

    #[test]
    fn untrusted_fallback_always_succeeds() {
        let config = default_config(SandboxTier::Untrusted);
        let executor = SandboxExecutor::new_with_fallback(SandboxTier::Untrusted, config);
        let _name = executor.backend_name();
    }

    #[test]
    fn direct_executor_runs_echo() {
        let config = default_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let output = executor.execute("echo", &["sandbox-ok"], &cwd()).unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("sandbox-ok"));
    }

    #[test]
    fn filtered_executor_blocks_rm_rf() {
        let config = default_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let result = executor.execute("rm", &["-rf", "/"], &cwd());
        assert!(result.is_err());
    }
}

// ---------------------------------------------------------------------------
// Backend detection helpers
// ---------------------------------------------------------------------------

mod detection {
    use super::*;

    #[test]
    fn detect_best_backend_does_not_panic() {
        let _ = backends::detect_best_backend();
    }

    #[test]
    fn list_available_backends_does_not_panic() {
        let list = backends::list_available_backends();
        // Direct is always available
        assert!(list.iter().any(|(name, avail)| *name == "direct" && *avail));
    }

    #[test]
    fn is_bwrap_available_does_not_panic() {
        let _ = backends::is_bwrap_available();
    }

    #[test]
    fn is_container_available_does_not_panic() {
        let _ = backends::is_container_available();
    }

    #[test]
    fn is_gvisor_available_does_not_panic() {
        let _ = backends::is_gvisor_available();
    }

    #[test]
    fn is_firecracker_available_does_not_panic() {
        let _ = backends::is_firecracker_available();
    }

    #[test]
    fn is_kvm_available_does_not_panic() {
        let _ = backends::is_kvm_available();
    }
}
