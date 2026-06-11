//! Sandbox backend implementations

use crate::error::Result;
use std::path::Path;
use std::process::Output;

#[cfg(target_os = "linux")]
mod bubblewrap;
#[cfg(target_os = "linux")]
pub use bubblewrap::BubblewrapBackend;

#[cfg(target_os = "macos")]
mod sandbox_exec;
#[cfg(target_os = "macos")]
pub use sandbox_exec::SandboxExecBackend;

mod direct;
pub use direct::DirectBackend;

mod filtered;
pub use filtered::FilteredBackend;

mod container;
pub use container::{
    ContainerBackend, ContainerConfig, ContainerInfo, ContainerIsolation, ContainerMount,
    ContainerRuntime, IsolationSession,
};

mod gvisor;
pub use gvisor::GvisorBackend;

mod firecracker;
pub use firecracker::FirecrackerBackend;

pub trait SandboxBackend: Send + Sync {
    fn execute(&self, command: &str, args: &[&str], cwd: &Path) -> Result<Output>;

    fn name(&self) -> &'static str;
}

#[must_use]
pub fn is_bwrap_available() -> bool {
    std::process::Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[must_use]
pub fn is_sandbox_exec_available() -> bool {
    std::process::Command::new("sandbox-exec")
        .arg("-h")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}

/// Check if container runtime (Docker/Podman) is available
#[must_use]
pub fn is_container_available() -> bool {
    ContainerBackend::is_available()
}

/// Check if gVisor runtime (runsc) is available.
#[must_use]
pub fn is_gvisor_available() -> bool {
    GvisorBackend::is_available()
}

/// Check if Firecracker is available.
#[must_use]
pub fn is_firecracker_available() -> bool {
    FirecrackerBackend::is_available()
}

/// Detect the best available sandbox backend.
///
/// **Security tiers** (strongest to weakest):
///
/// 1. **Hardware-isolated**: Firecracker microVM -- own kernel, memory, network.
/// 2. **User-space kernel**: gVisor (runsc) -- Sentry intercepts syscalls.
/// 3. **Namespace-level**: Bubblewrap (Linux) / sandbox-exec (macOS) -- OS-level
///    namespaces or Seatbelt profiles.
/// 4. **Container**: Docker/Podman -- shared host kernel, process-level isolation.
/// 5. **Filtered**: Command blocklist only -- **no real isolation**, trivially bypassed.
///
/// The `direct` backend (zero isolation) is **never** returned by this function.
/// It must be selected explicitly via [`SandboxExecutor::new`] with
/// [`SandboxTier::TrustedAudited`].
#[must_use]
pub fn detect_best_backend() -> &'static str {
    if is_firecracker_available() {
        return "firecracker";
    }

    if is_gvisor_available() {
        return "gvisor";
    }

    #[cfg(target_os = "linux")]
    {
        if is_bwrap_available() {
            return "bubblewrap";
        }
    }

    #[cfg(target_os = "macos")]
    {
        if is_sandbox_exec_available() {
            return "sandbox-exec";
        }
    }

    if is_container_available() {
        return "container";
    }

    "filtered"
}

/// List all available backends.
///
/// Each entry is `(name, available)`. Backends are ordered from strongest
/// isolation to weakest. See [`detect_best_backend`] for security tier details.
#[must_use]
pub fn list_available_backends() -> Vec<(&'static str, bool)> {
    vec![
        ("firecracker", is_firecracker_available()),
        ("gvisor", is_gvisor_available()),
        ("container", is_container_available()),
        #[cfg(target_os = "linux")]
        ("bubblewrap", is_bwrap_available()),
        #[cfg(target_os = "macos")]
        ("sandbox-exec", is_sandbox_exec_available()),
        ("filtered", true),
        ("direct", true),
    ]
}
