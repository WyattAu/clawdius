//! Formalized dev pipeline: the product loop as data.
//!
//! The canon loop: market research → technical research → feature
//! suggestions → architecture & planning → implementation → verify →
//! test → lint → repeat. Stages are typed; the loop repeats its tail
//! (implementation..lint) until the gates are green or the loop budget
//! is spent.
//!
//! LLM stages (research/suggestion/planning/implementation) execute via
//! the `auto` engine; gate stages (verify/test/lint) are deterministic
//! shell commands — the same ground-truth rule the engine already uses.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;

/// The kind of a pipeline stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum StageKind {
    /// Competitors, positioning, market gaps.
    MarketResearch,
    /// Technical landscape: prior art, protocols, libraries, benchmarks.
    TechnicalResearch,
    /// Distill research into candidate features, ranked.
    FeatureSuggestions,
    /// Architecture + step-by-step plan with acceptance criteria.
    ArchitecturePlanning,
    /// Build it (the auto engine with its tool loop).
    Implementation,
    /// Deterministic gate: compile/verify.
    Verify,
    /// Deterministic gate: test suite.
    Test,
    /// Deterministic gate: lint + format.
    Lint,
}

impl StageKind {
    /// Stage slug (journal/plan ids).
    #[must_use]
    pub fn slug(&self) -> &'static str {
        match self {
            Self::MarketResearch => "market-research",
            Self::TechnicalResearch => "technical-research",
            Self::FeatureSuggestions => "feature-suggestions",
            Self::ArchitecturePlanning => "architecture-planning",
            Self::Implementation => "implementation",
            Self::Verify => "verify",
            Self::Test => "test",
            Self::Lint => "lint",
        }
    }

    /// Whether humans/models produce the artifact (vs. shell gates).
    #[must_use]
    pub fn is_llm_stage(&self) -> bool {
        !matches!(self, Self::Verify | Self::Test | Self::Lint)
    }
}

/// One pipeline stage.
#[derive(Debug, Clone, Serialize)]
pub struct Stage {
    pub kind: StageKind,
    /// For LLM stages: the instruction to the engine (goal appended).
    /// For gates: unused (defaults come from [`default_gate_command`]).
    pub instruction: String,
}

/// A pipeline definition.
#[derive(Debug, Clone, Serialize)]
pub struct Pipeline {
    pub name: String,
    pub goal: String,
    pub stages: Vec<Stage>,
    /// Repeat budget for the implement..lint tail.
    pub max_loops: usize,
    /// Index of the stage the loop restarts from on gate failure.
    pub repeat_from: usize,
}

impl Pipeline {
    /// The builtin product loop for `goal` (the canon 9 stages; gates
    /// repeat their tail).
    #[must_use]
    pub fn product_loop(goal: &str) -> Self {
        let mk = |kind: StageKind, instruction: &str| Stage {
            kind,
            instruction: instruction.to_string(),
        };
        let mk_gate = |kind: StageKind| Stage {
            kind,
            instruction: String::new(),
        };
        Self {
            name: "product-loop".to_string(),
            goal: goal.to_string(),
            stages: vec![
                mk(
                    StageKind::MarketResearch,
                    "Research the market landscape for this goal: competitors, positioning, pricing, gaps. Produce a terse markdown brief with a competitor table.",
                ),
                mk(
                    StageKind::TechnicalResearch,
                    "Research the technical landscape: prior art, protocols, libraries, benchmarks, known pitfalls. Cite sources; end with a recommended approach.",
                ),
                mk(
                    StageKind::FeatureSuggestions,
                    "From the research, propose the top 5 features worth building, ranked by value/effort. One paragraph each: what, why now, effort estimate.",
                ),
                mk(
                    StageKind::ArchitecturePlanning,
                    "Produce the architecture and a step-by-step implementation plan with acceptance criteria per step. Decide what is NOT being built (explicit non-goals).",
                ),
                mk(
                    StageKind::Implementation,
                    "Implement the plan against the working tree. Use the available tools; finish by running the gates.",
                ),
                mk_gate(StageKind::Verify),
                mk_gate(StageKind::Test),
                mk_gate(StageKind::Lint),
            ],
            max_loops: 3,
            repeat_from: 4, // restart at Implementation on gate failure
        }
    }

