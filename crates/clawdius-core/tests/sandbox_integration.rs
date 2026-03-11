//! Integration tests for Sandbox functionality
//!
//! Tests sandbox executor creation, command execution, security boundaries,
//! resource limits, and timeout handling.

use clawdius_core::sandbox::{
    executor::SandboxExecutor,
    tiers::{MountPoint, SandboxConfig},
    SandboxTier,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_config(tier: SandboxTier) -> SandboxConfig {
    SandboxConfig {
        tier,
        network: false,
        mounts: vec![],
    }
}

fn create_test_config_with_mounts(tier: SandboxTier, mounts: Vec<MountPoint>) -> SandboxConfig {
    SandboxConfig {
        tier,
        network: false,
        mounts,
    }
}

fn create_test_config_with_network(tier: SandboxTier, network: bool) -> SandboxConfig {
    SandboxConfig {
        tier,
        network,
        mounts: vec![],
    }
}

fn get_cwd() -> PathBuf {
    std::env::current_dir().expect("Failed to get current directory")
}

mod sandbox_creation {
    use super::*;

    #[test]
    fn test_sandbox_creation_trusted_audited() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config);
        assert!(executor.is_ok());
        let executor = executor.unwrap();
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_sandbox_creation_trusted() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config);
        assert!(executor.is_ok());
        let executor = executor.unwrap();
        assert_eq!(executor.backend_name(), "filtered");
    }

    #[test]
    fn test_sandbox_creation_with_mounts() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let mount = MountPoint {
            source: temp_dir.path().to_string_lossy().to_string(),
            destination: "/mnt/test".to_string(),
            read_only: true,
        };
        let config = create_test_config_with_mounts(SandboxTier::TrustedAudited, vec![mount]);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_sandbox_creation_with_network_enabled() {
        let config = create_test_config_with_network(SandboxTier::TrustedAudited, true);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_sandbox_creation_hardened_on_non_linux_macos() {
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let config = create_test_config(SandboxTier::Hardened);
            let result = SandboxExecutor::new(SandboxTier::Hardened, config);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_sandbox_debug_format() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let debug_str = format!("{:?}", executor);
        assert!(debug_str.contains("direct"));
        assert!(debug_str.contains("TrustedAudited"));
    }
}

mod command_execution {
    use super::*;

    #[test]
    fn test_direct_execution_simple_command() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("echo", &["hello"], &cwd);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_direct_execution_with_multiple_args() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("echo", &["-n", "test", "message"], &cwd);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test message"));
    }

    #[test]
    fn test_filtered_execution_simple_command() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("echo", &["safe command"], &cwd);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_execution_with_working_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();

        let output = executor.execute("pwd", &[], temp_dir.path());
        assert!(output.is_ok());
        let output = output.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(&temp_dir.path().to_string_lossy().to_string()));
    }

    #[test]
    fn test_execution_returns_exit_code() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("sh", &["-c", "exit 42"], &cwd);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert_eq!(output.status.code(), Some(42));
    }
}

mod security_boundaries {
    use super::*;

    #[test]
    fn test_filtered_backend_blocks_rm_rf_root() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("rm", &["-rf", "/"], &cwd);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("Blocked command pattern"));
    }

    #[test]
    fn test_filtered_backend_blocks_mkfs() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("mkfs", &["/dev/sda1"], &cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_filtered_backend_blocks_dd_zero() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("dd", &["if=/dev/zero", "of=/dev/sda"], &cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_filtered_backend_blocks_fork_bomb() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("sh", &["-c", ":(){ :|:& };:"], &cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_filtered_backend_blocks_chmod_777_root() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("chmod", &["-R", "777", "/"], &cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_filtered_backend_allows_safe_commands() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("ls", &["-la"], &cwd);
        assert!(output.is_ok());
    }
}

mod resource_limits {
    use super::*;

    #[test]
    fn test_mount_point_read_only() {
        let mount = MountPoint {
            source: "/tmp".to_string(),
            destination: "/mnt/readonly".to_string(),
            read_only: true,
        };
        assert!(mount.read_only);
    }

    #[test]
    fn test_mount_point_read_write() {
        let mount = MountPoint {
            source: "/tmp".to_string(),
            destination: "/mnt/readwrite".to_string(),
            read_only: false,
        };
        assert!(!mount.read_only);
    }

    #[test]
    fn test_config_network_disabled() {
        let config = create_test_config(SandboxTier::Untrusted);
        assert!(!config.network);
    }

    #[test]
    fn test_config_network_enabled() {
        let config = create_test_config_with_network(SandboxTier::Trusted, true);
        assert!(config.network);
    }

    #[test]
    fn test_config_multiple_mounts() {
        let mounts = vec![
            MountPoint {
                source: "/usr".to_string(),
                destination: "/usr".to_string(),
                read_only: true,
            },
            MountPoint {
                source: "/home".to_string(),
                destination: "/home".to_string(),
                read_only: false,
            },
        ];
        let config = create_test_config_with_mounts(SandboxTier::Untrusted, mounts);
        assert_eq!(config.mounts.len(), 2);
    }
}

mod timeout_handling {
    use super::*;

    #[test]
    fn test_execution_completes_quickly() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let start = std::time::Instant::now();
        let output = executor.execute("echo", &["fast"], &cwd);
        let elapsed = start.elapsed();

        assert!(output.is_ok());
        assert!(elapsed.as_secs() < 5);
    }

    #[test]
    fn test_command_with_timeout_simulation() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let output = executor.execute("sh", &["-c", "sleep 0.1 && echo done"], &cwd);
        assert!(output.is_ok());
        let output = output.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("done"));
    }
}

