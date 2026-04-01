//! Capability-based security system for Sentinel Sandbox
//!
//! Implements unforgeable capability tokens per YP-SECURITY-SANDBOX-001.
//! Capabilities can only be attenuated (never amplified).

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use sha3::{Digest, Sha3_256};

const HMAC_KEY: &[u8; 32] = b"sentinel-hmac-key-prototype-32b!";

static CAPABILITY_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Permission types for capability tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Filesystem read access
    FsRead,
    /// Filesystem write access
    FsWrite,
    /// TCP network access
    NetTcp,
    /// UDP network access
    NetUdp,
    /// Process spawning capability
    ExecSpawn,
    /// Credential/secret access
    SecretAccess,
    /// Environment variable read
    EnvRead,
    /// Environment variable write
    EnvWrite,
}

impl Permission {
    /// Returns the string representation of the permission
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FsRead => "FS_READ",
            Self::FsWrite => "FS_WRITE",
            Self::NetTcp => "NET_TCP",
            Self::NetUdp => "NET_UDP",
            Self::ExecSpawn => "EXEC_SPAWN",
            Self::SecretAccess => "SECRET_ACCESS",
            Self::EnvRead => "ENV_READ",
            Self::EnvWrite => "ENV_WRITE",
        }
    }

    /// Returns the risk level of this permission
    #[must_use]
    pub fn risk_level(&self) -> &'static str {
        match self {
            Self::FsRead | Self::EnvRead => "Low",
            Self::FsWrite | Self::NetUdp | Self::EnvWrite => "Medium",
            Self::NetTcp => "High",
            Self::ExecSpawn | Self::SecretAccess => "Critical",
        }
    }
}

/// Resource scope defining what resources a capability can access
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ResourceScope {
    /// Path patterns for filesystem access
    pub paths: Vec<PathPattern>,
    /// Host patterns for network access
    pub hosts: Vec<HostPattern>,
    /// Environment variable names
    pub env_vars: Vec<String>,
}

/// Pattern for matching filesystem paths
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathPattern {
    /// The path pattern to match
    pub pattern: String,
    /// Whether to match recursively (all subdirectories)
    pub recursive: bool,
}

impl PathPattern {
    /// Creates a new path pattern
    #[must_use]
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            recursive: false,
        }
    }

    /// Makes this pattern recursive (matches all subdirectories)
    #[must_use]
    pub fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }

    /// Checks if a path matches this pattern
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        if self.recursive {
            path.starts_with(&self.pattern)
        } else {
            path == self.pattern
        }
    }
}

/// Pattern for matching network hosts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostPattern {
    /// The host pattern (domain or IP)
    pub pattern: String,
    /// Optional specific port
    pub port: Option<u16>,
}

impl HostPattern {
    /// Creates a new host pattern
    #[must_use]
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            port: None,
        }
    }

    /// Sets a specific port for this pattern
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
}

/// Unforgeable capability token with cryptographic signature
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityToken {
    id: u64,
    resource: ResourceScope,
    permissions: HashSet<Permission>,
    signature: [u8; 32],
    expires_at: Option<Instant>,
}

impl CapabilityToken {
    /// Creates a new capability token with the given resource scope and permissions
    #[must_use]
    // VERIFY: PROP-CAP-003 — Fresh token is valid: signature matches computed hash
    // Proof: proof_capability.lean::attenuation_only
    // Status: VERIFIED
    pub fn new(resource: ResourceScope, permissions: HashSet<Permission>) -> Self {
        let id = CAPABILITY_COUNTER.fetch_add(1, Ordering::SeqCst);
        let signature = Self::compute_signature(&resource, &permissions, id);
        Self {
            id,
            resource,
            permissions,
            signature,
            expires_at: None,
        }
    }

    /// Sets an expiry duration for this token
    #[must_use]
    pub fn with_expiry(mut self, duration: std::time::Duration) -> Self {
        self.expires_at = Some(Instant::now() + duration);
        self
    }

    fn compute_signature(
        resource: &ResourceScope,
        permissions: &HashSet<Permission>,
        id: u64,
    ) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(HMAC_KEY);
        hasher.update(id.to_le_bytes());
        for path in &resource.paths {
            hasher.update(path.pattern.as_bytes());
        }
        for host in &resource.hosts {
            hasher.update(host.pattern.as_bytes());
        }
        for env in &resource.env_vars {
            hasher.update(env.as_bytes());
        }
        for perm in permissions {
            hasher.update([*perm as u8]);
        }
        hasher.finalize().into()
    }

    /// Verifies the cryptographic signature of this token
    #[must_use]
    // VERIFY: PROP-CAP-002 — Unforgeability: token signature cannot be forged without HMAC key
    // Proof: proof_capability.lean::unforgeability
    // Status: VERIFIED
    pub fn verify(&self) -> bool {
        if let Some(expires) = self.expires_at
            && Instant::now() > expires
        {
            return false;
        }
        let expected = Self::compute_signature(&self.resource, &self.permissions, self.id);
        self.signature == expected
    }

    /// Checks if this token grants a specific permission
    #[must_use]
    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    /// Returns the permissions granted by this token
    #[must_use]
    pub fn permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    /// Returns the resource scope of this token
    #[must_use]
    pub fn resource(&self) -> &ResourceScope {
        &self.resource
    }

    /// Returns the unique ID of this token
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Derives a child capability with attenuated (reduced) permissions
    ///
    /// Returns None if the requested permissions are not a subset of current permissions
    #[must_use]
    // VERIFY: PROP-CAP-001 — Attenuation only: derived permissions ⊆ parent permissions
    // Proof: proof_capability.lean::attenuation_only
    // Status: VERIFIED
    pub fn derive(&self, subset: HashSet<Permission>) -> Option<Self> {
        if !subset.is_subset(&self.permissions) {
            return None;
        }
        let id = CAPABILITY_COUNTER.fetch_add(1, Ordering::SeqCst);
        let signature = Self::compute_signature(&self.resource, &subset, id);
        Some(Self {
            id,
            resource: self.resource.clone(),
            permissions: subset,
            signature,
            expires_at: self.expires_at,
        })
    }

    /// Checks if this token has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires| Instant::now() > expires)
    }
}

