//! Mutator contract: turns a parent program + inspirations into a child
//! candidate. Phase A ships the [`StubMutator`] (deterministic, seeded —
//! the convergence test's "LLM"). Phase B adapts it over `llm/providers`
//! via the weighted-ensemble + fallback-rotation pattern the orchestrator
//! already uses.

use anyhow::Result;

use super::program::Program;
use super::rng::Rng;

/// Produces a child code candidate from a parent and inspiration programs.
pub trait Mutator: Send + Sync {
    fn mutate(&self, parent: &Program, inspirations: &[Program], rng: &mut Rng) -> Result<String>;
}

/// Deterministic stub mutator: derives the child by appending a seeded
/// mutation marker to the parent. Used by the convergence test — the
/// "LLM" is a pure function of (parent, seed), so convergence is a
/// provable property of the selection loop rather than a model gamble.
pub struct StubMutator;

impl Mutator for StubMutator {
    fn mutate(&self, parent: &Program, inspirations: &[Program], rng: &mut Rng) -> Result<String> {
        let salt = rng.next_u64();
        let inspiration_hint = inspirations
            .first()
            .map(|p| p.code.len().to_string())
            .unwrap_or_else(|| "none".into());
        Ok(format!(
            "{}\n// evolved[gen+1, salt={salt:#x}, insp={inspiration_hint}]",
            parent.code
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn stub_mutator_is_deterministic_per_seed() {
        let parent = Program::new("code", Uuid::from_u64_pair(1, 1));
        let mut r1 = Rng::new(5);
        let mut r2 = Rng::new(5);
        let a = StubMutator.mutate(&parent, &[], &mut r1).unwrap();
        let b = StubMutator.mutate(&parent, &[], &mut r2).unwrap();
        assert_eq!(a, b, "same seed must reproduce the same child");
        assert!(a.starts_with("code"));
        assert!(a.contains("evolved"));
    }
}
