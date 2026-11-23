//! Prompt sampler: builds the mutation prompts that drive LLM-driven
//! evolution.
//!
//! The prompt carries everything the mutator needs to make a *targeted*
//! improvement rather than a blind rewrite: the parent code, the fitness
//! trajectory, evaluator artifacts (errors/logs), and up to N inspiration
//! programs from adjacent niches. This is the mechanism AlphaEvolve
//! credits for its non-trivial discoveries — artifact-conditioned prompts
//! beat blind resampling.

use super::program::Program;

/// One prior attempt on the same lineage, summarized for the prompt.
#[derive(Debug, Clone)]
pub struct AttemptRecord {
    /// Short code excerpt (first/last lines) identifying the attempt.
    pub summary: String,
    /// The fitness it achieved.
    pub score: f64,
    /// Why it fell short (evaluator artifacts, first line).
    pub failure_note: Option<String>,
}

/// Configuration for the prompt sampler.
#[derive(Debug, Clone)]
pub struct PromptConfig {
    /// Maximum inspiration programs included in the prompt.
    pub max_inspirations: usize,
    /// Maximum parent code lines in the prompt (long programs get
    /// truncated with a marker).
    pub max_parent_lines: usize,
    /// Maximum evolution-history entries in the prompt.
    pub max_history: usize,
    /// Instruction prefix (the task description the evolution optimizes
    /// toward).
    pub instruction: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            max_inspirations: 3,
            max_parent_lines: 200,
            max_history: 5,
            instruction: String::new(),
        }
    }
}

/// Builds the mutation prompt for one iteration.
#[derive(Debug, Clone)]
pub struct PromptSampler {
    pub config: PromptConfig,
}

impl PromptSampler {
    #[must_use]
    pub fn new(config: PromptConfig) -> Self {
        Self { config }
    }

    /// Build the full mutation prompt for one iteration.
    ///
    /// `parent` is the program under mutation; `inspirations` are adjacent
    /// elites; `artifacts` is the parent's evaluator stderr/log; `history`
    /// summarizes prior attempts on this lineage (what was tried and why
    /// it fell short) so the model avoids repeating them.
    #[must_use]
    pub fn build_mutation_prompt(
        &self,
        parent: &Program,
        inspirations: &[Program],
        artifacts: Option<&str>,
        history: &[AttemptRecord],
    ) -> String {
        let mut p = String::new();

        if !self.config.instruction.is_empty() {
            p.push_str("# Task\n\n");
            p.push_str(&self.config.instruction);
            p.push_str("\n\n");
        }

        p.push_str("# Current program (the parent)\n\n");
        p.push_str(&truncate_code(&parent.code, self.config.max_parent_lines));
        p.push_str("\n\n");

        if let Some(art) = artifacts {
            if !art.trim().is_empty() {
                p.push_str("# Evaluator artifacts from the parent\n\n");
                p.push_str("```\n");
                p.push_str(&truncate_str(art, 1_500));
                p.push_str("\n```\n\n");
            }
        }

        if !inspirations.is_empty() {
            p.push_str("# Inspiration programs (adjacent elites)\n\n");
            for (i, insp) in inspirations
                .iter()
                .take(self.config.max_inspirations)
                .enumerate()
            {
                p.push_str(&format!(
                    "## Inspiration {} (score {:.4})\n\n",
                    i + 1,
                    insp.fitness_opt(&metric_name(insp)).unwrap_or(0.0)
                ));
                p.push_str(&truncate_code(&insp.code, 60));
                p.push_str("\n\n");
            }
        }

        if !history.is_empty() {
            p.push_str("# Prior attempts on this lineage (avoid repeating these)\n\n");
            for rec in history.iter().take(self.config.max_history) {
                p.push_str(&format!(
                    "- attempt (score {:.4}): {}\n",
                    rec.score, rec.summary
                ));
                if let Some(note) = &rec.failure_note {
                    p.push_str(&format!("  fell short: {note}\n"));
                }
            }
            p.push('\n');
        }

        p.push_str("# Your task\n\n");
        p.push_str(
            "Improve the program. Output the COMPLETE improved program in a single \
             code block. Do not add explanations outside the code block. The \
             program must compile and pass the evaluator.\n",
        );
        p
    }

    /// Build the system prompt (evaluator contract + output format).
    #[must_use]
    pub fn build_system_prompt(&self) -> String {
        "You are an expert program optimizer. You receive a program and \
         evaluator feedback, and output an improved version of the program \
         in a single code block. Preserve the program's interface; improve \
         correctness and performance. Never add explanations outside the \
         code block."
            .to_string()
    }
}

fn metric_name(p: &Program) -> String {
    p.metrics.keys().next().cloned().unwrap_or_default()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…[truncated]", &s[..max])
    }
}

fn truncate_code(code: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = code.lines().collect();
    if lines.len() <= max_lines {
        code.to_string()
    } else {
        let mut out = lines[..max_lines].join("\n");
        out.push_str(&format!("\n// …{} more lines", lines.len() - max_lines));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sampler() -> PromptSampler {
        PromptSampler::new(PromptConfig {
            instruction: "make it faster".into(),
            ..PromptConfig::default()
        })
    }

    fn prog(code: &str, score: f64) -> Program {
        let mut p = Program::new(code, Uuid::new_v4());
        p.set_metrics([("score".into(), score)]);
        p
    }

    #[test]
    fn prompt_carries_parent_and_artifacts() {
        let s = sampler();
        let parent = prog("fn main() {}", 0.5);
        let artifacts = "test x failed: expected 42";
        let prompt = s.build_mutation_prompt(&parent, &[], Some(artifacts), &[]);
        assert!(prompt.contains("fn main() {}"), "parent code present");
        assert!(prompt.contains("test x failed"), "artifacts present");
    }

    #[test]
    fn prompt_lists_inspirations_and_history() {
        let s = sampler();
        let parent = prog("parent", 0.4);
        let insps = vec![prog("insp one", 0.6), prog("insp two", 0.8)];
        let history = vec![AttemptRecord {
            summary: "attempt 1: added loop".into(),
            score: 0.3,
            failure_note: Some("infinite loop on empty input".into()),
        }];
        let prompt = s.build_mutation_prompt(&parent, &insps, None, &history);
        assert!(prompt.contains("insp one"));
        assert!(prompt.contains("insp two"));
        assert!(prompt.contains("infinite loop on empty input"));
        assert!(prompt.contains("Prior attempts"));
    }

    #[test]
    fn long_parent_truncated_with_marker() {
        let s = sampler();
        let long_code = "line\n".repeat(500);
        let parent = prog(&long_code, 0.1);
        let prompt = s.build_mutation_prompt(&parent, &[], None, &[]);
        assert!(prompt.contains("more lines"), "truncation marker present");
        assert!(prompt.len() < long_code.len() * 2, "prompt bounded");
    }

    #[test]
    fn system_prompt_has_output_contract() {
        let s = sampler();
        let sys = s.build_system_prompt();
        assert!(sys.contains("code block"));
        assert!(sys.contains("improved"));
    }
}
