//! Container-based sandbox backend using Docker/Podman
//!
//! This backend provides strong isolation by running commands inside
//! containers with resource limits and security constraints.

use crate::error::{Error, Result};
use std::path::Path;
use std::process::Output;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::SandboxBackend;

/// Container runtime type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerRuntime {
    /// Docker runtime
    Docker,
    /// Podman runtime
    Podman,
}

impl ContainerRuntime {
    /// Get the command name
    #[must_use]
    pub fn command(&self) -> &'static str {
        match self {
            ContainerRuntime::Docker => "docker",
            ContainerRuntime::Podman => "podman",
        }
    }

    /// Check if the runtime is available
    #[must_use]
    pub fn is_available(&self) -> bool {
        std::process::Command::new(self.command())
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Detect the best available runtime
    #[must_use]
    pub fn detect() -> Option<Self> {
        if Self::Docker.is_available() {
            Some(Self::Docker)
        } else if Self::Podman.is_available() {
            Some(Self::Podman)
        } else {
            None
        }
    }
}

/// Container sandbox configuration
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Container runtime to use
    pub runtime: ContainerRuntime,
    /// Base image for containers
    pub base_image: String,
    /// Memory limit in bytes (0 = unlimited)
    pub memory_limit: u64,
    /// CPU limit (0.0 = unlimited, 1.0 = 1 CPU)
    pub cpu_limit: f64,
    /// Time limit in seconds (0 = unlimited)
    pub time_limit: u64,
    /// Whether to allow network access
    pub network: bool,
    /// Additional mount points
    pub mounts: Vec<ContainerMount>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Working directory inside container
    pub workdir: String,
    /// User to run as (e.g., "1000:1000")
    pub user: Option<String>,
    /// Security options (e.g., ["no-new-privileges"])
    pub security_opts: Vec<String>,
    /// Whether to keep the container after execution
    pub keep_container: bool,
    /// Container name prefix
    pub name_prefix: String,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            runtime: ContainerRuntime::detect().unwrap_or(ContainerRuntime::Docker),
            base_image: "alpine:latest".to_string(),
            memory_limit: 512 * 1024 * 1024, // 512 MB
            cpu_limit: 1.0,
            time_limit: 300, // 5 minutes
            network: false,
            mounts: Vec::new(),
            env: Vec::new(),
            workdir: "/workspace".to_string(),
            user: None,
            security_opts: vec!["no-new-privileges".to_string()],
            keep_container: false,
            name_prefix: "clawdius-".to_string(),
        }
    }
}

impl ContainerConfig {
    /// Create a new configuration with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base image
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.base_image = image.into();
        self
    }

    /// Set memory limit
    #[must_use]
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_limit = bytes;
        self
    }

    /// Set CPU limit
    #[must_use]
    pub fn with_cpu_limit(mut self, cpus: f64) -> Self {
        self.cpu_limit = cpus;
        self
    }

    /// Enable network access
    #[must_use]
    pub fn with_network(mut self, enabled: bool) -> Self {
        self.network = enabled;
        self
    }

    /// Add a mount point
    #[must_use]
    pub fn with_mount(mut self, mount: ContainerMount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Set the user
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }
}

/// Container mount point
#[derive(Debug, Clone)]
pub struct ContainerMount {
    /// Source path on host
    pub source: String,
    /// Destination path in container
    pub destination: String,
    /// Mount as read-only
    pub read_only: bool,
}

impl ContainerMount {
    /// Create a new mount point
    pub fn new(source: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            read_only: false,
        }
    }

    /// Make the mount read-only
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

/// Container sandbox backend
pub struct ContainerBackend {
    config: ContainerConfig,
    container_counter: Arc<RwLock<u64>>,
}

impl ContainerBackend {
    /// Create a new container backend
    #[must_use]
    pub fn new(config: ContainerConfig) -> Self {
        Self {
            config,
            container_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ContainerConfig::default())
    }

    /// Check if containers are available
    #[must_use]
    pub fn is_available() -> bool {
        ContainerRuntime::detect().is_some()
    }

