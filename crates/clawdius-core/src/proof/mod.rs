//! Lean 4 proof verification module
//!
//! Provides integration with Lean 4 for formal verification of critical algorithms.

// Unwrap purge batch 4: proof module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod templates;
mod types;
mod verifier;

pub use templates::{
    correctness_proof_template, safety_proof_template, termination_proof_template,
};
pub use types::{LeanError, ProofDefinition, ProofTemplate, VerificationResult};
pub use verifier::LeanVerifier;
