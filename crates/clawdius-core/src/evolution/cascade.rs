//! Cascade evaluation: compose evaluators from cheap to expensive with
//! short-circuit — the mechanism AlphaEvolve uses to keep the evaluation
//! budget bounded.
//!
//! Stage 1: fast sanity/syntax filter. Stage 2: the real evaluator (test
//! suite, benchmark). Stage 3: LLM-judge for subjective dimensions. A
//! program that fails an early stage never reaches the later ones, so the
//! expensive stages only see programs worth evaluating.

use super::evaluator::{Evaluated, Evaluator};
use super::program::Program;

/// A cascade: ordered evaluators, each with a minimum-score gate. A
/// program stops at the first stage whose score falls below the gate; the
/// returned metrics come from the deepest stage reached.
pub struct CascadeEvaluator {
    stages: Vec<(Box<dyn Evaluator>, f64)>,
}

impl CascadeEvaluator {
    /// Build a cascade from (evaluator, minimum-score-gate) pairs. A gate
    /// of NEG_INFINITY means the stage never short-circuits.
    #[must_use]
    pub fn new(stages: Vec<(Box<dyn Evaluator>, f64)>) -> Self {
        Self { stages }
    }

    /// Run the cascade. Returns the final metrics (from the deepest stage
    /// reached) and the number of stages that ran.
    #[must_use]
    pub fn evaluate(&self, program: &Program) -> (Evaluated, usize) {
        let mut stages_run = 0usize;
        let mut last = Evaluated::single("score", f64::NEG_INFINITY);
        for (evaluator, gate) in &self.stages {
            let evaluated = evaluator.evaluate(program);
            stages_run += 1;
            let score = evaluated
                .metrics
                .get("score")
                .copied()
                .unwrap_or(f64::NEG_INFINITY);
            if score < *gate {
                return (evaluated, stages_run); // short-circuit: below gate
            }
            last = evaluated;
        }
        (last, stages_run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::evaluator::FnEvaluator;
    use crate::evolution::program::Program;
    use uuid::Uuid;

    fn stage(score: f64) -> Box<dyn Evaluator> {
        Box::new(FnEvaluator::new(move |_p: &Program| {
            Evaluated::single("score", score)
        }))
    }

    #[test]
    fn cascade_stops_at_first_failing_stage() {
        let cascade = CascadeEvaluator::new(vec![
            (stage(0.1), 0.5),
        ]);
        let _ = cascade;
        let cascade2 = CascadeEvaluator::new(vec![
            (stage(0.1), 0.5),
            (stage(0.9), 0.8),
        ]);
        let p = Program::new("code", Uuid::new_v4());
        let (ev, ran) = cascade2.evaluate(&p);
        assert_eq!(ran, 1, "stage 1 below gate 0.5 -> short-circuit");
        assert_eq!(ev.metrics.get("score"), Some(&0.1));
    }

    #[test]
    fn cascade_runs_all_stages_when_passing() {
        let cascade = CascadeEvaluator::new(vec![
            (Box::new(stage(0.9)), 0.5),
            (stage(0.9), 0.8),
        ]);
        let p = Program::new("code", Uuid::new_v4());
        let (ev, ran) = cascade.evaluate(&p);
        assert_eq!(ran, 2);
        assert!(ev.metrics.get("score").copied().unwrap_or(0.0) >= 0.8);
    }
}