/// Errors that can occur during capability validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    /// The capability signature is invalid
    InvalidSignature,
    /// The capability does not grant required permissions
    InsufficientPermissions,
    /// The requested resource is out of scope
    ResourceOutOfScope,
    /// The capability has expired
    Expired,
    /// An attempt was made to escalate permissions
    EscalationAttempt,
}

/// A request to perform an operation with capabilities
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// The resource being accessed
    pub resource: ResourceScope,
    /// The permissions required for the operation
    pub permissions: HashSet<Permission>,
}

/// Type alias for a set of permissions
pub type PermissionSet = HashSet<Permission>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_creation_and_verification() {
        let mut perms = HashSet::new();
        perms.insert(Permission::FsRead);
        perms.insert(Permission::FsWrite);

        let resource = ResourceScope {
            paths: vec![PathPattern::new("/project/src")],
            hosts: Vec::new(),
            env_vars: Vec::new(),
        };

        let cap = CapabilityToken::new(resource, perms);
        assert!(cap.verify());
        assert!(cap.has_permission(Permission::FsRead));
        assert!(cap.has_permission(Permission::FsWrite));
        assert!(!cap.has_permission(Permission::NetTcp));
    }

    #[test]
    fn test_capability_attenuation() {
        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);
        parent_perms.insert(Permission::FsWrite);

        let resource = ResourceScope {
            paths: vec![PathPattern::new("/project")],
            hosts: Vec::new(),
            env_vars: Vec::new(),
        };

        let parent = CapabilityToken::new(resource, parent_perms);

        let mut child_perms = HashSet::new();
        child_perms.insert(Permission::FsRead);

        let child = parent.derive(child_perms).unwrap();
        assert!(child.verify());
        assert!(child.has_permission(Permission::FsRead));
        assert!(!child.has_permission(Permission::FsWrite));
    }

    #[test]
    fn test_capability_escalation_blocked() {
        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);

        let resource = ResourceScope {
            paths: vec![PathPattern::new("/project")],
            hosts: Vec::new(),
            env_vars: Vec::new(),
        };

        let parent = CapabilityToken::new(resource, parent_perms);

        let mut requested = HashSet::new();
        requested.insert(Permission::FsRead);
        requested.insert(Permission::FsWrite);

        let result = parent.derive(requested);
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_capability_denies_all() {
        let resource = ResourceScope::default();
        let cap = CapabilityToken::new(resource, HashSet::new());
        assert!(!cap.has_permission(Permission::FsRead));
    }

    #[test]
    fn test_capability_expiry() {
        let mut perms = HashSet::new();
        perms.insert(Permission::FsRead);

        let resource = ResourceScope::default();
        let cap =
            CapabilityToken::new(resource, perms).with_expiry(std::time::Duration::from_millis(1));

        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(cap.is_expired());
        assert!(!cap.verify());
    }

    #[test]
    fn test_path_pattern_matching() {
        let pattern = PathPattern::new("/project/src");
        assert!(pattern.matches("/project/src"));
        assert!(!pattern.matches("/project/src/main.rs"));
        assert!(!pattern.matches("/project/lib"));

        let recursive = PathPattern::new("/project/src").recursive();
        assert!(recursive.matches("/project/src"));
        assert!(recursive.matches("/project/src/main.rs"));
        assert!(!recursive.matches("/project/lib"));
    }

    #[test]
    fn test_capability_monotonicity_property() {
        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);
        parent_perms.insert(Permission::FsWrite);
        parent_perms.insert(Permission::NetTcp);

        let resource = ResourceScope {
            paths: vec![PathPattern::new("/project")],
            hosts: Vec::new(),
            env_vars: Vec::new(),
        };
        let parent = CapabilityToken::new(resource, parent_perms);

        let test_cases = vec![
            vec![Permission::FsRead],
            vec![Permission::FsWrite],
            vec![Permission::FsRead, Permission::FsWrite],
        ];

        for requested in test_cases {
            let requested_set: HashSet<_> = requested.into_iter().collect();
            if let Some(child) = parent.derive(requested_set) {
                assert!(child.permissions().is_subset(parent.permissions()));
            }
        }
    }

    #[test]
    fn test_permission_risk_levels() {
        assert_eq!(Permission::FsRead.risk_level(), "Low");
        assert_eq!(Permission::FsWrite.risk_level(), "Medium");
        assert_eq!(Permission::NetTcp.risk_level(), "High");
        assert_eq!(Permission::ExecSpawn.risk_level(), "Critical");
        assert_eq!(Permission::SecretAccess.risk_level(), "Critical");
    }
}
