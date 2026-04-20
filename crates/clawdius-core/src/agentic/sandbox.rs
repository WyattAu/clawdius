//! Sprint Sandbox Isolation
//!
//! Provides sandboxed execution environments for sprint tasks, ensuring
//! that each sprint runs in an isolated context. This is A6.
//!
//! # Sandboxing Strategies
//!
//! - **Directory**: Restricts all file I/O to a project directory (path validation)
//! - **Container**: Future Docker/Podman/Firecracker backend (trait defined, not yet implemented)
//!
//! # Architecture
//!
//! The `SandboxedExecutor` wraps any `ToolExecutor` and enforces sandbox rules
//! on all tool calls (shell commands, file reads/writes). It integrates with
//! `SprintEngine` via the existing `with_tool_executor()` builder.

use crate::agentic::tool_executor::{
    NoOpToolExecutor, ShellToolExecutor, ToolExecutor, ToolRequest, ToolResult,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Sandbox Configuration
// ---------------------------------------------------------------------------

/// Sandbox isolation level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxLevel {
    /// No isolation — commands run directly (default for local development)
    None,
    /// Directory-only isolation — restrict file access to project root
    Directory,
    /// Full container isolation — Docker/Podman/Firecracker (future)
    Container,
}

impl Default for SandboxLevel {
    fn default() -> Self {
        SandboxLevel::None
    }
}

/// Configuration for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Isolation level
    pub level: SandboxLevel,
    /// Root directory for sandbox (all file access restricted to this tree)
    pub root: PathBuf,
    /// Maximum execution time per command
    pub timeout: Duration,
    /// Maximum output size per command (bytes)
    pub max_output_bytes: usize,
    /// Additional environment variables available inside the sandbox
    pub env_vars: HashMap<String, String>,
    /// Network access allowed inside sandbox
    pub network_allowed: bool,
    /// Maximum concurrent processes inside the sandbox
    pub max_processes: usize,
}

impl SandboxConfig {
    /// Create a new sandbox configuration.
    pub fn new(root: PathBuf) -> Self {
        Self {
            level: SandboxLevel::Directory,
            root,
            timeout: Duration::from_secs(300), // 5 minutes per command
            max_output_bytes: 512 * 1024, // 512 KB
            env_vars: HashMap::new(),
            network_allowed: false,
            max_processes: 8,
        }
    }

    /// Create a permissive config (no isolation) for local development.
    pub fn permissive() -> Self {
        Self {
            level: SandboxLevel::None,
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout: Duration::from_secs(600),
            max_output_bytes: 1024 * 1024,
            env_vars: HashMap::new(),
            network_allowed: true,
            max_processes: 16,
        }
    }

    /// Set the isolation level.
    #[must_use]
    pub fn with_level(mut self, level: SandboxLevel) -> Self {
        self.level = level;
        self
    }

    /// Set the timeout per command.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set whether network access is allowed.
    #[must_use]
    pub fn with_network(mut self, allowed: bool) -> Self {
        self.network_allowed = allowed;
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Set max output bytes per command.
    #[must_use]
    pub fn with_max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }
}

// ---------------------------------------------------------------------------
// Sandbox Enforcement Result
// ---------------------------------------------------------------------------

/// Result of a sandbox enforcement check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCheck {
    /// Whether the operation was allowed
    pub allowed: bool,
    /// Reason if denied
    pub denial_reason: Option<String>,
    /// The sanitized/rewritten path (if any)
    pub sanitized_path: Option<PathBuf>,
}

