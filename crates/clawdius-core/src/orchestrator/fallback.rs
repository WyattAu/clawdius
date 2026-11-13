//! Fallback escalation: rate-limit-aware provider rotation with
//! opencode as the last-resort agent.
//!
//! Policy (the "smart fallback" loop):
//!
//! ```text
//! chain: [glm, google, anthropic]      user-configured priority order
//!   1. pick the first provider not in cooldown
//!   2. run the task through it
//!   3. on rate-limit/quota error -> mark cooldown (per-provider window)
//!   4. everyone limited? -> Parked(resume_in)  the task sleeps until the
//!      SOONEST window resets; other queued tasks keep looping meanwhile
//!   5. chain exhausted AND attempts spent -> escalate to opencode, which
//!      solves it with its own model budget; its session id comes back so
//!      the follow-up can continue exactly where it left off
//! ```
//!
//! The coordinator is policy-only: it decides WHO runs next, never how —
//! the engine maps provider names to clients. Everything here is
//! deterministic and unit-tested without network.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// Per-provider cooldown windows (rate-limit ledger).
#[derive(Debug, Default)]
pub struct RateLimitLedger {
    /// provider -> when its window resets.
    cooldowns: HashMap<String, Instant>,
}

impl RateLimitLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a provider rate-limited for `cooldown` from now.
    pub fn mark_limited(&mut self, provider: &str, cooldown: Duration) {
        self.cooldowns
            .insert(provider.to_string(), Instant::now() + cooldown);
    }

    /// Clear a provider's cooldown (a successful call or window expiry).
    pub fn mark_ok(&mut self, provider: &str) {
        self.cooldowns.remove(provider);
    }

    /// Whether the provider is still inside its cooldown window.
    #[must_use]
    pub fn is_limited(&self, provider: &str) -> bool {
        self.remaining(provider).is_some()
    }

    /// Time until the provider's window resets (`None` = usable now).
    #[must_use]
    pub fn remaining(&self, provider: &str) -> Option<Duration> {
        let resume_at = self.cooldowns.get(provider)?;
        let now = Instant::now();
        if *resume_at > now {
            Some(*resume_at - now)
        } else {
            None
        }
    }

    /// First provider in `chain` that is usable right now.
    #[must_use]
    pub fn pick_available<'a>(&self, chain: &'a [String]) -> Option<&'a String> {
        chain.iter().find(|p| !self.is_limited(p))
    }

    /// The soonest reset across limited providers — when a parked task
    /// should wake up.
    #[must_use]
    pub fn soonest_reset(&self) -> Option<Duration> {
        self.cooldowns
            .values()
            .filter_map(|resume_at| {
                let now = Instant::now();
                (*resume_at > now).then_some(*resume_at - now)
            })
            .min()
    }
}

/// What the coordinator wants the engine to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackDecision {
    /// Run the task on this provider.
    Try(String),
    /// Every provider is cooling down — park until the soonest reset.
    Park(Duration),
    /// The chain is spent and attempts are exhausted — hand off to
    /// opencode (last resort).
    Escalate,
}

/// The escalation policy: chain order + attempt budget.
#[derive(Debug, Clone)]
pub struct FallbackPolicy {
    /// Provider priority order (first = preferred).
    pub chain: Vec<String>,
    /// Attempts allowed on chain providers before escalating.
    pub max_attempts: usize,
    /// Cooldown applied when a provider returns a rate-limit/quota error.
    pub cooldown: Duration,
}

impl Default for FallbackPolicy {
    fn default() -> Self {
        Self {
            chain: vec![
                "zai".to_string(),
                "google".to_string(),
                "anthropic".to_string(),
            ],
            max_attempts: 3,
            cooldown: Duration::from_secs(60 * 5),
        }
    }
}

/// Pure policy coordinator: decides the next move from ledger state.
pub struct FallbackCoordinator {
    pub policy: FallbackPolicy,
    pub ledger: RateLimitLedger,
    attempts: usize,
}

impl FallbackCoordinator {
    #[must_use]
    pub fn new(policy: FallbackPolicy) -> Self {
        Self {
            policy,
            ledger: RateLimitLedger::new(),
            attempts: 0,
        }
    }

