//! Sentinel Sandbox Prototype
//!
//! JIT sandboxing with tiered isolation per YP-SECURITY-SANDBOX-001.
//!
//! Sandbox Tiers:
//! - Tier 1: Native Passthrough (trusted Rust/C++/Vulkan)
//! - Tier 2: OS Container (bubblewrap/podman for Node.js/Python)
//! - Tier 3: WASM Sandbox (wasmtime for LLM reasoning)
//! - Tier 4: Hardened Container (gVisor/Kata for untrusted code)

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

pub const HMAC_KEY: &[u8; 32] = b"sentinel-hmac-key-prototype-32b!";
pub const MAX_MOUNT_POINTS: usize = 10;
pub const MAX_WASM_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4GB

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SandboxTier {
    Tier1 = 1,
    Tier2 = 2,
    Tier3 = 3,
    Tier4 = 4,
}

impl SandboxTier {
    pub fn isolation_type(&self) -> &'static str {
        match self {
            Self::Tier1 => "Native Passthrough",
            Self::Tier2 => "OS Container",
            Self::Tier3 => "WASM Sandbox",
            Self::Tier4 => "Hardened Container",
        }
    }

    pub fn isolation_tech(&self) -> &'static str {
        match self {
            Self::Tier1 => "none",
            Self::Tier2 => "bubblewrap",
            Self::Tier3 => "wasmtime",
            Self::Tier4 => "gvisor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toolchain {
    Rust,
    Cpp,
    Vulkan,
    NodeJs,
    Python,
    Ruby,
    LlmReasoning,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrustLevel {
    TrustedAudited,
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    FsRead,
    FsWrite,
    NetTcp,
    NetUdp,
    ExecSpawn,
    SecretAccess,
    EnvRead,
    EnvWrite,
}

impl Permission {
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

    pub fn risk_level(&self) -> &'static str {
        match self {
            Self::FsRead | Self::EnvRead => "Low",
            Self::FsWrite | Self::NetUdp | Self::EnvWrite => "Medium",
            Self::NetTcp => "High",
            Self::ExecSpawn | Self::SecretAccess => "Critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    resource: String,
    permissions: HashSet<Permission>,
    signature: [u8; 32],
    expiry: u64,
    id: u64,
}

static CAPABILITY_COUNTER: AtomicU64 = AtomicU64::new(1);

impl Capability {
    pub fn new(resource: String, permissions: HashSet<Permission>) -> Self {
        let id = CAPABILITY_COUNTER.fetch_add(1, Ordering::SeqCst);
        let signature = Self::compute_signature(&resource, &permissions);
        Self {
            resource,
            permissions,
            signature,
            expiry: u64::MAX,
            id,
        }
    }

    fn compute_signature(resource: &str, permissions: &HashSet<Permission>) -> [u8; 32] {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        resource.hash(&mut hasher);
        for perm in permissions {
            perm.hash(&mut hasher);
        }
        HMAC_KEY.hash(&mut hasher);

        let hash = hasher.finish();
        let mut sig = [0u8; 32];
        sig[..8].copy_from_slice(&hash.to_le_bytes());
        sig[8..16].copy_from_slice(&hash.to_le_bytes());
        sig[16..24].copy_from_slice(&hash.to_le_bytes());
        sig[24..].copy_from_slice(&hash.to_le_bytes());
        sig
    }

    pub fn verify(&self) -> bool {
        let expected = Self::compute_signature(&self.resource, &self.permissions);
        self.signature == expected
    }

    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    pub fn derive(&self, subset: HashSet<Permission>) -> Option<Capability> {
        if !subset.is_subset(&self.permissions) {
            return None;
        }

        Some(Capability {
            resource: self.resource.clone(),
            permissions: subset,
            signature: Self::compute_signature(&self.resource, &subset),
            expiry: self.expiry,
            id: CAPABILITY_COUNTER.fetch_add(1, Ordering::SeqCst),
        })
    }

    pub fn permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    pub fn resource(&self) -> &str {
        &self.resource
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    pub resource: String,
    pub permissions: HashSet<Permission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    InvalidSignature,
    InsufficientPermissions,
    ResourceOutOfScope,
    Expired,
    EscalationAttempt,
}

pub struct SentinelSandbox {
    tier: SandboxTier,
    capabilities: Vec<Capability>,
}

impl SentinelSandbox {
    pub fn new(tier: SandboxTier, capabilities: Vec<Capability>) -> Self {
        Self { tier, capabilities }
    }

    pub fn tier(&self) -> SandboxTier {
        self.tier
    }

    pub fn validate_request(&self, request: &CapabilityRequest) -> Result<(), CapabilityError> {
        for cap in &self.capabilities {
            if !cap.verify() {
                return Err(CapabilityError::InvalidSignature);
            }

            if cap.resource() != request.resource {
                continue;
            }

            if !request.permissions.is_subset(cap.permissions()) {
                return Err(CapabilityError::InsufficientPermissions);
            }

            return Ok(());
        }

        Err(CapabilityError::ResourceOutOfScope)
    }

    pub fn derive_capability(
        &self,
        parent: &Capability,
        subset: HashSet<Permission>,
    ) -> Result<Capability, CapabilityError> {
        if !parent.verify() {
            return Err(CapabilityError::InvalidSignature);
        }

        if !subset.is_subset(parent.permissions()) {
            return Err(CapabilityError::EscalationAttempt);
        }

        parent
            .derive(subset)
            .ok_or(CapabilityError::EscalationAttempt)
    }
}

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
        (Toolchain::Untrusted, _) => SandboxTier::Tier4,
        (_, TrustLevel::Untrusted) => SandboxTier::Tier4,
        _ => SandboxTier::Tier2,
    }
}

#[derive(Debug, Clone)]
pub struct GlobalPolicy {
    pub forbidden_keys: HashSet<String>,
    pub allowed_commands: HashSet<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    MalformedToml,
    ForbiddenKey(String),
    UnsafeCommand(String),
    UnsafeMount(String),
    MaxMountExceeded(usize),
    InvalidPath(String),
}

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
                if let Some(cmd_str) = cmd_val.as_str() {
                    if !is_safe_command(cmd_str, &policy.allowed_commands) {
                        return Err(SettingsError::UnsafeCommand(format!(
                            "{}: {}",
                            name, cmd_str
                        )));
                    }
                }
            }
        }

        if let Some(mounts) = table.get("mounts").and_then(|m| m.as_array()) {
            if mounts.len() > policy.max_mount_points {
                return Err(SettingsError::MaxMountExceeded(mounts.len()));
            }
            for mount in mounts {
                if let Some(mount_table) = mount.as_table() {
                    if let Some(source) = mount_table.get("source").and_then(|s| s.as_str()) {
                        if !is_within_project(source) {
                            return Err(SettingsError::UnsafeMount(source.to_string()));
                        }
                    }
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

fn is_within_project(path: &str) -> bool {
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
        assert_eq!(tier.isolation_tech(), "bubblewrap");
    }

    #[test]
    fn test_tier3_llm_reasoning() {
        let tier = select_sandbox_tier(Toolchain::LlmReasoning, TrustLevel::Untrusted);
        assert_eq!(tier, SandboxTier::Tier3);
        assert_eq!(tier.isolation_type(), "WASM Sandbox");
    }

    #[test]
    fn test_capability_creation_and_verification() {
        let mut perms = HashSet::new();
        perms.insert(Permission::FsRead);
        perms.insert(Permission::FsWrite);

        let cap = Capability::new("/project/src".to_string(), perms);
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

        let parent = Capability::new("/project".to_string(), parent_perms);

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

        let parent = Capability::new("/project".to_string(), parent_perms);

        let mut requested = HashSet::new();
        requested.insert(Permission::FsRead);
        requested.insert(Permission::FsWrite);

        let result = parent.derive(requested);
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_capability_denies_all() {
        let cap = Capability::new("/project".to_string(), HashSet::new());
        assert!(!cap.has_permission(Permission::FsRead));
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

    #[test]
    fn test_path_traversal_rejection() {
        let settings = r#"
[[mounts]]
source = "../../../etc"
destination = "/etc"
"#;
        let policy = GlobalPolicy::default();
        let result = validate_settings(settings, &policy);
        assert!(matches!(result, Err(SettingsError::UnsafeMount(_))));
    }

    #[test]
    fn test_max_mount_exceeded() {
        let mut settings = String::from("[[mounts]]\nsource = \"/src\"\ndestination = \"/src\"\n");
        for i in 0..12 {
            settings.push_str(&format!(
                "[[mounts]]\nsource = \"/{}\"\ndestination = \"/{}\"\n",
                i, i
            ));
        }

        let policy = GlobalPolicy::default();
        let result = validate_settings(&settings, &policy);
        assert!(matches!(result, Err(SettingsError::MaxMountExceeded(_))));
    }

    #[test]
    fn test_capability_validation_in_sandbox() {
        let mut perms = HashSet::new();
        perms.insert(Permission::FsRead);

        let cap = Capability::new("/project/src".to_string(), perms);
        let sandbox = SentinelSandbox::new(SandboxTier::Tier3, vec![cap]);

        let request = CapabilityRequest {
            resource: "/project/src".to_string(),
            permissions: {
                let mut p = HashSet::new();
                p.insert(Permission::FsRead);
                p
            },
        };

        assert!(sandbox.validate_request(&request).is_ok());
    }

    #[test]
    fn test_capability_validation_denied() {
        let mut perms = HashSet::new();
        perms.insert(Permission::FsRead);

        let cap = Capability::new("/project/src".to_string(), perms);
        let sandbox = SentinelSandbox::new(SandboxTier::Tier3, vec![cap]);

        let request = CapabilityRequest {
            resource: "/project/src".to_string(),
            permissions: {
                let mut p = HashSet::new();
                p.insert(Permission::FsWrite);
                p
            },
        };

        let result = sandbox.validate_request(&request);
        assert!(matches!(
            result,
            Err(CapabilityError::InsufficientPermissions)
        ));
    }

    #[test]
    fn test_capability_monotonicity_property() {
        let mut parent_perms = HashSet::new();
        parent_perms.insert(Permission::FsRead);
        parent_perms.insert(Permission::FsWrite);
        parent_perms.insert(Permission::NetTcp);

        let parent = Capability::new("/project".to_string(), parent_perms);

        let test_cases = vec![
            vec![Permission::FsRead],
            vec![Permission::FsWrite],
            vec![Permission::FsRead, Permission::FsWrite],
        ];

        for requested in test_cases {
            let requested_set: HashSet<_> = requested.into_iter().collect();
            if let Ok(child) = parent.derive(requested_set.clone()) {
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
