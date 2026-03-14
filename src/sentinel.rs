//! Sentinel Sandbox Component
//!
//! Implements COMP-SENTINEL-001 per BP-SENTINEL-001.
//! Provides JIT sandboxing with capability-based security.

use std::collections::HashSet;
use std::path::Path;

use crate::capability::{CapabilityRequest, CapabilityToken, PermissionSet};
use crate::component::{Component, ComponentId, ComponentInfo, ComponentState};
use crate::error::{Result, SandboxError};
use crate::sandbox::{
    select_sandbox_tier, validate_settings, CommandSpec, ExitStatus, GlobalPolicy, NativeSandbox,
    PlatformSandbox, SandboxHandle, SandboxSpawnConfig, SandboxTier, Toolchain, TrustLevel,
};

/// Sentinel component version
pub const SENTINEL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sentinel Sandbox Component
///
/// Provides JIT sandboxing with tiered isolation and capability-based security.
#[derive(Debug)]
pub struct Sentinel {
    state: ComponentState,
    policy: GlobalPolicy,
    #[cfg(target_os = "linux")]
    tier2_backend: crate::sandbox::BubblewrapSandbox,
    #[cfg(target_os = "macos")]
    tier2_backend: crate::sandbox::SandboxExecSandbox,
    tier1_backend: NativeSandbox,
}

impl Sentinel {
    /// Creates a new Sentinel component
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ComponentState::Uninitialized,
            policy: GlobalPolicy::default(),
            #[cfg(target_os = "linux")]
            tier2_backend: crate::sandbox::BubblewrapSandbox::new(),
            #[cfg(target_os = "macos")]
            tier2_backend: crate::sandbox::SandboxExecSandbox::new(),
            tier1_backend: NativeSandbox::new(),
        }
    }

    /// Spawns a sandbox for the given request
    ///
    /// # Errors
    /// Returns an error if sandbox creation fails
    pub fn spawn(&self, request: SpawnRequest) -> Result<SandboxHandle> {
        let tier = select_sandbox_tier(request.toolchain, request.trust_level);

        let mut config =
            SandboxConfig::new(&request.working_directory).with_timeout(request.timeout);

        for mount in request.mounts {
            config = config.with_mount(mount);
        }

        for perm in &request.required_capabilities {
            config = config.with_capability(*perm);
        }

        config.validate()?;

        match tier {
            SandboxTier::Tier1 => self.tier1_backend.spawn(&config),
            SandboxTier::Tier2 => self.tier2_backend.spawn(&config),
            SandboxTier::Tier3 | SandboxTier::Tier4 => Err(SandboxError::CreationFailed {
                reason: format!("Tier {} not yet implemented", tier as u8),
            }),
        }
        .map_err(Into::into)
    }

    /// Executes a command in the sandbox
    ///
    /// # Errors
    /// Returns an error if execution fails
    pub fn execute(&self, handle: &SandboxHandle, command: &CommandSpec) -> Result<ExitStatus> {
        match handle.tier {
            SandboxTier::Tier1 => self.tier1_backend.execute(handle, command),
            SandboxTier::Tier2 => self.tier2_backend.execute(handle, command),
            SandboxTier::Tier3 | SandboxTier::Tier4 => Err(SandboxError::CreationFailed {
                reason: format!("Tier {} not yet implemented", handle.tier as u8),
            }),
        }
        .map_err(Into::into)
    }

    /// Validates a settings file for safety
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn validate_settings(&self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|e| SandboxError::CreationFailed {
            reason: e.to_string(),
        })?;
        validate_settings(&content, &self.policy)
            .map_err(|e| SandboxError::SettingsValidation(e.to_string()))?;
        Ok(())
    }

    /// Derives an attenuated capability from a parent capability
    ///
    /// # Errors
    /// Returns an error if derivation fails (escalation attempt or invalid parent)
    pub fn derive_capability(
        &self,
        parent: &CapabilityToken,
        subset: PermissionSet,
    ) -> Result<CapabilityToken> {
        if !parent.verify() {
            return Err(SandboxError::CapabilityViolation {
                capability: "Invalid signature".into(),
            }
            .into());
        }

        parent.derive(subset).ok_or_else(|| {
            SandboxError::CapabilityViolation {
                capability: "Escalation attempt".into(),
            }
            .into()
        })
    }

    /// Validates a capability request
    ///
    /// # Errors
    /// Returns an error if the request is invalid
    pub fn validate_request(&self, request: &CapabilityRequest) -> Result<()> {
        if request.permissions.is_empty() {
            return Err(SandboxError::CapabilityViolation {
                capability: "No permissions".into(),
            }
            .into());
        }
        Ok(())
    }

    /// Returns component information
    #[must_use]
    pub fn info(&self) -> ComponentInfo {
        ComponentInfo::new(ComponentId::SENTINEL, self.name(), SENTINEL_VERSION)
    }
}