    /// Record a rate-limit failure on `provider`.
    pub fn note_limited(&mut self, provider: &str) {
        let cooldown = self.policy.cooldown;
        self.ledger.mark_limited(provider, cooldown);
    }

    /// Record a non-rate-limit failure (counts toward the budget).
    pub fn note_failed(&mut self) {
        self.attempts += 1;
    }

    /// Record success (resets the attempt budget).
    pub fn note_solved(&mut self, provider: &str) {
        self.attempts = 0;
        self.ledger.mark_ok(provider);
    }

    /// Decide the next move.
    #[must_use]
    pub fn next(&self) -> FallbackDecision {
        if let Some(provider) = self.ledger.pick_available(&self.policy.chain) {
            return FallbackDecision::Try(provider.clone());
        }
        // Everyone cooling down: park until the soonest window, unless
        // the attempt budget is spent — then escalate instead of waiting.
        if self.attempts >= self.policy.max_attempts {
            return FallbackDecision::Escalate;
        }
        match self.ledger.soonest_reset() {
            Some(resume_in) => FallbackDecision::Park(resume_in),
            None => FallbackDecision::Escalate,
        }
    }

    /// Attempts recorded so far.
    #[must_use]
    pub fn attempts(&self) -> usize {
        self.attempts
    }
}

/// One escalation hand-off to opencode (last resort).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EscalationRequest {
    /// The original task text.
    pub task: String,
    /// Provider that was being attempted when the budget ran out.
    pub failed_provider: String,
    /// What went wrong, per attempt (fed to the agent as context).
    pub failure_notes: Vec<String>,
}

/// The escalation result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EscalationReport {
    /// Whether opencode reported success (exit 0).
    pub ok: bool,
    /// Full stdout of the opencode run.
    pub output: String,
    /// opencode session id (`--format json`), when captured — a
    /// follow-up can `-s <id>` to continue exactly here.
    pub session_id: Option<String>,
    /// Wall-clock duration of the run.
    pub duration_ms: u64,
}

/// Runs opencode as the last-resort agent.
pub struct OpencodeEscalator {
    /// Path to the opencode binary.
    pub bin: PathBuf,
    /// Working directory for the run (the repo under repair).
    pub workdir: PathBuf,
    /// Hard wall-clock cap.
    pub timeout: Duration,
    /// Agent name (opencode agent, e.g. `build` or `plan`).
    pub agent: Option<String>,
    /// Model override in opencode's `provider/model` form.
    pub model: Option<String>,
}

