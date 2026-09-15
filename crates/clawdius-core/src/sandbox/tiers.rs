//! Sandbox tiers

use serde::{Deserialize, Serialize};

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Current tier
    pub tier: super::SandboxTier,
    /// Enable network access
    #[serde(default)]
    pub network: bool,
    /// Mount points
    #[serde(default)]
    pub mounts: Vec<MountPoint>,
    /// Explicitly allow the unisolated `filtered` backend (command blocklist
    /// only) when no real isolation backend is available.
    ///
    /// **DANGEROUS.** The blocklist is trivially bypassed (flag reordering,
    /// interpreter eval, string-embedded payloads) and must never be treated
    /// as a security boundary. When this is `false` (the default) and no
    /// isolating backend (gVisor, Firecracker, container, bubblewrap,
    /// sandbox-exec) is available, executor construction fails with
    /// [`crate::Error::SandboxUnavailable`] instead of degrading silently.
    #[serde(default)]
    pub allow_unisolated: bool,
}

/// Mount point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    /// Source path
    pub source: String,
    /// Destination path in sandbox
    pub destination: String,
    /// Read-only
    #[serde(default)]
    pub read_only: bool,
}