    /// The engineering-only loop for `goal`: plan → implement → gates,
    /// repeating the implement..lint tail. No research stages — for
    /// testing the loop mechanics and for goals that need no market
    /// study.
    #[must_use]
    pub fn dev_loop(goal: &str) -> Self {
        let mk = |kind: StageKind, instruction: &str| Stage {
            kind,
            instruction: instruction.to_string(),
        };
        let mk_gate = |kind: StageKind| Stage {
            kind,
            instruction: String::new(),
        };
        Self {
            name: "dev-loop".to_string(),
            goal: goal.to_string(),
            stages: vec![
                mk(
                    StageKind::ArchitecturePlanning,
                    "Produce a short step-by-step plan with acceptance criteria. Keep it minimal: this loop exists to converge fast.",
                ),
                mk(
                    StageKind::Implementation,
                    "Implement the plan against the working tree. Use the available tools; finish by running the gates.",
                ),
                mk_gate(StageKind::Verify),
                mk_gate(StageKind::Test),
                mk_gate(StageKind::Lint),
            ],
            max_loops: 2,
            repeat_from: 2, // restart at Implementation on gate failure
        }
    }

    /// Structural validation: non-empty, gates present, repeat index sane.
    ///
    /// # Errors
    ///
    /// [`PipelineError`] describes the violation.
    pub fn validate(&self) -> Result<(), PipelineError> {
        if self.stages.is_empty() {
            return Err(PipelineError::Empty);
        }
        let has = |k: &StageKind| self.stages.iter().any(|s| &s.kind == k);
        for gate in [StageKind::Verify, StageKind::Test, StageKind::Lint] {
            if !has(&gate) {
                return Err(PipelineError::MissingGate(gate.slug().to_string()));
            }
        }
        if self.repeat_from >= self.stages.len() {
            return Err(PipelineError::BadRepeatIndex {
                index: self.repeat_from,
                len: self.stages.len(),
            });
        }
        if self.max_loops == 0 {
            return Err(PipelineError::ZeroLoops);
        }
        Ok(())
    }

    /// Human-readable execution plan (the dry-run artifact).
    #[must_use]
    pub fn plan(&self) -> String {
        let mut out = format!(
            "Pipeline `{}` — goal: {}\nLoops: {} (repeat tail from stage {})\n\n",
            self.name,
            self.goal,
            self.max_loops,
            self.stages[self.repeat_from].kind.slug(),
        );
        for (i, stage) in self.stages.iter().enumerate() {
            let marker = if stage.kind.is_llm_stage() {
                "LLM"
            } else {
                "GATE"
            };
            out.push_str(&format!("  {}. [{marker}] {}\n", i + 1, stage.kind.slug()));
        }
        out
    }
}

/// Pipeline validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Empty,
    MissingGate(String),
    BadRepeatIndex { index: usize, len: usize },
    ZeroLoops,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "pipeline has no stages"),
            Self::MissingGate(slug) => {
                write!(f, "pipeline is missing the required `{slug}` gate")
            },
            Self::BadRepeatIndex { index, len } => {
                write!(f, "repeat_from {index} out of range (stages: {len})")
            },
            Self::ZeroLoops => write!(f, "max_loops must be at least 1"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// The result of one gate stage.
#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub slug: String,
    pub ok: bool,
    /// Last ~40 lines of combined output (for the next loop's context).
    pub output_tail: String,
    pub duration_ms: u64,
}