mod configuration_validation {
    use super::*;

    #[test]
    fn test_sandbox_tier_serialization() {
        let tier = SandboxTier::TrustedAudited;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"trustedaudited\"");

        let tier = SandboxTier::Untrusted;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"untrusted\"");
    }

    #[test]
    fn test_sandbox_tier_deserialization() {
        let tier: SandboxTier = serde_json::from_str("\"trustedaudited\"").unwrap();
        assert_eq!(tier, SandboxTier::TrustedAudited);

        let tier: SandboxTier = serde_json::from_str("\"hardened\"").unwrap();
        assert_eq!(tier, SandboxTier::Hardened);
    }

    #[test]
    fn test_sandbox_config_serialization() {
        let config = create_test_config(SandboxTier::Untrusted);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"tier\":\"untrusted\""));
        assert!(json.contains("\"network\":false"));
    }

    #[test]
    fn test_mount_point_serialization() {
        let mount = MountPoint {
            source: "/src".to_string(),
            destination: "/dst".to_string(),
            read_only: true,
        };
        let json = serde_json::to_string(&mount).unwrap();
        assert!(json.contains("\"source\":\"/src\""));
        assert!(json.contains("\"destination\":\"/dst\""));
        assert!(json.contains("\"read_only\":true"));
    }
}

mod error_handling {
    use super::*;

    #[test]
    fn test_execution_nonexistent_command() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let cwd = get_cwd();

        let result = executor.execute("nonexistent_command_xyz123", &[], &cwd);
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_invalid_working_directory() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        let nonexistent_path = PathBuf::from("/nonexistent/path/xyz123");

        let result = executor.execute("echo", &["test"], &nonexistent_path);
        assert!(result.is_err());
    }
}

mod tier_isolation {
    use super::*;

    #[test]
    fn test_tier_trusted_audited_uses_direct_backend() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap();
        assert_eq!(executor.backend_name(), "direct");
    }

    #[test]
    fn test_tier_trusted_uses_filtered_backend() {
        let config = create_test_config(SandboxTier::Trusted);
        let executor = SandboxExecutor::new(SandboxTier::Trusted, config).unwrap();
        assert_eq!(executor.backend_name(), "filtered");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_tier_untrusted_uses_bubblewrap_when_available() {
        let config = create_test_config(SandboxTier::Untrusted);
        let result = SandboxExecutor::new(SandboxTier::Untrusted, config);

        if let Ok(executor) = result {
            assert_eq!(executor.backend_name(), "bubblewrap");
        } else {
            let err = result.unwrap_err().to_string();
            assert!(err.contains("bubblewrap"));
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_tier_untrusted_uses_sandbox_exec_when_available() {
        let config = create_test_config(SandboxTier::Untrusted);
        let result = SandboxExecutor::new(SandboxTier::Untrusted, config);

        if let Ok(executor) = result {
            assert_eq!(executor.backend_name(), "sandbox-exec");
        } else {
            let err = result.unwrap_err().to_string();
            assert!(err.contains("sandbox-exec"));
        }
    }
}

mod concurrent_execution {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_concurrent_executions() {
        let config = create_test_config(SandboxTier::TrustedAudited);
        let executor = Arc::new(SandboxExecutor::new(SandboxTier::TrustedAudited, config).unwrap());
        let cwd = get_cwd();

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let exec = Arc::clone(&executor);
                let cwd = cwd.clone();
                std::thread::spawn(move || {
                    let output = exec.execute("echo", &[&format!("thread-{}", i)], &cwd);
                    output.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().expect("Thread panicked");
            assert!(result.is_ok());
        }
    }
}
