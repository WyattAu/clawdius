//! gVisor (runsc) sandbox backend
//!
//! gVisor provides strong isolation using a userspace kernel.
//! This backend requires runsc to be installed.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::Output;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::SandboxBackend;

/// gVisor sandbox configuration
#[derive(Debug, Clone)]
pub struct GVisorConfig {
    /// Whether to use network namespace
    pub network: bool,
    /// Whether to use rootless mode
    pub rootless: bool,
    /// Memory limit in bytes
    pub memory_limit: u64,
    /// CPU limit (fraction, 0.0-1.0)
    pub cpu_limit: f64,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Working directory inside sandbox
    pub workdir: String,
    /// Additional mounts
    pub mounts: Vec<MountSpec>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Strace logging enabled
    pub strace: bool,
    /// Platform to use (ptrace, kvm, systrap)
    pub platform: String,
}

impl Default for GVisorConfig {
    fn default() -> Self {
        Self {
            network: false,
            rootless: true,
            memory_limit: 512 * 1024 * 1024, // 512 MB
            cpu_limit: 1.0,
            timeout_secs: 300,
            workdir: "/workspace".to_string(),
            mounts: Vec::new(),
            env: Vec::new(),
            strace: false,
            platform: "systrap".to_string(),
        }
    }
}

/// Mount specification
#[derive(Debug, Clone)]
pub struct MountSpec {
    /// Source path on host
    pub source: String,
    /// Destination path in sandbox
    pub destination: String,
    /// Mount options (e.g., "ro", "rw")
    pub options: String,
}

/// gVisor sandbox backend
pub struct GVisorBackend {
    config: GVisorConfig,
    container_counter: Arc<RwLock<u64>>,
}

impl GVisorBackend {
    /// Create a new gVisor backend
    #[must_use]
    pub fn new(config: GVisorConfig) -> Self {
        Self {
            config,
            container_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(GVisorConfig::default())
    }

    /// Check if gVisor (runsc) is available
    #[must_use]
    pub fn is_available() -> bool {
        std::process::Command::new("runsc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Generate a unique container name
    async fn generate_container_name(&self) -> String {
        let mut counter = self.container_counter.write().await;
        *counter += 1;
        format!("clawdius-gvisor-{}", *counter)
    }

    /// Build the runsc command
    #[allow(clippy::vec_init_then_push)]
    fn build_run_command(
        &self,
        name: &str,
        command: &str,
        args: &[&str],
        cwd: &Path,
        detach: bool,
    ) -> Vec<String> {
        let mut cmd = Vec::new();

        // Base command
        cmd.push("run".to_string());
        if detach {
            cmd.push("--detach".to_string());
        }
        cmd.push("--name".to_string());
        cmd.push(name.to_string());

        // Platform
        cmd.push("--platform".to_string());
        cmd.push(self.config.platform.clone());

        // Rootless mode
        if self.config.rootless {
            cmd.push("--rootless".to_string());
        }

        // Network
        if !self.config.network {
            cmd.push("--network=none".to_string());
        }

        // Memory limit
        if self.config.memory_limit > 0 {
            cmd.push("--memory".to_string());
            cmd.push(format!("{}b", self.config.memory_limit));
        }

        // CPU limit
        if self.config.cpu_limit > 0.0 {
            cmd.push("--cpus".to_string());
            cmd.push(format!("{}", self.config.cpu_limit));
        }

        // Timeout
        if self.config.timeout_secs > 0 {
            cmd.push("--timeout".to_string());
            cmd.push(format!("{}s", self.config.timeout_secs));
        }

        // Strace logging
        if self.config.strace {
            cmd.push("--strace".to_string());
        }

        // Working directory
        cmd.push("--cwd".to_string());
        cmd.push(self.config.workdir.clone());

        // Mounts
        let cwd_str = cwd.to_string_lossy();
        cmd.push("--mount".to_string());
        cmd.push(format!(
            "type=bind,src={},dst={}",
            cwd_str, self.config.workdir
        ));

        for mount in &self.config.mounts {
            cmd.push("--mount".to_string());
            cmd.push(format!(
                "type=bind,src={},dst={},options={}",
                mount.source, mount.destination, mount.options
            ));
        }

        // Environment variables
        for (key, value) in &self.config.env {
            cmd.push("--env".to_string());
            cmd.push(format!("{key}={value}"));
        }

        // Image (use minimal alpine)
        cmd.push("alpine:latest".to_string());

        // Command and args
        cmd.push("--".to_string());
        cmd.push(command.to_string());
        for arg in args {
            cmd.push(arg.to_string());
        }

        cmd
    }

    /// Execute a command in gVisor
    pub async fn execute_async(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let name = self.generate_container_name().await;
        let cmd_args = self.build_run_command(&name, command, args, cwd, true);

        let output = tokio::process::Command::new("runsc")
            .args(&cmd_args)
            .current_dir(cwd)
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to execute gVisor: {e}")))?;

        // Cleanup container
        let _ = tokio::process::Command::new("runsc")
            .args(["delete", "--force", &name])
            .output()
            .await;

        Ok(output)
    }

    /// List running containers
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let output = tokio::process::Command::new("runsc")
            .args(["list", "--format", "{{.Name}}\t{{.Status}}"])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to list containers: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    Some(ContainerInfo {
                        name: parts[0].to_string(),
                        status: parts[1].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(containers)
    }

    /// Kill a container
    pub async fn kill_container(&self, name: &str) -> Result<()> {
        let output = tokio::process::Command::new("runsc")
            .args(["kill", name])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to kill container: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Sandbox(format!(
                "Failed to kill container: {stderr}"
            )));
        }

        Ok(())
    }

    /// Delete a container
    pub async fn delete_container(&self, name: &str) -> Result<()> {
        let output = tokio::process::Command::new("runsc")
            .args(["delete", "--force", name])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to delete container: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Sandbox(format!(
                "Failed to delete container: {stderr}"
            )));
        }

        Ok(())
    }

    /// Cleanup all clawdius containers
    pub async fn cleanup(&self) -> Result<()> {
        let containers = self.list_containers().await?;

        for container in containers {
            if container.name.starts_with("clawdius-") {
                self.kill_container(&container.name).await.ok();
                self.delete_container(&container.name).await.ok();
            }
        }

        Ok(())
    }
}

/// Container information
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container name
    pub name: String,
    /// Container status
    pub status: String,
}

impl SandboxBackend for GVisorBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let name = format!("clawdius-gvisor-{}", chrono::Utc::now().timestamp());
        // Run in foreground (no --detach) so we capture stdout/stderr.
        let cmd_args = self.build_run_command(&name, command, args, cwd, false);

        let output = std::process::Command::new("runsc")
            .args(&cmd_args)
            .current_dir(cwd)
            .output()
            .map_err(|e| Error::Sandbox(format!("Failed to execute gVisor: {e}")))?;

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "gvisor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gvisor_config_defaults() {
        let config = GVisorConfig::default();
        assert!(config.rootless);
        assert!(!config.network);
    }

    #[test]
    fn test_gvisor_backend_creation() {
        let _backend = GVisorBackend::with_defaults();
        // Test passes if creation succeeds without panic
    }
}
