//! Program ledger for the evolution engine: one `Program` per candidate
//! solution, with its code, metrics, lineage, and generation.
//!
//! A Program is the unit the evolution loop mutates, evaluates, and
//! stores. `code` is the artifact under evolution (source text, config,
//! a kernel body); `metrics` are the evaluator's scores (the primary
//! fitness plus any auxiliary dimensions the MAP-Elites grid keys on);
//! `lineage` records the parent chain so champion provenance is
//! auditable; `metadata` carries evaluator artifacts (errors, logs)
//! that feed artifact-conditioned prompts.
//!
//! Typed rather than dynamic-python-dict: metrics aggregation is
//! explicit, serialization is serde JSON (no pickle), and the ledger is
//! reproducible under a fixed seed.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One candidate solution in the evolution database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// Stable id (uuid v7-style time-ordered when generated via
    /// [`Program::new`]; deterministic ids come from the seeder).
    pub id: Uuid,
    /// The artifact under evolution.
    pub code: String,
    /// Evaluator scores. `fitness_metric()` selects the primary.
    pub metrics: std::collections::BTreeMap<String, f64>,
    /// Parent chain, oldest first. Empty for the seed population.
    pub lineage: Vec<Uuid>,
    /// Generation counter at insertion time.
    pub generation: usize,
    /// Evaluator artifacts (stderr, logs, partial output) for
    /// artifact-conditioned prompts. Bounded by the config.
    pub artifacts: Option<String>,
    /// Monotonic insertion order within the database.
    pub inserted_at: u64,
}

impl Program {
    /// Create a seed program (generation 0, no lineage).
    #[must_use]
    pub fn new(code: impl Into<String>, id: Uuid) -> Self {
        Self {
            id,
            code: code.into(),
            metrics: std::collections::BTreeMap::new(),
            lineage: Vec::new(),
            generation: 0,
            artifacts: None,
            inserted_at: 0,
        }
    }

    /// A child of `parent` at `generation`, inheriting its lineage.
    #[must_use]
    pub fn child_of(
        parent: &Program,
        code: impl Into<String>,
        id: Uuid,
        generation: usize,
    ) -> Self {
        let mut lineage = parent.lineage.clone();
        lineage.push(parent.id);
        Self {
            id,
            code: code.into(),
            // Inherit the parent's scores until re-evaluated (standard
            // MAP-Elites convention for unevaluated mutants: cell
            // comparison stays conservative instead of panicking).
            metrics: parent.metrics.clone(),
            lineage,
            generation,
            artifacts: None,
            inserted_at: 0,
        }
    }

    /// Set metrics from (name, value) pairs.
    pub fn set_metrics(&mut self, pairs: impl IntoIterator<Item = (String, f64)>) {
        for (k, v) in pairs {
            self.metrics.insert(k, v);
        }
    }

    /// The primary fitness value. Panics only if the evaluator never set
    /// it — a programming error, not a runtime condition.
    #[must_use]
    pub fn fitness(&self, metric: &str) -> f64 {
        self.metrics
            .get(metric)
            .copied()
            .expect("primary fitness metric missing — evaluator bug")
    }

    /// Fitness as `Option` for callers that tolerate unevaluated programs.
    #[must_use]
    pub fn fitness_opt(&self, metric: &str) -> Option<f64> {
        self.metrics.get(metric).copied()
    }

    /// Direct ancestor id, if any.
    #[must_use]
    pub fn parent_id(&self) -> Option<Uuid> {
        self.lineage.last().copied()
    }

    /// Depth of the lineage chain.
    #[must_use]
    pub fn lineage_depth(&self) -> usize {
        self.lineage.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u64) -> Uuid {
        Uuid::from_u64_pair(n, n)
    }

    #[test]
    fn child_inherits_lineage_and_generation() {
        let mut parent = Program::new("parent code", pid(1));
        parent.generation = 3;
        parent.set_metrics([("score".into(), 0.5)]);

        let child = Program::child_of(&parent, "child code", pid(2), 4);
        assert_eq!(child.lineage, vec![pid(1)]);
        assert_eq!(child.generation, 4);
        assert_eq!(child.parent_id(), Some(pid(1)));
        assert_eq!(child.lineage_depth(), 1);
    }

    #[test]
    fn grandchild_chain_deepens() {
        let g0 = Program::new("a", pid(1));
        let g1 = Program::child_of(&g0, "b", pid(2), 1);
        let g2 = Program::child_of(&g1, "c", pid(3), 2);
        assert_eq!(g2.lineage, vec![pid(1), pid(2)]);
        assert_eq!(g2.lineage_depth(), 2);
    }

    #[test]
    fn metrics_are_set_and_read() {
        let mut p = Program::new("code", pid(1));
        p.set_metrics([("score".into(), 0.9), ("speed".into(), 12.0)]);
        assert_eq!(p.fitness("score"), 0.9);
        assert_eq!(p.fitness_opt("speed"), Some(12.0));
        assert_eq!(p.fitness_opt("missing"), None);
    }

    #[test]
    fn serde_roundtrip_preserves_everything() {
        let mut p = Program::new("code body", pid(7));
        p.set_metrics([("score".into(), 0.75)]);
        p.artifacts = Some("stderr: ok".into());
        p.inserted_at = 42;

        let json = serde_json::to_string(&p).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