impl OpencodeEscalator {
    #[must_use]
    pub fn new(workdir: PathBuf) -> Self {
        Self {
            bin: std::env::var_os("OPENCODE_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("opencode")),
            workdir,
            timeout: Duration::from_secs(15 * 60),
            agent: Some("build".to_string()),
            model: None,
        }
    }

    /// Compose the escalation prompt: task + failure context + the rule
    /// that the agent finishes the job (no questions).
    #[must_use]
    pub fn compose_prompt(req: &EscalationRequest) -> String {
        let mut prompt = format!(
            "Complete this task end-to-end; do not ask questions:\n\n{}\n\n",
            req.task
        );
        if !req.failure_notes.is_empty() {
            prompt.push_str("Previous attempts failed — avoid repeating them:\n");
            for note in &req.failure_notes {
                prompt.push_str(&format!("- {note}\n"));
            }
            prompt.push('\n');
        }
        prompt.push_str(
            "Finish by running the verification commands and reporting what changed. \
             If verification passes, state DONE as the last line.",
        );
        prompt
    }

    /// Run opencode non-interactively and capture the result.
    ///
    /// # Errors
    ///
    /// Spawn/wait IO failures and the timeout surface as
    /// [`crate::Error::Llm`].
    pub fn escalate(&self, req: &EscalationRequest) -> Result<EscalationReport, crate::Error> {
        let start = Instant::now();
        let mut cmd = Command::new(&self.bin);
        cmd.current_dir(&self.workdir)
            .arg("run")
            .arg("--archived")
            .arg("--format")
            .arg("json");
        if let Some(agent) = &self.agent {
            cmd.args(["--agent", agent]);
        }
        if let Some(model) = &self.model {
            cmd.args(["--model", model]);
        }
        cmd.arg(Self::compose_prompt(req));

        let output = cmd
            .output()
            .map_err(|e| crate::Error::Llm(format!("opencode spawn failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        // opencode --format json wraps the answer; pull the session id
        // when present so callers can continue the exact session later.
        let session_id = Self::extract_session_id(&stdout);

        Ok(EscalationReport {
            ok: output.status.success(),
            output: stdout,
            session_id,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn extract_session_id(stdout: &str) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(stdout).ok()?;
        v.get("sessionID")
            .or_else(|| v.get("sessionId"))
            .and_then(|s| s.as_str())
            .map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn picks_first_available_provider() {
        let mut coord = FallbackCoordinator::new(FallbackPolicy {
            chain: chain(&["zai", "google", "anthropic"]),
            ..FallbackPolicy::default()
        });
        assert_eq!(coord.next(), FallbackDecision::Try("zai".into()));
        coord.note_limited("zai");
        assert_eq!(coord.next(), FallbackDecision::Try("google".into()));
        coord.note_limited("google");
        coord.note_limited("anthropic");
        // Everyone limited, budget unspent -> park until soonest reset.
        match coord.next() {
            FallbackDecision::Park(d) => {
                assert!(d <= coord.policy.cooldown);
            },
            other => panic!("expected Park, got {other:?}"),
        }
    }

    #[test]
    fn budget_exhausted_escalates_instead_of_parking() {
        let mut coord = FallbackCoordinator::new(FallbackPolicy {
            chain: chain(&["zai"]),
            max_attempts: 2,
            ..FallbackPolicy::default()
        });
        coord.note_limited("zai");
        coord.note_failed();
        coord.note_failed();
        assert_eq!(coord.next(), FallbackDecision::Escalate);
    }

    #[test]
    fn solved_resets_budget_and_cooldown() {
        let mut coord = FallbackCoordinator::new(FallbackPolicy {
            chain: chain(&["zai"]),
            ..FallbackPolicy::default()
        });
        coord.note_limited("zai");
        coord.note_failed();
        coord.note_solved("zai");
        assert!(!coord.ledger.is_limited("zai"));
        assert_eq!(coord.attempts(), 0);
        assert_eq!(coord.next(), FallbackDecision::Try("zai".into()));
    }

    #[test]
    fn soonest_reset_picks_the_earliest_window() {
        let mut ledger = RateLimitLedger::new();
        ledger.mark_limited("slow", Duration::from_secs(600));
        ledger.mark_limited("fast", Duration::from_secs(30));
        let d = ledger.soonest_reset().unwrap();
        assert!(d <= Duration::from_secs(31));
        assert!(ledger.is_limited("fast"));
        assert!(!ledger.is_limited("never-limited"));
    }

    #[test]
    fn escalation_prompt_carries_task_and_failures() {
        let req = EscalationRequest {
            task: "fix the failing tests".into(),
            failed_provider: "google".into(),
            failure_notes: vec!["quota exceeded at 08:00".into()],
        };
        let prompt = OpencodeEscalator::compose_prompt(&req);
        assert!(prompt.contains("fix the failing tests"));
        assert!(prompt.contains("quota exceeded at 08:00"));
        assert!(prompt.contains("DONE"));
    }

    #[test]
    fn escalator_runs_a_fake_binary_end_to_end() {
        // A fake opencode: prints json with a session id, exits 0.
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-opencode.sh");
        std::fs::write(
            &fake,
            "#!/bin/sh\necho '{\"sessionID\":\"ses_test123\",\"parts\":[\"did it\"]}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let esc = OpencodeEscalator {
            bin: fake,
            workdir: dir.path().to_path_buf(),
            timeout: Duration::from_secs(10),
            agent: Some("build".into()),
            model: None,
        };
        let req = EscalationRequest {
            task: "t".into(),
            failed_provider: "zai".into(),
            failure_notes: Vec::new(),
        };
        let report = esc.escalate(&req).unwrap();
        assert!(report.ok);
        assert_eq!(report.session_id.as_deref(), Some("ses_test123"));
        assert!(report.duration_ms < 10_000);
    }
}