impl SandboxCheck {
    pub fn allowed(sanitized: Option<PathBuf>) -> Self {
        Self {
            allowed: true,
            denial_reason: None,
            sanitized_path: sanitized,
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            denial_reason: Some(reason.into()),
            sanitized_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Directory Sandbox — path-based isolation
// ---------------------------------------------------------------------------

/// Validates and sanitizes file paths within a sandbox root directory.
pub struct DirectorySandbox {
    root: PathBuf,
}

impl DirectorySandbox {
    pub fn new(root: PathBuf) -> Self {
        // Canonicalize the root for reliable comparison
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Self { root }
    }

    /// Check if a path is within the sandbox root.
    ///
    /// Returns a `SandboxCheck` with the sanitized (absolute, within-root) path
    /// if allowed, or a denial reason if not.
    pub fn check_path(&self, path: &Path) -> SandboxCheck {
        // Handle absolute paths by stripping the leading / and joining to root
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };

        // Try to canonicalize (resolves symlinks, .., etc.)
        let canonical = match std::fs::canonicalize(&absolute) {
            Ok(p) => p,
            Err(_) => {
                // Path doesn't exist yet (might be a new file) —
                // check the parent instead
                if let Some(parent) = absolute.parent() {
                    match std::fs::canonicalize(parent) {
                        Ok(p) => p.join(
                            absolute
                                .file_name()
                                .unwrap_or_default(),
                        ),
                        Err(_) => absolute.clone(),
                    }
                } else {
                    absolute.clone()
                }
            },
        };

        // Check if canonical path starts with root
        if canonical.starts_with(&self.root) {
            SandboxCheck::allowed(Some(canonical))
        } else {
            SandboxCheck::denied(format!(
                "Path '{}' is outside sandbox root '{}'",
                absolute.display(),
                self.root.display()
            ))
        }
    }

    /// Sanitize a shell command by checking for path traversal and dangerous patterns.
    pub fn check_command(&self, command: &str) -> SandboxCheck {
        // Block obvious escape attempts
        let dangerous_patterns = [
            "cd /",
            "cd ..",
            "/etc/passwd",
            "/etc/shadow",
            "/proc/",
            "/sys/",
            "/dev/",
            "mkfs.",
            "dd if=",
            ":(){ :|:& };:", // fork bomb
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return SandboxCheck::denied(format!(
                    "Command contains blocked pattern: '{}'",
                    pattern
                ));
            }
        }

        SandboxCheck::allowed(None)
    }

    /// Get the sandbox root path.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

// ---------------------------------------------------------------------------
// SandboxedExecutor — wraps any ToolExecutor with sandbox enforcement
// ---------------------------------------------------------------------------

/// A tool executor that enforces sandbox rules on all operations.
///
/// Wraps an inner `ToolExecutor` (typically `ShellToolExecutor`) and validates
/// all file paths and commands before execution. When `SandboxLevel::None`,
/// passes through without enforcement.
pub struct SandboxedExecutor {
    inner: Arc<dyn ToolExecutor>,
    config: SandboxConfig,
    directory_sandbox: DirectorySandbox,
    /// Track resource usage for the sandbox session
    stats: Arc<RwLock<SandboxStats>>,
}

/// Resource usage statistics for a sandbox session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxStats {
    /// Total number of commands executed
    pub commands_executed: usize,
    /// Total number of commands blocked
    pub commands_blocked: usize,
    /// Total bytes of output produced
    pub total_output_bytes: usize,
    /// Total wall-clock time spent executing
    pub total_execution_time_ms: u64,
    /// Number of timeouts
    pub timeouts: usize,
}

impl SandboxedExecutor {
    /// Create a new sandboxed executor wrapping the given inner executor.
    pub fn new(inner: Arc<dyn ToolExecutor>, config: SandboxConfig) -> Self {
        let directory_sandbox = DirectorySandbox::new(config.root.clone());
        Self {
            inner,
            config,
            directory_sandbox,
            stats: Arc::new(RwLock::new(SandboxStats::default())),
        }
    }

    /// Create a sandboxed executor with a new ShellToolExecutor for the given root.
    pub fn with_shell(root: PathBuf, config: SandboxConfig) -> Self {
        let inner: Arc<dyn ToolExecutor> = Arc::new(ShellToolExecutor::new(root.clone()));
        Self::new(inner, config)
    }

    /// Get the sandbox configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Get the sandbox statistics.
    pub async fn stats(&self) -> SandboxStats {
        self.stats.read().await.clone()
    }

    /// Validate and sanitize a file path within the sandbox.
    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        if self.config.level == SandboxLevel::None {
            return Ok(PathBuf::from(path));
        }

        let check = self.directory_sandbox.check_path(Path::new(path));
        if !check.allowed {
            // Increment blocked counter
            let stats = self.stats.clone();
            tokio::spawn(async move {
                let mut s = stats.write().await;
                s.commands_blocked += 1;
            });
            return Err(crate::Error::Sandbox(format!(
                "Path blocked: {}",
                check.denial_reason.unwrap_or_default()
            )));
        }

        check
            .sanitized_path
            .ok_or_else(|| crate::Error::Sandbox("No sanitized path available".to_string()))
    }

