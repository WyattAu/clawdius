//! Firecracker microVM sandbox backend.
//!
//! Provides hardware-isolated VM execution via Firecracker microVMs.
//! Each execution runs in a lightweight VM with its own kernel, memory, and network.
//!
//! **Status:** Stub -- requires `firecracker` binary and a root filesystem image.
//! **Planned for:** v1.7.0
//! **Prerequisite:** Firecracker installation (https://github.com/firecracker-microvm/firecracker)

use super::SandboxBackend;
use crate::error::Result;
use crate::sandbox::tiers::SandboxConfig;
use std::path::Path;
use std::process::{Command, Output};

/// Default root filesystem for sandboxed execution.
const DEFAULT_ROOTFS: &str = "/opt/clawdius/rootfs.ext4";

/// Default kernel image for microVM.
const DEFAULT_KERNEL: &str = "/opt/clawdius/vmlinux";

/// Firecracker microVM sandbox backend.
#[allow(dead_code)]
pub struct FirecrackerBackend {
    config: SandboxConfig,
    kernel_path: String,
    rootfs_path: String,
}

impl FirecrackerBackend {
    /// Create a new Firecracker backend with default kernel and rootfs paths.
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            kernel_path: DEFAULT_KERNEL.to_string(),
            rootfs_path: DEFAULT_ROOTFS.to_string(),
        }
    }

    /// Create with custom kernel and rootfs paths.
    #[allow(dead_code)]
    pub fn with_paths(config: SandboxConfig, kernel: &str, rootfs: &str) -> Self {
        Self {
            config,
            kernel_path: kernel.to_string(),
            rootfs_path: rootfs.to_string(),
        }
    }

    /// Check if Firecracker is available.
    pub fn is_available() -> bool {
        if Command::new("firecracker")
            .arg("--version")
            .output()
            .is_err()
        {
            return false;
        }
        Path::new(DEFAULT_KERNEL).exists()
    }
}

impl SandboxBackend for FirecrackerBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        // Firecracker requires a JSON configuration for the microVM.
        // In production, this would:
        // 1. Generate a unique VM ID
        // 2. Create a jailer configuration
        // 3. Launch firecracker with the VM config
        // 4. Execute the command inside the VM via MMDS
        // 5. Collect output and tear down the VM
        //
        // Current stub: delegate to container runtime as fallback.

        let mut docker_args: Vec<String> = vec![
            "run".into(),
            "--rm".into(),
            "-v".into(),
            format!("{}:/workspace", cwd.display()),
            "-w".into(),
            "/workspace".into(),
        ];

        if !self.config.network {
            docker_args.push("--network=none".into());
        }

        for mount in &self.config.mounts {
            let flag = if mount.read_only { ":ro" } else { "" };
            docker_args.push("-v".into());
            docker_args.push(format!("{}:{}{}", mount.source, mount.destination, flag));
        }

        docker_args.push("alpine".into());
        docker_args.push(command.into());
        for arg in args {
            docker_args.push((*arg).into());
        }

        let output = Command::new("docker").args(&docker_args).output()?;

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "firecracker"
    }
}
