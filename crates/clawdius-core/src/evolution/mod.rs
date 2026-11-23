//! Evolutionary code optimization (AlphaEvolve architecture, Rust).
//!
//! An LLM-driven quality-diversity loop: candidate programs are mutated,
//! evaluated, and selected through a MAP-Elites grid with island
//! populations — the architecture AlphaEvolve proved on open problems and
//! OpenEvolve standardized as the open reference.
//!
//! This module is the Rust engine, dogfooding the clawdius estate:
//! mutation routes through the LLM provider ensemble (Phase B) with the
//! orchestrator's rate-limit fallback, evaluation rides the existing
//! subprocess/test-execution patterns, islands map onto fleet worktree
//! isolation, and cost caps come from the resource governor.
//!
//! Determinism: a fixed seed replays an entire run exactly (rng,
//! database, and mutator all derive from it).
//!
//! # Layout
//!
//! - [`program`] — the typed program ledger
//! - [`database`] — MAP-Elites grid + island populations + migration
//! - [`evaluator`] — the evaluation contract + subprocess adapter
//! - [`mutator`] — the mutation contract + deterministic stub
//! - [`engine`] — the evolution loop + convergence acceptance
//! - [`rng`] — seeded SplitMix64 (zero-dependency PRNG)
//! - [`sandbox`] — per-evaluation worktree isolation (fleet worktree dogfood)
//! - [`prompt`] — artifact-conditioned mutation prompts (Phase B)
//! - [`llm_mutator`] — LLM-driven mutation with governor caps + stub fallback (Phase B)

pub mod database;
pub mod engine;
pub mod evaluator;
pub mod llm_mutator;
pub mod mutator;
pub mod program;
pub mod prompt;
pub mod rng;
pub mod sandbox;

pub use database::{DatabaseConfig, FeatureDimension, ProgramDatabase};
pub use engine::{converge, run, run_parallel, EvolutionConfig, RunStats};
pub use evaluator::{CommandEvaluator, Evaluated, Evaluator, FnEvaluator};
pub use llm_mutator::{extract_code_block, LlmMutator};
pub use mutator::{Mutator, StubMutator};
pub use program::Program;
pub use prompt::{AttemptRecord, PromptConfig, PromptSampler};
pub use rng::Rng;
pub use sandbox::SandboxedCommandEvaluator;
