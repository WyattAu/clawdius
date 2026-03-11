//! Lean 4 proof verification module
//!
//! Provides integration with Lean 4 for formal verification of critical algorithms.

mod templates;
mod types;
mod verifier;

pub use templates::{
    correctness_proof_template, safety_proof_template, termination_proof_template,
};
pub use types::{LeanError, ProofDefinition, ProofTemplate, VerificationResult};
pub use verifier::LeanVerifier;
