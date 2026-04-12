//! Sandboxed execution environments for secure command execution.
//!
//! This module provides multiple isolation tiers for executing untrusted or
//! semi-trusted code with varying levels of security.
//!
//! # Security Tiers
//!
//! The sandbox system uses a tiered approach based on trust levels:
//!
//! - **Tier 1 (`TrustedAudited`)**: Trusted, audited code (Rust/C++)
//!   - **NO isolation** — commands run directly on the host.
//!   - Only use for fully trusted, reviewed code (e.g. project build scripts).
//!
//! - **Tier 2 (Trusted)**: Trusted code (Python/Node.js)
//!   - **Weak protection** — hardcoded command blocklist only.
//!   - Trivially bypassed via interpreter eval, flag reordering, etc.
//!   - Should only be used alongside another security boundary.
//!
//! - **Tier 3 (Untrusted)**: Untrusted code (LLM reasoning)
//!   - **Real isolation** — OS-level sandbox (bubblewrap, sandbox-exec),
//!     container, or VM-based backend (gVisor, Firecracker).
//!   - For AI-generated code.
//!
//! - **Tier 4 (Hardened)**: Maximum isolation (unknown code)
//!   - **Strongest isolation** — same backends as Tier 3, with stricter
//!     defaults (no network, read-only filesystem).
//!   - For completely untrusted code.
//!
//! # Usage
//!
//! ```ignore
//! use clawdius_core::sandbox::{SandboxTier, executor::SandboxExecutor};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Choose appropriate tier based on code trust level
//! let tier = SandboxTier::Untrusted;
//!
//! // Execute command in sandbox
//! let executor = SandboxExecutor::new(tier)?;
//! let result = executor.execute("ls", &["-la"])?;
//!
//! println!("Output: {}", result.stdout);
//! println!("Exit code: {}", result.exit_code);
//! # Ok(())
//! # }
//! ```
//!
//! # Backend Support
//!
//! The module supports multiple sandboxing backends, ordered by isolation strength:
//!
//! | Backend       | Isolation           | Platform  | Notes                              |
//! |---------------|---------------------|-----------|-------------------------------------|
//! | **Firecracker** | VM (KVM)          | Linux     | Strongest; requires KVM             |
//! | **gVisor**      | Userspace kernel  | Any       | Intercepts all syscalls             |
//! | **Bubblewrap**  | Linux namespaces  | Linux     | Lightweight, uses kernel features   |
//! | **sandbox-exec**| Seatbelt profile  | macOS     | macOS native sandbox                |
//! | **Container**   | Process (Docker)  | Any       | Shared kernel                       |
//! | **Filtered**    | Blocklist only    | Any       | **No real isolation**; easily bypassed |
//! | **Direct**      | None              | Any       | **Zero isolation**; trusted code only  |
//!
//! The `direct` backend is never auto-selected by [`detect_best_backend`];
//! it must be explicitly chosen via [`SandboxTier::TrustedAudited`].
//!
//! # Configuration
//!
//! Sandbox behavior is configured in [`ShellSandboxConfig`]:
//!
//! ```rust
//! use clawdius_core::config::ShellSandboxConfig;
//!
//! let config = ShellSandboxConfig {
//!     blocked_commands: vec!["rm -rf /".to_string()],
//!     timeout_secs: 120,
//!     max_output_bytes: 1_048_576,
//!     restrict_to_cwd: true,
//! };
//! ```
//!
//! # Security Guarantees
//!
//! Guarantees depend entirely on the selected backend:
//!
//! | Guarantee             | Direct | Filtered | Container/NS | gVisor | Firecracker |
//! |-----------------------|--------|----------|--------------|--------|-------------|
//! | Command filtering     | No     | Weak*    | N/A          | N/A    | N/A         |
//! | Filesystem isolation  | No     | No       | Yes          | Yes    | Yes         |
//! | Network access control| No     | No       | Yes          | Yes    | Yes         |
//! | Resource limits       | No     | No       | Partial      | Yes    | Yes         |
//! | Kernel-level isolation| No     | No       | No           | Yes    | Yes         |
//!
//! \* The filter blocklist is trivially bypassed and must not be relied upon.
//!
//! [`ShellSandboxConfig`]: crate::config::ShellSandboxConfig

pub mod backends;
pub mod executor;
pub mod tiers;

use serde::{Deserialize, Serialize};

/// Sandbox tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxTier {
    /// Tier 1: Trusted audited code (Rust/C++)
    TrustedAudited,
    /// Tier 2: Trusted code (Python/Node.js)
    Trusted,
    /// Tier 3: Untrusted code (LLM reasoning)
    Untrusted,
    /// Tier 4: Hardened isolation (unknown code)
    Hardened,
}