    /// Generate a unique container name
    async fn generate_container_name(&self) -> String {
        let mut counter = self.container_counter.write().await;
        *counter += 1;
        format!("{}{}", self.config.name_prefix, *counter)
    }

    /// Build the docker/podman run command
    #[allow(clippy::vec_init_then_push)]
    fn build_run_command(
        &self,
        name: &str,
        command: &str,
        args: &[&str],
        cwd: &Path,
    ) -> Vec<String> {
        let mut cmd = Vec::new();

        // Base command
        cmd.push("run".to_string());
        cmd.push("--rm".to_string()); // Remove container after execution
        cmd.push("--name".to_string());
        cmd.push(name.to_string());

        // Resource limits
        if self.config.memory_limit > 0 {
            cmd.push("--memory".to_string());
            cmd.push(format!("{}b", self.config.memory_limit));
        }

        if self.config.cpu_limit > 0.0 {
            cmd.push("--cpus".to_string());
            cmd.push(format!("{}", self.config.cpu_limit));
        }

        if self.config.time_limit > 0 {
            cmd.push("--timeout".to_string());
            cmd.push(self.config.time_limit.to_string());
        }

        // Network
        if !self.config.network {
            cmd.push("--network".to_string());
            cmd.push("none".to_string());
        }

        // Security options
        for opt in &self.config.security_opts {
            cmd.push("--security-opt".to_string());
            cmd.push(opt.clone());
        }

        // User
        if let Some(ref user) = self.config.user {
            cmd.push("--user".to_string());
            cmd.push(user.clone());
        }

        // Working directory
        cmd.push("--workdir".to_string());
        cmd.push(self.config.workdir.clone());

        // Mounts
        // Always mount the current working directory
        let cwd_str = cwd.to_string_lossy();
        cmd.push("-v".to_string());
        cmd.push(format!("{}:{}", cwd_str, self.config.workdir));

        // Additional mounts
        for mount in &self.config.mounts {
            cmd.push("-v".to_string());
            let mode = if mount.read_only { ":ro" } else { "" };
            cmd.push(format!("{}:{}{}", mount.source, mount.destination, mode));
        }

        // Environment variables
        for (key, value) in &self.config.env {
            cmd.push("-e".to_string());
            cmd.push(format!("{key}={value}"));
        }

        // Image
        cmd.push(self.config.base_image.clone());

        // Command and args
        cmd.push(command.to_string());
        for arg in args {
            cmd.push(arg.to_string());
        }

        cmd
    }

    /// Execute a command in a container
    pub async fn execute_async(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let name = self.generate_container_name().await;
        let cmd_args = self.build_run_command(&name, command, args, cwd);

        let output = tokio::process::Command::new(self.config.runtime.command())
            .args(&cmd_args)
            .current_dir(cwd)
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to execute container: {e}")))?;

        Ok(output)
    }

    /// Pull the base image if not present
    pub async fn pull_image(&self) -> Result<()> {
        let output = tokio::process::Command::new(self.config.runtime.command())
            .args(["pull", &self.config.base_image])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to pull image: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Sandbox(format!("Failed to pull image: {stderr}")));
        }

        Ok(())
    }

    /// List running containers
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let output = tokio::process::Command::new(self.config.runtime.command())
            .args(["ps", "--format", "{{.ID}}\t{{.Names}}\t{{.Status}}"])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to list containers: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 3 {
                    Some(ContainerInfo {
                        id: parts[0].to_string(),
                        name: parts[1].to_string(),
                        status: parts[2].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(containers)
    }

    /// Stop a container
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let output = tokio::process::Command::new(self.config.runtime.command())
            .args(["stop", container_id])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to stop container: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Sandbox(format!(
                "Failed to stop container: {stderr}"
            )));
        }

        Ok(())
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let output = tokio::process::Command::new(self.config.runtime.command())
            .args(["rm", "-f", container_id])
            .output()
            .await
            .map_err(|e| Error::Sandbox(format!("Failed to remove container: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Sandbox(format!(
                "Failed to remove container: {stderr}"
            )));
        }

        Ok(())
    }

    /// Clean up all containers with the name prefix
    pub async fn cleanup(&self) -> Result<()> {
        let containers = self.list_containers().await?;

        for container in containers {
            if container.name.starts_with(&self.config.name_prefix) {
                self.stop_container(&container.id).await.ok();
                self.remove_container(&container.id).await.ok();
            }
        }

        Ok(())
    }
}

