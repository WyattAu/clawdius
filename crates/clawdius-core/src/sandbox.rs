//! Sandboxed execution environments for secure command execution.
//!
//! This module provides multiple isolation tiers for executing untrusted or
//! semi-trusted code with varying levels of security.
//!
//! # Security Tiers
//!
//! The sandbox system uses a tiered approach based on trust levels:
//!
//! - **Tier 1 (TrustedAudited)**: Trusted, audited code (Rust/C++)
//!   - Minimal restrictions
//!   - Direct execution
//!   - For core system components
//!
//! - **Tier 2 (Trusted)**: Trusted code (Python/Node.js)
//!   - Basic restrictions
//!   - Filtered execution
//!   - For well-known tools
//!
//! - **Tier 3 (Untrusted)**: Untrusted code (LLM reasoning)
//!   - Significant restrictions
//!   - Sandboxed execution
//!   - For AI-generated code
//!
//! - **Tier 4 (Hardened)**: Maximum isolation (unknown code)
//!   - Full isolation
//!   - Namespace/container sandbox
//!   - For completely untrusted code
//!
//! # Usage
//!
//! ```rust,no_run
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
//! The module supports multiple sandboxing backends:
//!
//! - **Direct**: No isolation (Tier 1 only)
//! - **Filtered**: Command filtering and restrictions (Tier 2)
//! - **Sandbox-exec**: macOS sandbox (Tier 3)
//! - **Bubblewrap**: Linux namespace isolation (Tier 3-4)
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
//! - Command injection protection
//! - Path traversal prevention
//! - Resource limits (CPU, memory, time)
//! - Network access control
//! - Filesystem isolation
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