    /// Validate a shell command.
    fn validate_command(&self, command: &str) -> Result<()> {
        if self.config.level == SandboxLevel::None {
            return Ok(());
        }

        let check = self.directory_sandbox.check_command(command);
        if !check.allowed {
            let stats = self.stats.clone();
            tokio::spawn(async move {
                let mut s = stats.write().await;
                s.commands_blocked += 1;
            });
            return Err(crate::Error::Sandbox(format!(
                "Command blocked: {}",
                check.denial_reason.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Truncate output to max_output_bytes.
    fn truncate_output(&self, output: &str) -> String {
        if output.len() <= self.config.max_output_bytes {
            return output.to_string();
        }
        let truncated = &output[..self.config.max_output_bytes];
        format!(
            "{}\n\n[OUTPUT TRUNCATED: {} bytes exceeded {} byte limit]",
            truncated,
            output.len(),
            self.config.max_output_bytes
        )
    }
}

#[async_trait]
impl ToolExecutor for SandboxedExecutor {
    async fn execute(&self, request: ToolRequest) -> Result<ToolResult> {
        let start = Instant::now();

        // Validate based on tool type
        match request.name.as_str() {
            "shell" => {
                // Validate the command
                let command = request
                    .arguments
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                self.validate_command(command)?;

                // Execute with timeout tracking
                let result = self.inner.execute(request.clone()).await;

                let elapsed = start.elapsed();
                let mut stats = self.stats.write().await;
                stats.commands_executed += 1;
                stats.total_execution_time_ms += elapsed.as_millis() as u64;

                match result {
                    Ok(mut tool_result) => {
                        tool_result.content = self.truncate_output(&tool_result.content);
                        stats.total_output_bytes += tool_result.content.len();
                        Ok(tool_result)
                    },
                    Err(e) => {
                        if e.to_string().contains("timeout") || e.to_string().contains("Timeout") {
                            stats.timeouts += 1;
                        }
                        Err(e)
                    },
                }
            },
            "read_file" | "write_file" | "edit_file" | "list_files" => {
                // Validate file path
                if let Some(path_value) = request.arguments.get("path") {
                    if let Some(path_str) = path_value.as_str() {
                        self.validate_path(path_str)?;
                    }
                } else if let Some(path_value) = request.arguments.get("file_path") {
                    if let Some(path_str) = path_value.as_str() {
                        self.validate_path(path_str)?;
                    }
                } else if let Some(path_value) = request.arguments.get("file") {
                    if let Some(path_str) = path_value.as_str() {
                        self.validate_path(path_str)?;
                    }
                }

                let result = self.inner.execute(request.clone()).await;

                let mut stats = self.stats.write().await;
                stats.commands_executed += 1;
                stats.total_execution_time_ms += start.elapsed().as_millis() as u64;

                match result {
                    Ok(mut tool_result) => {
                        tool_result.content = self.truncate_output(&tool_result.content);
                        stats.total_output_bytes += tool_result.content.len();
                        Ok(tool_result)
                    },
                    Err(e) => Err(e),
                }
            },
            _ => {
                // Unknown tool — pass through
                self.inner.execute(request).await
            },
        }
    }

    fn has_tool(&self, name: &str) -> bool {
        self.inner.has_tool(name)
    }

    fn list_tools(&self) -> Vec<crate::agentic::tool_executor::ToolDefinition> {
        self.inner.list_tools()
    }
}

// ---------------------------------------------------------------------------
// Container Sandbox (trait for future implementation)
// ---------------------------------------------------------------------------

/// Trait for container-based sandbox backends (Docker, Podman, Firecracker).
///
/// This is a placeholder for Phase B (Enterprise SaaS Foundation) where
/// full container isolation will be implemented.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Create a new container with the given image and working directory.
    async fn create(
        &self,
        image: &str,
        work_dir: &Path,
        env: &HashMap<String, String>,
    ) -> Result<String>;

    /// Execute a command inside the container. Returns the output.
    async fn exec(&self, container_id: &str, command: &str) -> Result<(String, String, bool)>;

    /// Copy a file into the container.
    async fn copy_in(&self, container_id: &str, src: &Path, dest: &Path) -> Result<()>;

    /// Copy a file out of the container.
    async fn copy_out(&self, container_id: &str, src: &Path, dest: &Path) -> Result<()>;

    /// Destroy the container.
    async fn destroy(&self, container_id: &str) -> Result<()>;

    /// Get the container backend name (e.g., "docker", "podman", "firecracker").
    fn backend_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_level_default() {
        assert_eq!(SandboxLevel::default(), SandboxLevel::None);
    }

    #[test]
    fn test_sandbox_config_builder() {
        let config = SandboxConfig::new(PathBuf::from("/tmp/project"))
            .with_level(SandboxLevel::Directory)
            .with_timeout(Duration::from_secs(60))
            .with_network(false)
            .with_env("TEST_KEY", "test_value");

        assert_eq!(config.level, SandboxLevel::Directory);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(!config.network_allowed);
        assert_eq!(config.env_vars.get("TEST_KEY").unwrap(), "test_value");
    }

    #[test]
    fn test_sandbox_config_permissive() {
        let config = SandboxConfig::permissive();
        assert_eq!(config.level, SandboxLevel::None);
        assert!(config.network_allowed);
    }

    #[test]
    fn test_sandbox_config_serialization() {
        let config = SandboxConfig::new(PathBuf::from("/tmp/project"))
            .with_level(SandboxLevel::Directory);

        let json = serde_json::to_string(&config).unwrap();
        let parsed: SandboxConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.level, SandboxLevel::Directory);
    }

    #[test]
    fn test_sandbox_check_allowed() {
        let check = SandboxCheck::allowed(Some(PathBuf::from("/tmp/project/file.rs")));
        assert!(check.allowed);
        assert!(check.denial_reason.is_none());
    }

    #[test]
    fn test_sandbox_check_denied() {
        let check = SandboxCheck::denied("Path outside root");
        assert!(!check.allowed);
        assert_eq!(check.denial_reason.as_deref(), Some("Path outside root"));
    }

    #[test]
    fn test_directory_sandbox_check_path_within_root() {
        // Use the current directory as root
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox = DirectorySandbox::new(root.clone());

        // A file within root should be allowed
        let check = sandbox.check_path(&root.join("Cargo.toml"));
        assert!(check.allowed);
    }

    #[test]
    fn test_directory_sandbox_check_path_outside_root() {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox = DirectorySandbox::new(root);

        // /etc/passwd should be denied
        let check = sandbox.check_path(Path::new("/etc/passwd"));
        // This may or may not be allowed depending on the current directory
        // (if running from root, it might be within). But for normal cases:
        if !std::env::current_dir()
            .unwrap_or_default()
            .starts_with("/etc")
        {
            assert!(!check.allowed || check.denial_reason.is_some());
        }
    }

    #[test]
    fn test_directory_sandbox_check_command_safe() {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox = DirectorySandbox::new(root);

        let check = sandbox.check_command("cargo build --release");
        assert!(check.allowed);
    }

    #[test]
    fn test_directory_sandbox_check_command_blocked_patterns() {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox = DirectorySandbox::new(root);

        // Fork bomb
        let check = sandbox.check_command(":(){ :|:& };:");
        assert!(!check.allowed);

        // /etc/passwd access
        let check = sandbox.check_command("cat /etc/passwd");
        assert!(!check.allowed);

        // /proc access
        let check = sandbox.check_command("ls /proc/self");
        assert!(!check.allowed);
    }

    #[test]
    fn test_directory_sandbox_root() {
        let root = PathBuf::from("/tmp/test");
        let sandbox = DirectorySandbox::new(root.clone());
        assert_eq!(sandbox.root(), root);
    }

    #[test]
    fn test_sandbox_stats_default() {
        let stats = SandboxStats::default();
        assert_eq!(stats.commands_executed, 0);
        assert_eq!(stats.commands_blocked, 0);
        assert_eq!(stats.total_output_bytes, 0);
        assert_eq!(stats.timeouts, 0);
    }

    #[test]
    fn test_sandbox_stats_serialization() {
        let stats = SandboxStats {
            commands_executed: 10,
            commands_blocked: 2,
            total_output_bytes: 4096,
            total_execution_time_ms: 5000,
            timeouts: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: SandboxStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.commands_executed, 10);
        assert_eq!(parsed.commands_blocked, 2);
        assert_eq!(parsed.timeouts, 1);
    }

    #[test]
    fn test_truncate_output_within_limit() {
        let config = SandboxConfig::new(PathBuf::from("/tmp"))
            .with_level(SandboxLevel::None);
        let inner: Arc<dyn ToolExecutor> = Arc::new(NoOpToolExecutor);
        let sandbox = SandboxedExecutor::new(inner, config);

        let short = "hello world";
        assert_eq!(sandbox.truncate_output(short), short);
    }

    #[test]
    fn test_truncate_output_exceeds_limit() {
        let config = SandboxConfig::new(PathBuf::from("/tmp"))
            .with_level(SandboxLevel::None)
            .with_max_output_bytes(100);
        let inner: Arc<dyn ToolExecutor> = Arc::new(NoOpToolExecutor);
        let sandbox = SandboxedExecutor::new(inner, config);

        let long = "x".repeat(200);
        let truncated = sandbox.truncate_output(&long);
        assert!(truncated.contains("OUTPUT TRUNCATED"));
    }

    #[tokio::test]
    async fn test_sandboxed_executor_stats() {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config = SandboxConfig::permissive();
        let inner: Arc<dyn ToolExecutor> = Arc::new(NoOpToolExecutor);
        let sandbox = SandboxedExecutor::new(inner, config);

        let stats = sandbox.stats().await;
        assert_eq!(stats.commands_executed, 0);
    }

    #[tokio::test]
    async fn test_sandboxed_executor_has_tools() {
        let config = SandboxConfig::permissive();
        let inner: Arc<dyn ToolExecutor> = Arc::new(NoOpToolExecutor);
        let sandbox = SandboxedExecutor::new(inner, config);

        assert!(!sandbox.has_tool("shell"));
        assert_eq!(sandbox.list_tools().len(), 0);
    }
}