impl Default for Sentinel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Sentinel {
    fn id(&self) -> ComponentId {
        ComponentId::SENTINEL
    }

    fn name(&self) -> &'static str {
        "Sentinel"
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        self.state = ComponentState::Initialized;
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        self.state = ComponentState::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = ComponentState::Stopped;
        Ok(())
    }
}

/// Request to spawn a sandbox
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    /// Toolchain to use
    pub toolchain: Toolchain,
    /// Trust level of the code
    pub trust_level: TrustLevel,
    /// Required capability permissions
    pub required_capabilities: PermissionSet,
    /// Working directory for execution
    pub working_directory: std::path::PathBuf,
    /// Mount points to create
    pub mounts: Vec<crate::sandbox::MountPoint>,
    /// Execution timeout
    pub timeout: std::time::Duration,
}

impl Default for SpawnRequest {
    fn default() -> Self {
        Self {
            toolchain: Toolchain::Untrusted,
            trust_level: TrustLevel::Untrusted,
            required_capabilities: HashSet::new(),
            working_directory: std::path::PathBuf::from("."),
            mounts: Vec::new(),
            timeout: std::time::Duration::from_secs(300),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Permission;
    use crate::sandbox::MountPoint;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn test_sentinel_component_trait() {
        let mut sentinel = Sentinel::new();
        assert_eq!(sentinel.id(), ComponentId::SENTINEL);
        assert_eq!(sentinel.name(), "Sentinel");
        assert_eq!(sentinel.state(), ComponentState::Uninitialized);
        assert!(sentinel.initialize().is_ok());
        assert_eq!(sentinel.state(), ComponentState::Initialized);
        assert!(sentinel.start().is_ok());
        assert_eq!(sentinel.state(), ComponentState::Running);
        assert!(sentinel.stop().is_ok());
        assert_eq!(sentinel.state(), ComponentState::Stopped);
    }

    #[test]
    fn test_sentinel_spawn_tier1() {
        let sentinel = Sentinel::new();
        let request = SpawnRequest {
            toolchain: Toolchain::Rust,
            trust_level: TrustLevel::TrustedAudited,
            required_capabilities: {
                let mut p = HashSet::new();
                p.insert(Permission::FsRead);
                p
            },
            working_directory: PathBuf::from("/project"),
            mounts: vec![MountPoint::new("/project/src", "/src")],
            timeout: std::time::Duration::from_secs(60),
        };

        let result = sentinel.spawn(request);
        assert!(result.is_ok());
        let handle = result.unwrap();
        assert_eq!(handle.tier, SandboxTier::Tier1);
    }

    #[test]
    fn test_sentinel_spawn_tier2() {
        let sentinel = Sentinel::new();
        let request = SpawnRequest {
            toolchain: Toolchain::Python,
            trust_level: TrustLevel::Trusted,
            required_capabilities: {
                let mut p = HashSet::new();
                p.insert(Permission::FsRead);
                p
            },
            working_directory: PathBuf::from("/project"),
            mounts: vec![],
            timeout: std::time::Duration::from_secs(60),
        };

        let result = sentinel.spawn(request);
        assert!(result.is_ok());
        let handle = result.unwrap();
        assert_eq!(handle.tier, SandboxTier::Tier2);
    }

    #[test]
    fn test_sentinel_derive_capability() {
        let sentinel = Sentinel::new();

        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);
        parent_perms.insert(Permission::FsWrite);

        let parent =
            CapabilityToken::new(crate::capability::ResourceScope::default(), parent_perms);

        let mut child_perms = HashSet::new();
        child_perms.insert(Permission::FsRead);

        let result = sentinel.derive_capability(&parent, child_perms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sentinel_derive_capability_escalation_blocked() {
        let sentinel = Sentinel::new();

        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);

        let parent =
            CapabilityToken::new(crate::capability::ResourceScope::default(), parent_perms);

        let mut child_perms = HashSet::new();
        child_perms.insert(Permission::FsRead);
        child_perms.insert(Permission::FsWrite);

        let result = sentinel.derive_capability(&parent, child_perms);
        assert!(result.is_err());
    }

    #[test]
    fn test_sentinel_validate_request_empty() {
        let sentinel = Sentinel::new();
        let request = CapabilityRequest {
            resource: crate::capability::ResourceScope::default(),
            permissions: HashSet::new(),
        };

        let result = sentinel.validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_sentinel_info() {
        let sentinel = Sentinel::new();
        let info = sentinel.info();
        assert_eq!(info.id, ComponentId::SENTINEL);
        assert_eq!(info.name, "Sentinel");
    }

    #[test]
    fn test_spawn_request_default() {
        let request = SpawnRequest::default();
        assert_eq!(request.toolchain, Toolchain::Untrusted);
        assert_eq!(request.trust_level, TrustLevel::Untrusted);
        assert_eq!(request.timeout, std::time::Duration::from_secs(300));
    }
}
