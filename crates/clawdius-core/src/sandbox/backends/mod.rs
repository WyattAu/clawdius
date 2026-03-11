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
pub use gvisor::{ContainerInfo as GVisorContainerInfo, GVisorBackend, GVisorConfig, MountSpec};

mod firecracker;
pub use firecracker::{FirecrackerBackend, FirecrackerConfig, JailerConfig};

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

/// Check if gVisor (runsc) is available
#[must_use]
pub fn is_gvisor_available() -> bool {
    GVisorBackend::is_available()
}

/// Check if Firecracker is available
#[must_use]
pub fn is_firecracker_available() -> bool {
    FirecrackerBackend::is_available()
}

/// Check if KVM is available (required for Firecracker)
#[must_use]
pub fn is_kvm_available() -> bool {
    FirecrackerBackend::is_kvm_available()
}

/// Detect the best available sandbox backend
#[must_use]
pub fn detect_best_backend() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        // Prefer gVisor for strong isolation
        if is_gvisor_available() {
            return "gvisor";
        }

        if is_bwrap_available() {
            return "bubblewrap";
        }

        // Firecracker for VM-level isolation
        if is_firecracker_available() && is_kvm_available() {
            return "firecracker";
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

    "direct"
}

/// List all available backends
#[must_use]
pub fn list_available_backends() -> Vec<(&'static str, bool)> {
    vec![
        ("direct", true), // Always available
        ("container", is_container_available()),
        ("gvisor", is_gvisor_available()),
        ("firecracker", is_firecracker_available()),
        #[cfg(target_os = "linux")]
        ("bubblewrap", is_bwrap_available()),
        #[cfg(target_os = "macos")]
        ("sandbox-exec", is_sandbox_exec_available()),
    ]
}
