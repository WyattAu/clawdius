//! Sandbox tier selection and configuration
//!
//! Implements tiered isolation per YP-SECURITY-SANDBOX-001.
//! - Tier 1: Native Passthrough (trusted Rust/C++/Vulkan)
//! - Tier 2: OS Container (bubblewrap/podman for Node.js/Python)
//! - Tier 3: WASM Sandbox (wasmtime for LLM reasoning)
//! - Tier 4: Hardened Container (gVisor/Kata for untrusted code)

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use crate::capability::{CapabilityToken, Permission, PermissionSet, ResourceScope};
use crate::error::SandboxError;

/// Maximum number of mount points allowed in a sandbox
pub const MAX_MOUNT_POINTS: usize = 10;
/// Maximum WASM memory in bytes (4GB)
pub const MAX_WASM_MEMORY: u64 = 4 * 1024 * 1024 * 1024;
/// Default sandbox timeout (5 minutes)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Sandbox isolation tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SandboxTier {
    /// Native passthrough - no isolation (trusted code only)
    Tier1 = 1,
    /// OS container - bubblewrap/sandbox-exec isolation
    Tier2 = 2,
    /// WASM sandbox - wasmtime isolation for LLM reasoning
    Tier3 = 3,
    /// Hardened container - gVisor/Kata for untrusted code
    Tier4 = 4,
}

impl SandboxTier {
    /// Returns a human-readable name for this tier
    #[must_use]
    pub fn isolation_type(&self) -> &'static str {
        match self {
            Self::Tier1 => "Native Passthrough",
            Self::Tier2 => "OS Container",
            Self::Tier3 => "WASM Sandbox",
            Self::Tier4 => "Hardened Container",
        }
    }

    /// Returns the isolation technology used for this tier
    #[must_use]
    pub fn isolation_tech(&self) -> &'static str {
        match self {
            Self::Tier1 => "none",
            Self::Tier2 => {
                #[cfg(target_os = "linux")]
                {
                    "bubblewrap"
                }
                #[cfg(target_os = "macos")]
                {
                    "sandbox-exec"
                }
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                {
                    "unknown"
                }
            }
            Self::Tier3 => "wasmtime",
            Self::Tier4 => {
                #[cfg(target_os = "linux")]
                {
                    "gvisor"
                }
                #[cfg(not(target_os = "linux"))]
                {
                    "unavailable"
                }
            }
        }
    }
}

/// Toolchain type for sandbox selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toolchain {
    /// Rust compiler
    Rust,
    /// C++ compiler
    Cpp,
    /// Vulkan graphics
    Vulkan,
    /// Node.js runtime
    NodeJs,
    /// Python interpreter
    Python,
    /// Ruby interpreter
    Ruby,
    /// LLM reasoning (Brain)
    LlmReasoning,
    /// Untrusted/unknown code
    Untrusted,
}

/// Trust level for code execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrustLevel {
    /// Code has been audited and is fully trusted
    TrustedAudited,
    /// Code is trusted but not audited
    Trusted,
    /// Code is untrusted
    Untrusted,
}

/// Selects the appropriate sandbox tier based on toolchain and trust level
#[must_use]
pub fn select_sandbox_tier(toolchain: Toolchain, trust_level: TrustLevel) -> SandboxTier {
    match (toolchain, trust_level) {
        (Toolchain::Rust | Toolchain::Cpp | Toolchain::Vulkan, TrustLevel::TrustedAudited) => {
            SandboxTier::Tier1
        }
        (Toolchain::NodeJs | Toolchain::Python | Toolchain::Ruby, TrustLevel::Trusted) => {
            SandboxTier::Tier2
        }
        (Toolchain::NodeJs | Toolchain::Python | Toolchain::Ruby, TrustLevel::Untrusted) => {
            SandboxTier::Tier4
        }
        (Toolchain::LlmReasoning, _) => SandboxTier::Tier3,
        (Toolchain::Untrusted, _) | (_, TrustLevel::Untrusted) => SandboxTier::Tier4,
        _ => SandboxTier::Tier2,
    }
}