/// Container information
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container ID
    pub id: String,
    /// Container name
    pub name: String,
    /// Container status
    pub status: String,
}

impl SandboxBackend for ContainerBackend {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output> {
        let name = format!(
            "{}{}",
            self.config.name_prefix,
            chrono::Utc::now().timestamp()
        );
        let cmd_args = self.build_run_command(&name, command, args, cwd);

        let output = std::process::Command::new(self.config.runtime.command())
            .args(&cmd_args)
            .current_dir(cwd)
            .output()
            .map_err(|e| Error::Sandbox(format!("Failed to execute container: {e}")))?;

        Ok(output)
    }

    fn name(&self) -> &'static str {
        match self.config.runtime {
            ContainerRuntime::Docker => "docker",
            ContainerRuntime::Podman => "podman",
        }
    }
}

/// Container isolation manager
pub struct ContainerIsolation {
    backend: ContainerBackend,
    sessions: Arc<RwLock<Vec<IsolationSession>>>,
}

/// Isolation session
#[derive(Debug, Clone)]
pub struct IsolationSession {
    /// Session ID
    pub id: String,
    /// Container ID (if running)
    pub container_id: Option<String>,
    /// Working directory
    pub workdir: String,
    /// Created at
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Is active
    pub active: bool,
}

impl ContainerIsolation {
    /// Create a new container isolation manager
    #[must_use]
    pub fn new(config: ContainerConfig) -> Self {
        Self {
            backend: ContainerBackend::new(config),
            sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with defaults
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ContainerConfig::default())
    }

    /// Create a new isolation session
    pub async fn create_session(&self, workdir: &Path) -> Result<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = IsolationSession {
            id: session_id.clone(),
            container_id: None,
            workdir: workdir.to_string_lossy().to_string(),
            created_at: chrono::Utc::now(),
            active: true,
        };

        self.sessions.write().await.push(session);
        Ok(session_id)
    }

    /// Execute a command in a session
    pub async fn execute_in_session(
        &self,
        _session_id: &str,
        command: &str,
        args: &[&str],
        cwd: &Path,
    ) -> Result<Output> {
        self.backend.execute_async(command, args, cwd).await
    }

    /// End a session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.active = false;
            if let Some(ref container_id) = session.container_id {
                self.backend.stop_container(container_id).await.ok();
                self.backend.remove_container(container_id).await.ok();
            }
        }
        Ok(())
    }

    /// Get active sessions
    pub async fn get_active_sessions(&self) -> Vec<IsolationSession> {
        let sessions = self.sessions.read().await;
        sessions.iter().filter(|s| s.active).cloned().collect()
    }

    /// Cleanup all sessions
    pub async fn cleanup_all(&self) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        for session in sessions.iter_mut() {
            if session.active {
                session.active = false;
                if let Some(ref container_id) = session.container_id {
                    self.backend.stop_container(container_id).await.ok();
                    self.backend.remove_container(container_id).await.ok();
                }
            }
        }
        sessions.clear();
        self.backend.cleanup().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_config_defaults() {
        let config = ContainerConfig::new();
        assert_eq!(config.base_image, "alpine:latest");
        assert!(!config.network);
    }

    #[test]
    fn test_container_mount() {
        let mount = ContainerMount::new("/host/path", "/container/path").read_only();
        assert!(mount.read_only);
    }

    #[test]
    fn test_runtime_detect() {
        // Just ensure it doesn't panic
        let _ = ContainerRuntime::detect();
    }
}