/// The default gate command for a Rust workspace.
#[must_use]
pub fn default_gate_command(kind: &StageKind) -> Vec<String> {
    match kind {
        StageKind::Verify => vec!["cargo", "check", "--workspace"]
            .into_iter()
            .map(String::from)
            .collect(),
        StageKind::Test => vec!["cargo", "test", "--workspace"]
            .into_iter()
            .map(String::from)
            .collect(),
        // fmt check first (cheap fail), then clippy.
        StageKind::Lint => vec![
            "sh",
            "-c",
            "cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        _ => vec!["true".to_string()],
    }
}

/// Run one gate's command in `workdir`; capture a tail of the output.
#[allow(clippy::needless_pass_by_value)]
pub fn run_gate(kind: &StageKind, workdir: &Path) -> GateResult {
    let cmd = default_gate_command(kind);
    run_command(kind.slug(), &cmd, workdir)
}

/// Run an explicit command as a gate (test-visible).
pub fn run_command(slug: &str, cmd: &[String], workdir: &Path) -> GateResult {
    let start = Instant::now();
    let Some((program, args)) = cmd.split_first() else {
        return GateResult {
            slug: slug.to_string(),
            ok: true,
            output_tail: String::new(),
            duration_ms: 0,
        };
    };
    let output = Command::new(program)
        .args(args)
        .current_dir(workdir)
        .output();

    let (ok, tail) = match output {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            let tail: Vec<String> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(String::from)
                .collect();
            let start_line = tail.len().saturating_sub(40);
            (out.status.success(), tail[start_line..].join("\n"))
        },
        Err(e) => (false, format!("spawn failed: {e}")),
    };
    GateResult {
        slug: slug.to_string(),
        ok,
        output_tail: tail,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Timeout hint for callers driving gates with wall-clock limits.
pub const GATE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_pipeline_validates_and_has_the_canon_stages() {
        let p = Pipeline::product_loop("ship the thing");
        p.validate().expect("builtin must validate");
        let slugs: Vec<&str> = p.stages.iter().map(|s| s.kind.slug()).collect();
        assert_eq!(
            slugs,
            vec![
                "market-research",
                "technical-research",
                "feature-suggestions",
                "architecture-planning",
                "implementation",
                "verify",
                "test",
                "lint",
            ]
        );
        assert_eq!(p.repeat_from, 4);
        assert!(p.stages.iter().filter(|s| s.kind.is_llm_stage()).count() == 5);
    }

    #[test]
    fn dev_loop_validates_and_starts_repeat_at_implementation() {
        let p = Pipeline::dev_loop("g");
        p.validate().expect("dev-loop must validate");
        assert_eq!(p.name, "dev-loop");
        assert_eq!(p.stages[0].kind, StageKind::ArchitecturePlanning);
        assert_eq!(p.stages[2].kind, StageKind::Verify);
        assert_eq!(p.repeat_from, 2);
        assert!(!p.stages.iter().any(|s| s.kind == StageKind::MarketResearch));
    }

    #[test]
    fn validate_rejects_missing_gates_and_bad_indices() {
        let mut p = Pipeline::product_loop("g");
        p.stages.retain(|s| s.kind != StageKind::Lint);
        assert_eq!(
            p.validate(),
            Err(PipelineError::MissingGate("lint".to_string()))
        );

        let mut p = Pipeline::product_loop("g");
        p.repeat_from = 99;
        assert!(matches!(
            p.validate(),
            Err(PipelineError::BadRepeatIndex { .. })
        ));

        let mut p = Pipeline::product_loop("g");
        p.max_loops = 0;
        assert_eq!(p.validate(), Err(PipelineError::ZeroLoops));

        let mut p = Pipeline::product_loop("g");
        p.stages.clear();
        assert_eq!(p.validate(), Err(PipelineError::Empty));
    }

    #[test]
    fn plan_lists_stages_with_markers() {
        let p = Pipeline::product_loop("goal X");
        let plan = p.plan();
        assert!(plan.contains("Pipeline `product-loop` — goal: goal X"));
        assert!(plan.contains("[LLM] market-research"));
        assert!(plan.contains("[GATE] lint"));
        assert!(plan.contains("repeat tail from stage implementation"));
    }

    #[test]
    fn gate_runner_captures_pass_and_fail() {
        let dir = tempfile::tempdir().unwrap();
        let ok = run_command("probe-ok", &["true".to_string()], dir.path());
        assert!(ok.ok);

        let bad = run_command(
            "probe-bad",
            &[
                "sh".to_string(),
                "-c".to_string(),
                "echo failing-fast >&2; exit 3".to_string(),
            ],
            dir.path(),
        );
        assert!(!bad.ok);
        assert!(bad.output_tail.contains("failing-fast"));
    }

    #[test]
    fn default_gate_commands_target_cargo() {
        let verify = default_gate_command(&StageKind::Verify);
        assert_eq!(verify, vec!["cargo", "check", "--workspace"]);
        let lint = default_gate_command(&StageKind::Lint);
        assert!(lint.join(" ").contains("cargo clippy"));
    }
}