/// Mount point configuration for sandbox
#[derive(Debug, Clone)]
pub struct MountPoint {
    /// Source path on host
    pub source: PathBuf,
    /// Destination path inside sandbox
    pub destination: PathBuf,
    /// Whether the mount is read-only
    pub read_only: bool,
}

impl MountPoint {
    /// Creates a new mount point
    #[must_use]
    pub fn new(source: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            read_only: false,
        }
    }

    /// Makes this mount point read-only
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

/// Configuration for spawning a sandbox
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Working directory for sandbox execution
    pub working_directory: PathBuf,
    /// Mount points to create inside sandbox
    pub mount_points: Vec<MountPoint>,
    /// Capability permissions granted to sandbox
    pub capabilities: PermissionSet,
    /// Execution timeout
    pub timeout: Duration,
    /// Memory limit in bytes
    pub memory_limit: Option<u64>,
    /// Environment variables to pass to sandbox
    pub environment: HashMap<String, String>,
}

impl SandboxConfig {
    /// Creates a new sandbox configuration with the given working directory
    #[must_use]
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
            mount_points: Vec::new(),
            capabilities: PermissionSet::new(),
            timeout: DEFAULT_TIMEOUT,
            memory_limit: None,
            environment: HashMap::new(),
        }
    }

    /// Adds a mount point to the configuration
    #[must_use]
    pub fn with_mount(mut self, mount: MountPoint) -> Self {
        self.mount_points.push(mount);
        self
    }

    /// Adds a capability permission to the configuration
    #[must_use]
    pub fn with_capability(mut self, perm: Permission) -> Self {
        self.capabilities.insert(perm);
        self
    }

    /// Sets the execution timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the memory limit
    #[must_use]
    pub fn with_memory_limit(mut self, limit: u64) -> Self {
        self.memory_limit = Some(limit);
        self
    }

    /// Adds an environment variable
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Validates the configuration
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid
    pub fn validate(&self) -> Result<(), SandboxError> {
        if self.mount_points.len() > MAX_MOUNT_POINTS {
            return Err(SandboxError::CreationFailed {
                reason: format!(
                    "Too many mount points: {} > {}",
                    self.mount_points.len(),
                    MAX_MOUNT_POINTS
                ),
            });
        }

        for mount in &self.mount_points {
            if !is_within_project(&mount.source) {
                return Err(SandboxError::CreationFailed {
                    reason: format!("Mount outside project: {}", mount.source.display()),
                });
            }
        }

        Ok(())
    }
}

fn is_within_project(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    !path_str.contains("..") && !path_str.starts_with("/etc") && !path_str.starts_with("/root")
}

/// Platform-specific sandbox backend trait
pub trait PlatformSandbox: Send + Sync + std::fmt::Debug {
    /// Spawns a sandbox with the given configuration
    ///
    /// # Errors
    /// Returns an error if sandbox creation fails
    fn spawn(&self, config: &SandboxConfig) -> Result<SandboxHandle, SandboxError>;
    /// Executes a command in the sandbox
    ///
    /// # Errors
    /// Returns an error if execution fails
    fn execute(
        &self,
        handle: &SandboxHandle,
        command: &CommandSpec,
    ) -> Result<ExitStatus, SandboxError>;
    /// Kills the sandbox process
    ///
    /// # Errors
    /// Returns an error if kill fails
    fn kill(&self, handle: &SandboxHandle) -> Result<(), SandboxError>;
}

/// Command specification for sandbox execution
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// Program to execute
    pub program: String,
    /// Command-line arguments
    pub args: Vec<String>,
    /// Working directory override
    pub working_dir: Option<PathBuf>,
}

impl CommandSpec {
    /// Creates a new command specification
    #[must_use]
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_dir: None,
        }
    }

    /// Adds an argument to the command
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Sets the working directory for the command
    #[must_use]
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }
}

/// Exit status from sandbox execution
#[derive(Debug, Clone)]
pub struct ExitStatus {
    /// Exit code
    pub code: i32,
    /// Whether execution succeeded (code == 0)
    pub success: bool,
}

