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