impl ExitStatus {
    /// Creates a new exit status with the given code
    #[must_use]
    pub fn new(code: i32) -> Self {
        Self {
            code,
            success: code == 0,
        }
    }

    /// Creates a successful exit status
    #[must_use]
    pub fn success() -> Self {
        Self {
            code: 0,
            success: true,
        }
    }
}

/// Handle to a running sandbox
#[derive(Debug)]
pub struct SandboxHandle {
    /// Unique sandbox ID
    pub id: u64,
    /// Sandbox tier
    pub tier: SandboxTier,
    /// Capabilities granted to this sandbox
    pub capabilities: Vec<CapabilityToken>,
}

impl SandboxHandle {
    /// Creates a new sandbox handle
    #[must_use]
    pub fn new(id: u64, tier: SandboxTier, capabilities: Vec<CapabilityToken>) -> Self {
        Self {
            id,
            tier,
            capabilities,
        }
    }
}

/// Linux bubblewrap sandbox backend
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct BubblewrapSandbox;

#[cfg(target_os = "linux")]
impl BubblewrapSandbox {
    /// Creates a new bubblewrap sandbox backend
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "linux")]
impl PlatformSandbox for BubblewrapSandbox {
    fn spawn(&self, config: &SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        config.validate()?;

        let capabilities = vec![CapabilityToken::new(
            ResourceScope::default(),
            config.capabilities.clone(),
        )];

        Ok(SandboxHandle::new(1, SandboxTier::Tier2, capabilities))
    }

    fn execute(
        &self,
        _handle: &SandboxHandle,
        command: &CommandSpec,
    ) -> Result<ExitStatus, SandboxError> {
        let mut cmd = std::process::Command::new("bwrap");
        cmd.arg("--unshare-all")
            .arg("--die-with-parent")
            .arg("--ro-bind")
            .arg("/usr")
            .arg("/usr")
            .arg("--proc")
            .arg("/proc")
            .arg("--dev")
            .arg("/dev")
            .arg("--")
            .arg(&command.program);

        for arg in &command.args {
            cmd.arg(arg);
        }

        let status = cmd.status().map_err(|e| SandboxError::CreationFailed {
            reason: e.to_string(),
        })?;

        Ok(ExitStatus::new(status.code().unwrap_or(1)))
    }

    fn kill(&self, _handle: &SandboxHandle) -> Result<(), SandboxError> {
        Ok(())
    }
}

/// macOS sandbox-exec backend
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct SandboxExecSandbox;

#[cfg(target_os = "macos")]
impl SandboxExecSandbox {
    /// Creates a new sandbox-exec backend
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
impl PlatformSandbox for SandboxExecSandbox {
    fn spawn(&self, config: &SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        config.validate()?;

        let capabilities = vec![CapabilityToken::new(
            ResourceScope::default(),
            config.capabilities.clone(),
        )];

        Ok(SandboxHandle::new(1, SandboxTier::Tier2, capabilities))
    }

    fn execute(
        &self,
        _handle: &SandboxHandle,
        command: &CommandSpec,
    ) -> Result<ExitStatus, SandboxError> {
        let mut cmd = std::process::Command::new("sandbox-exec");
        cmd.arg("-f")
            .arg("/dev/stdin")
            .arg("--")
            .arg(&command.program);

        for arg in &command.args {
            cmd.arg(arg);
        }

        let status = cmd.status().map_err(|e| SandboxError::CreationFailed {
            reason: e.to_string(),
        })?;

        Ok(ExitStatus::new(status.code().unwrap_or(1)))
    }

    fn kill(&self, _handle: &SandboxHandle) -> Result<(), SandboxError> {
        Ok(())
    }
}

/// Native (no isolation) sandbox backend for trusted code
#[derive(Debug)]
pub struct NativeSandbox;

impl NativeSandbox {
    /// Creates a new native sandbox backend
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PlatformSandbox for NativeSandbox {
    fn spawn(&self, config: &SandboxConfig) -> Result<SandboxHandle, SandboxError> {
        config.validate()?;

        let capabilities = vec![CapabilityToken::new(
            ResourceScope::default(),
            config.capabilities.clone(),
        )];

        Ok(SandboxHandle::new(1, SandboxTier::Tier1, capabilities))
    }

    fn execute(
        &self,
        _handle: &SandboxHandle,
        command: &CommandSpec,
    ) -> Result<ExitStatus, SandboxError> {
        let mut cmd = std::process::Command::new(&command.program);
        cmd.args(&command.args);

        if let Some(dir) = &command.working_dir {
            cmd.current_dir(dir);
        }

        let status = cmd.status().map_err(|e| SandboxError::CreationFailed {
            reason: e.to_string(),
        })?;

        Ok(ExitStatus::new(status.code().unwrap_or(1)))
    }

    fn kill(&self, _handle: &SandboxHandle) -> Result<(), SandboxError> {
        Ok(())
    }
}

impl Default for NativeSandbox {
    fn default() -> Self {
        Self::new()
    }
}

/// Global policy for settings validation
#[derive(Debug, Clone)]
pub struct GlobalPolicy {
    /// Forbidden environment variable key patterns
    pub forbidden_keys: HashSet<String>,
    /// Allowed command base names
    pub allowed_commands: HashSet<String>,
    /// Maximum number of mount points
    pub max_mount_points: usize,
}

impl Default for GlobalPolicy {
    fn default() -> Self {
        let mut forbidden = HashSet::new();
        forbidden.insert("exec".to_string());
        forbidden.insert("shell".to_string());
        forbidden.insert("system".to_string());

        Self {
            forbidden_keys: forbidden,
            allowed_commands: HashSet::new(),
            max_mount_points: MAX_MOUNT_POINTS,
        }
    }
}

/// Errors that can occur during settings validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// The TOML is malformed
    MalformedToml,
    /// A forbidden key was found
    ForbiddenKey(String),
    /// An unsafe command was detected
    UnsafeCommand(String),
    /// A mount path is outside the project
    UnsafeMount(String),
    /// Too many mount points
    MaxMountExceeded(usize),
    /// An invalid path was specified
    InvalidPath(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedToml => write!(f, "Malformed TOML"),
            Self::ForbiddenKey(key) => write!(f, "Forbidden key: {key}"),
            Self::UnsafeCommand(cmd) => write!(f, "Unsafe command: {cmd}"),
            Self::UnsafeMount(path) => write!(f, "Unsafe mount: {path}"),
            Self::MaxMountExceeded(count) => write!(f, "Too many mounts: {count}"),
            Self::InvalidPath(path) => write!(f, "Invalid path: {path}"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Validates a settings TOML string against the global policy
///
/// # Errors
/// Returns an error if validation fails
pub fn validate_settings(settings: &str, policy: &GlobalPolicy) -> Result<(), SettingsError> {
    let parsed: toml::Value = toml::from_str(settings).map_err(|_| SettingsError::MalformedToml)?;

    if let Some(table) = parsed.as_table() {
        for key in table.keys() {
            if policy.forbidden_keys.contains(key) {
                return Err(SettingsError::ForbiddenKey(key.clone()));
            }
        }

        if let Some(commands) = table.get("commands").and_then(|c| c.as_table()) {
            for (name, cmd_val) in commands {
                if let Some(cmd_str) = cmd_val.as_str()
                    && !is_safe_command(cmd_str, &policy.allowed_commands)
                {
                    return Err(SettingsError::UnsafeCommand(format!(
                        "{name}: {cmd_str}"
                    )));
                }
            }
        }

        if let Some(mounts) = table.get("mounts").and_then(|m| m.as_array()) {
            if mounts.len() > policy.max_mount_points {
                return Err(SettingsError::MaxMountExceeded(mounts.len()));
            }
            for mount in mounts {
                if let Some(mount_table) = mount.as_table()
                    && let Some(source) = mount_table.get("source").and_then(|s| s.as_str())
                    && !is_safe_path(source)
                {
                    return Err(SettingsError::UnsafeMount(source.to_string()));
                }
            }
        }
    }

    Ok(())
}

fn is_safe_command(cmd: &str, allowed: &HashSet<String>) -> bool {
    let dangerous_chars = ['&', ';', '|', '$', '`', '>', '<', '\n', '\r'];

    for ch in dangerous_chars {
        if cmd.contains(ch) {
            return false;
        }
    }

    let base_cmd = cmd.split_whitespace().next().unwrap_or("");
    allowed.is_empty() || allowed.contains(base_cmd)
}

fn is_safe_path(path: &str) -> bool {
    !path.contains("..") && !path.starts_with("/etc") && !path.starts_with("/root")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier1_trusted_rust() {
        let tier = select_sandbox_tier(Toolchain::Rust, TrustLevel::TrustedAudited);
        assert_eq!(tier, SandboxTier::Tier1);
        assert_eq!(tier.isolation_type(), "Native Passthrough");
    }

    #[test]
    fn test_tier2_python_trusted() {
        let tier = select_sandbox_tier(Toolchain::Python, TrustLevel::Trusted);
        assert_eq!(tier, SandboxTier::Tier2);
    }

    #[test]
    fn test_tier3_llm_reasoning() {
        let tier = select_sandbox_tier(Toolchain::LlmReasoning, TrustLevel::Untrusted);
        assert_eq!(tier, SandboxTier::Tier3);
        assert_eq!(tier.isolation_type(), "WASM Sandbox");
    }

    #[test]
    fn test_tier4_untrusted() {
        let tier = select_sandbox_tier(Toolchain::Untrusted, TrustLevel::Untrusted);
        assert_eq!(tier, SandboxTier::Tier4);
    }

    #[test]
    fn test_sandbox_config_validation() {
        let config = SandboxConfig::new("/project")
            .with_mount(MountPoint::new("/project/src", "/src"))
            .with_capability(Permission::FsRead);

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_too_many_mounts() {
        let mut config = SandboxConfig::new("/project");

        for i in 0..15 {
            config = config.with_mount(MountPoint::new(format!("/src{i}"), format!("/mnt{i}")));
        }

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_unsafe_mount_rejected() {
        let config = SandboxConfig::new("/project")
            .with_mount(MountPoint::new("/etc/passwd", "/etc/passwd"));

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_native_sandbox_spawn() {
        let sandbox = NativeSandbox::new();
        let config = SandboxConfig::new("/project");

        let handle = sandbox.spawn(&config);
        assert!(handle.is_ok());
    }

    #[test]
    fn test_command_spec_builder() {
        let cmd = CommandSpec::new("cargo")
            .arg("build")
            .arg("--release")
            .working_dir("/project");

        assert_eq!(cmd.program, "cargo");
        assert_eq!(cmd.args, vec!["build", "--release"]);
        assert_eq!(cmd.working_dir, Some(PathBuf::from("/project")));
    }

    #[test]
    fn test_mount_point_read_only() {
        let mount = MountPoint::new("/host/src", "/src").read_only();
        assert!(mount.read_only);
    }

    #[test]
    fn test_trust_level_selection() {
        assert_eq!(
            select_sandbox_tier(Toolchain::Rust, TrustLevel::Trusted),
            SandboxTier::Tier2
        );
        assert_eq!(
            select_sandbox_tier(Toolchain::Rust, TrustLevel::Untrusted),
            SandboxTier::Tier4
        );
    }

    #[test]
    fn test_valid_settings() {
        let settings = r#"
[project]
name = "test"
version = "1.0.0"

[commands]
build = "cargo build"
"#;
        let policy = GlobalPolicy::default();
        let result = validate_settings(settings, &policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_forbidden_key_rejection() {
        let settings = r#"
[exec]
command = "rm -rf /"
"#;
        let policy = GlobalPolicy::default();
        let result = validate_settings(settings, &policy);
        assert!(matches!(result, Err(SettingsError::ForbiddenKey(_))));
    }

    #[test]
    fn test_shell_injection_rejection() {
        let settings = r#"
[commands]
build = "cargo build && rm -rf /"
"#;
        let mut policy = GlobalPolicy::default();
        policy.allowed_commands.insert("cargo".to_string());

        let result = validate_settings(settings, &policy);
        assert!(matches!(result, Err(SettingsError::UnsafeCommand(_))));
    }
}
