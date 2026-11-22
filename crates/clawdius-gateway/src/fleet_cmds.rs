//! Fleet commands for chat platforms.
//!
//! Turns `/fleet …` chat lines into calls on the multi-repo dev-cycle
//! machinery ([`clawdius_core::fleet`]) so the central agent can be driven
//! from Telegram without a TUI. Verbs:
//!
//! | Verb | Class | Effect |
//! |---|---|---|
//! | `status` | readonly | lanes, queues, review backlog |
//! | `repos` | readonly | active repo list |
//! | `review` | readonly | pending gate records |
//! | `work <repo> <task…>` | mutating | enqueue + dispatch to OpenCode |
//! | `collect <repo> ok <gates> -- <reason>` | mutating | finish + gate policy |
//! | `collect <repo> fail <reason…>` | mutating | requeue the cycle |
//! | `approve <repo> <gate>` / `deny …` | mutating | resolve a review record |
//!
//! # Authorization
//!
//! Capability-gate pattern: every verb is classed readonly or mutating, and
//! mutating verbs require the sender id to be in the admin allowlist. An
//! empty allowlist denies every mutating verb (fail closed).
//!
//! # Task preservation
//!
//! [`FleetRouter::dispatch_next`] pops the task from the queue, so the
//! commander keeps the in-flight task per repo and re-enqueues it when a
//! cycle is collected as failed — a failed cycle never loses its work.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::adapter::User;
use crate::error::GatewayError;
use clawdius_core::fleet::{
    CyclePhase, FleetState, GateDecision, GateId, OpenCodeClient, ReviewQueue,
};

/// A parsed `/fleet` command.
#[derive(Debug, Clone)]
pub enum FleetCmd {
    /// Lane, queue, and review summary.
    Status,
    /// Active repo list.
    Repos,
    /// Pending review records.
    Review,
    /// Verb usage text.
    Help,
    /// Enqueue a task and dispatch it to the repo's OpenCode lane.
    Work {
        /// Repo short name.
        repo: String,
        /// Task text for the agent.
        task: String,
    },
    /// Report a finish (`ok`) or failure (`fail`) for the repo's cycle.
    Collect {
        /// Repo short name.
        repo: String,
        /// True = finish report, false = execution failure.
        ok: bool,
        /// Hard gates the cycle satisfied (finish reports only).
        passed: Vec<GateId>,
        /// Human reason attached to the report.
        reason: String,
    },
    /// Resolve a pending review record.
    Decide {
        /// Repo short name.
        repo: String,
        /// Which gate record.
        gate: GateId,
        /// Approve the skip, or reject it.
        approved: bool,
    },
}

impl FleetCmd {
    /// Whether this command mutates fleet state (and thus requires an admin
    /// sender).
    #[must_use]
    pub fn is_mutating(&self) -> bool {
        !matches!(self, Self::Status | Self::Repos | Self::Review | Self::Help)
    }
}

/// One dispatched task held until its cycle is collected.
#[derive(Debug, Clone)]
struct InFlight {
    task: String,
    session: String,
}

/// The persistable commander document: fleet state + review queue.
#[derive(serde::Serialize, serde::Deserialize)]
struct Persisted {
    state: FleetState,
    review: ReviewQueue,
}

/// Chat-facing driver for the fleet dev-cycle.
///
/// Holds the persistable [`FleetState`] plus the review queue and the
/// in-flight dispatches, and talks to the OpenCode server for `work`.
pub struct FleetCommander {
    client: Arc<OpenCodeClient>,
    admins: HashSet<String>,
    state: FleetState,
    review: ReviewQueue,
    in_flight: BTreeMap<String, InFlight>,
    state_path: Option<PathBuf>,
}

impl FleetCommander {
    /// Create a commander over an OpenCode client.
    ///
    /// `admins` is the mutating-verb allowlist (platform user ids); empty
    /// means every mutating verb is denied.
    #[must_use]
    pub fn new(client: OpenCodeClient, admins: Vec<String>) -> Self {
        Self {
            client: Arc::new(client),
            admins: admins.into_iter().collect(),
            state: FleetState::default(),
            review: ReviewQueue::default(),
            in_flight: BTreeMap::new(),
            state_path: None,
        }
    }

    /// Load state from, and persist to, `path` (JSON).
    ///
    /// A missing file starts from empty state; an unreadable/corrupt file is
    /// ignored in favor of starting fresh (the dashboard can rebuild).
    #[must_use]
    pub fn with_state_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(doc) = serde_json::from_slice::<Persisted>(&bytes) {
                self.state = doc.state;
                self.review = doc.review;
            }
        }
        self.state_path = Some(path);
        self
    }

    /// Replace the gate policy (before first dispatch).
    pub fn set_gate_policy(&mut self, policy: clawdius_core::fleet::GatePolicy) {
        self.state.gate_policy = policy;
    }

    /// Register (or update) a repo and mark it active.
    pub fn register_repo(&mut self, name: impl Into<String>, path: impl Into<String>) {
        let mut entry = clawdius_core::fleet::RepoEntry::new(name, path);
        entry.active = true;
        self.state.registry.upsert(entry);
    }

    /// Build a commander from the environment.
    ///
    /// - `CLAWDIUS_FLEET_ADMINS` (required): comma-separated platform user
    ///   ids allowed to run mutating verbs.
    /// - `CLAWDIUS_FLEET_STATE` (optional): state file path.
    ///
    /// The OpenCode server is auto-discovered via
    /// [`OpenCodeClient::discover`].
    ///
    /// # Errors
    ///
    /// A string reason when the env is missing or discovery fails; callers
    /// treat that as "fleet commands stay disabled".
    pub fn from_env() -> Result<Self, String> {
        let admins_csv = std::env::var("CLAWDIUS_FLEET_ADMINS")
            .map_err(|_| "CLAWDIUS_FLEET_ADMINS not set".to_string())?;
        let admins: Vec<String> = admins_csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let client =
            OpenCodeClient::discover().map_err(|e| format!("opencode discover failed: {e}"))?;
        let mut cmdr = Self::new(client, admins);
        if let Ok(path) = std::env::var("CLAWDIUS_FLEET_STATE") {
            if !path.is_empty() {
                cmdr = cmdr.with_state_path(path);
            }
        }
        Ok(cmdr)
    }

    /// Parse a chat line into a [`FleetCmd`].
    ///
    /// `None` means the line is not addressed to the fleet (fall through to
    /// the LLM). Lines that *are* addressed to the fleet but carry bad
    /// arguments return [`FleetCmd::Help`] so the sender always gets usage
    /// text instead of a chat-model guess.
    #[must_use]
    pub fn parse(text: &str) -> Option<FleetCmd> {
        let trimmed = text.trim();
        let body = if let Some(rest) = trimmed.strip_prefix("/fleet") {
            rest
        } else if trimmed == "fleet" || trimmed.strip_prefix("fleet ").is_some() {
            trimmed.strip_prefix("fleet ").unwrap_or("")
        } else {
            return None;
        };
        let body = body.trim();
        let mut toks = body.split_whitespace();
        let Some(verb) = toks.next() else {
            return Some(FleetCmd::Help);
        };
        match verb.to_ascii_lowercase().as_str() {
            "status" => Some(FleetCmd::Status),
            "repos" => Some(FleetCmd::Repos),
            "review" => Some(FleetCmd::Review),
            "work" => {
                let Some(repo) = toks.next() else {
                    return Some(FleetCmd::Help);
                };
                let repo = repo.trim_start_matches('#').to_string();
                let task: Vec<&str> = toks.collect();
                if task.is_empty() {
                    return Some(FleetCmd::Help);
                }
                Some(FleetCmd::Work {
                    repo,
                    task: task.join(" "),
                })
            },
            "collect" => {
                let Some(repo) = toks.next() else {
                    return Some(FleetCmd::Help);
                };
                let repo = repo.trim_start_matches('#').to_string();
                let Some(outcome) = toks.next() else {
                    return Some(FleetCmd::Help);
                };
                let rest: Vec<&str> = toks.collect();
                match outcome.to_ascii_lowercase().as_str() {
                    "ok" | "finish" => {
                        let (gate_part, reason) = match rest.iter().position(|t| *t == "--") {
                            Some(idx) => (rest[..idx].to_vec(), rest[idx + 1..].join(" ")),
                            None => (rest.clone(), String::new()),
                        };
                        let mut passed = Vec::new();
                        for name in gate_part {
                            for g in name.split(',') {
                                let g = g.trim();
                                if g.is_empty() {
                                    continue;
                                }
                                let Some(gate_id) = parse_gate(g) else {
                                    return Some(FleetCmd::Help);
                                };
                                passed.push(gate_id);
                            }
                        }
                        Some(FleetCmd::Collect {
                            repo,
                            ok: true,
                            passed,
                            reason,
                        })
                    },
                    "fail" | "failed" => Some(FleetCmd::Collect {
                        repo,
                        ok: false,
                        passed: Vec::new(),
                        reason: rest.join(" "),
                    }),
                    _ => Some(FleetCmd::Help),
                }
            },
            "approve" | "deny" => {
                let Some(repo) = toks.next() else {
                    return Some(FleetCmd::Help);
                };
                let repo = repo.trim_start_matches('#').to_string();
                let Some(gate_label) = toks.next() else {
                    return Some(FleetCmd::Help);
                };
                let Some(gate) = parse_gate(gate_label) else {
                    return Some(FleetCmd::Help);
                };
                Some(FleetCmd::Decide {
                    repo,
                    gate,
                    approved: verb.eq_ignore_ascii_case("approve"),
                })
            },
            _ => Some(FleetCmd::Help),
        }
    }

    /// Execute a parsed command for a sender.
    ///
    /// # Errors
    ///
    /// [`GatewayError::Unauthorized`] for non-admin senders on mutating
    /// verbs; [`GatewayError::Config`] for unknown repos / bad lane state;
    /// [`GatewayError::Agent`] for OpenCode failures.
    pub async fn execute(&mut self, cmd: FleetCmd, user: &User) -> Result<String, GatewayError> {
        self.ensure_authorized(&cmd, user)?;
        match cmd {
            FleetCmd::Help => Ok(Self::HELP_TEXT.to_string()),
            FleetCmd::Status => Ok(self.status_text()),
            FleetCmd::Repos => Ok(self.repos_text()),
            FleetCmd::Review => Ok(self.review_text()),
            FleetCmd::Work { repo, task } => self.work(&repo, &task).await,
            FleetCmd::Collect {
                repo,
                ok,
                passed,
                reason,
            } => self.collect(&repo, ok, &passed, &reason),
            FleetCmd::Decide {
                repo,
                gate,
                approved,
            } => self.decide(&repo, gate, approved),
        }
    }

    /// Capability gate: mutating verbs require the sender in the allowlist.
    fn ensure_authorized(&self, cmd: &FleetCmd, user: &User) -> Result<(), GatewayError> {
        if !cmd.is_mutating() || self.admins.contains(&user.id) {
            return Ok(());
        }
        Err(GatewayError::Unauthorized(format!(
            "fleet command '{}' requires an admin; ask to be added to CLAWDIUS_FLEET_ADMINS",
            match cmd {
                FleetCmd::Work { .. } => "work",
                FleetCmd::Collect { ok, .. } => {
                    if *ok {
                        "collect ok"
                    } else {
                        "collect fail"
                    }
                },
                FleetCmd::Decide { approved, .. } => {
                    if *approved {
                        "approve"
                    } else {
                        "deny"
                    }
                },
                _ => "mutating",
            }
        )))
    }

    /// `work`: enqueue, dispatch if the lane is free, create the OpenCode
    /// session, and prompt it in the background.
    async fn work(&mut self, repo: &str, task: &str) -> Result<String, GatewayError> {
        if let Some(lane) = self.state.router.lane(repo) {
            if lane.occupies_lane() {
                self.state.router.enqueue(repo, task);
                self.persist();
                return Ok(format!(
                    "{repo} is busy ({}) — task queued ({} ahead of it).",
                    phase_name(lane),
                    self.state.router.queue_len(repo) - 1,
                ));
            }
        }
        let entry = self.state.registry.get(repo).ok_or_else(|| {
            GatewayError::Config(format!(
                "unknown repo '{repo}' — register it via the dashboard first"
            ))
        })?;
        if !entry.active {
            return Err(GatewayError::Config(format!(
                "repo '{repo}' is inactive — activate it first"
            )));
        }
        self.state.router.enqueue(repo, task);
        let active = self.state.registry.active_names();
        let Some(decision) = self.state.router.dispatch_next(&active) else {
            self.persist();
            return Ok(format!("task queued for {repo}; lane busy elsewhere."));
        };
        let dispatched_repo = decision.repo.clone();
        let task_text = decision.task.clone();
        let dir = self
            .state
            .registry
            .get(&dispatched_repo)
            .map(|e| e.path.clone());
        let title: String = task_text.chars().take(60).collect();

        let session = match self
            .client
            .create_session(Some(&title), dir.as_deref())
            .await
        {
            Ok(s) => s,
            Err(e) => {
                // Roll the lane back and put the task back in the queue so
                // nothing is lost.
                self.state.router.report_failure(&dispatched_repo);
                self.state.router.enqueue(&dispatched_repo, &task_text);
                self.persist();
                return Err(GatewayError::Agent(format!(
                    "session create failed for {dispatched_repo}: {e}; task requeued"
                )));
            },
        };
        self.state.router.mark_running(&dispatched_repo);
        self.in_flight.insert(
            dispatched_repo.clone(),
            InFlight {
                task: task_text.clone(),
                session: session.id.clone(),
            },
        );

        let client = Arc::clone(&self.client);
        let bg_repo = dispatched_repo.clone();
        let bg_session = session.id.clone();
        let bg_task = task_text.clone();
        tokio::spawn(async move {
            match client
                .prompt(&bg_session, &bg_task, dir.as_deref(), None, None)
                .await
            {
                Ok(reply) => tracing::info!(
                    repo = %bg_repo,
                    session = %bg_session,
                    chars = reply.text.len(),
                    "fleet cycle prompt finished"
                ),
                Err(e) => tracing::warn!(
                    repo = %bg_repo,
                    session = %bg_session,
                    error = %e,
                    "fleet cycle prompt failed"
                ),
            }
        });

        self.persist();
        Ok(format!(
            "Dispatched `{}` → {dispatched_repo}\n\
             session {} — running in the background.\n\
             When it lands: `fleet collect {dispatched_repo} ok build,tests -- merged to main`",
            task_text, session.id,
        ))
    }

    /// `collect`: finish report through the gate policy, or failure requeue
    /// (which re-enqueues the in-flight task so no work is lost).
    fn collect(
        &mut self,
        repo: &str,
        ok: bool,
        passed: &[GateId],
        reason: &str,
    ) -> Result<String, GatewayError> {
        let lane = self
            .state
            .router
            .lane(repo)
            .ok_or_else(|| GatewayError::Config(format!("no fleet lane for '{repo}'")))?;
        if !lane.occupies_lane() {
            return Err(GatewayError::Config(format!(
                "{repo} lane is {} — nothing to collect",
                phase_name(lane)
            )));
        }
        if ok {
            let decision = self
                .state
                .router
                .report_finish(
                    repo,
                    passed,
                    reason,
                    &self.state.gate_policy,
                    &mut self.review,
                )
                .ok_or_else(|| {
                    GatewayError::Config(format!("{repo} lane could not accept a finish report"))
                })?;
            self.in_flight.remove(repo);
            self.persist();
            Ok(format_decision(repo, &decision, &self.review))
        } else {
            if let Some(inf) = self.in_flight.remove(repo) {
                self.state.router.enqueue(repo, &inf.task);
            }
            let requeued = self.state.router.report_failure(repo);
            self.persist();
            if requeued {
                let queued = self.state.router.queue_len(repo);
                Ok(if queued > 0 {
                    format!("{repo} cycle failed and was requeued ({queued} in queue).")
                } else {
                    format!("{repo} cycle failed and was requeued.")
                })
            } else {
                Ok(format!("{repo} lane released (was not in-flight)."))
            }
        }
    }

    /// `approve`/`deny`: resolve one pending review record.
    fn decide(&mut self, repo: &str, gate: GateId, approved: bool) -> Result<String, GatewayError> {
        if self.review.resolve(repo, gate, approved) {
            self.persist();
            Ok(format!(
                "{} on {repo}: {}",
                gate.label(),
                if approved {
                    "skip approved, record closed"
                } else {
                    "rejected — cycle flagged for rework"
                },
            ))
        } else {
            Err(GatewayError::Config(format!(
                "no pending {} record for {repo}",
                gate.label()
            )))
        }
    }

    fn status_text(&self) -> String {
        let mut names: Vec<String> = self.state.registry.active_names();
        for repo in self.state.router.pending_repos() {
            if !names.contains(&repo) {
                names.push(repo);
            }
        }
        names.sort();
        if names.is_empty() {
            return format!(
                "fleet idle — no active repos. review backlog: {}",
                self.review.pending().len()
            );
        }
        let mut lines = vec!["fleet lanes:".to_string()];
        for repo in &names {
            let lane = self
                .state
                .router
                .lane(repo)
                .map_or("idle", phase_name)
                .to_string();
            lines.push(format!(
                "  {repo}: {} · queue {}{}",
                lane,
                self.state.router.queue_len(repo),
                self.in_flight
                    .get(repo)
                    .map_or(String::new(), |f| format!(" · session {}", f.session)),
            ));
        }
        let pending_reviews = self.review.pending().len();
        if pending_reviews > 0 {
            lines.push(format!("review backlog: {pending_reviews} (fleet review)"));
        }
        lines.join("\n")
    }

    fn repos_text(&self) -> String {
        let active = self.state.registry.active_names();
        if active.is_empty() {
            return "no active repos registered.".to_string();
        }
        format!("active repos:\n  {}", active.join("\n  "))
    }

    fn review_text(&self) -> String {
        let pending = self.review.pending();
        if pending.is_empty() {
            return "review queue empty.".to_string();
        }
        let mut lines = vec![format!("{} pending review record(s):", pending.len())];
        for record in pending {
            lines.push(format!(
                "  {} · {} · {} ({})",
                record.repo,
                record.gate.label(),
                record.reason,
                record.flagged_at.format("%m-%d %H:%M UTC"),
            ));
        }
        lines.join("\n")
    }

    fn persist(&self) {
        let Some(path) = &self.state_path else {
            return;
        };
        let doc = Persisted {
            state: self.state.clone(),
            review: self.review.clone(),
        };
        if let Ok(json) = serde_json::to_vec_pretty(&doc) {
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!(error = %e, path = %path.display(), "fleet state persist failed");
            }
        }
    }

    /// Verb usage text.
    const HELP_TEXT: &'static str = "fleet verbs:
  fleet status
  fleet repos
  fleet review
  fleet work <repo> <task…>
  fleet collect <repo> ok <gates> -- <reason…>
  fleet collect <repo> fail <reason…>
  fleet approve <repo> <gate>
  fleet deny <repo> <gate>
gates: build, tests, clippy, proofs, audit
mutating verbs need an admin id in CLAWDIUS_FLEET_ADMINS";
}

/// Map a gate label back to its [`GateId`].
fn parse_gate(label: &str) -> Option<GateId> {
    let lower = label.to_ascii_lowercase();
    GateId::all().into_iter().find(|g| g.label() == lower)
}

/// Short name for a lane phase.
fn phase_name(phase: CyclePhase) -> &'static str {
    match phase {
        CyclePhase::Queued => "queued",
        CyclePhase::Dispatched => "dispatched",
        CyclePhase::Running => "running",
        CyclePhase::FinishReported => "finish-reported",
        CyclePhase::Completed => "completed",
        CyclePhase::GatesSkipped => "gates-skipped",
        CyclePhase::Requeued => "requeued",
    }
}

/// Human text for a gate decision.
fn format_decision(repo: &str, decision: &GateDecision, review: &ReviewQueue) -> String {
    match decision {
        GateDecision::Complete => format!("{repo}: ✅ completed — all gates satisfied."),
        GateDecision::SkippedAndFlagged(records) => {
            let labels: Vec<&str> = records.iter().map(|r| r.gate.label()).collect();
            format!(
                "{repo}: gates skipped and flagged for review ({}). backlog: {}",
                labels.join(", "),
                review.pending().len(),
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::fleet::{GatePolicy, GateStatus};

    fn dummy_client() -> OpenCodeClient {
        // Never contacted by these tests (no `work` path exercised against
        // the network); port 1 is a closed socket.
        OpenCodeClient::attach("http://127.0.0.1:1", "test-password").expect("dummy client")
    }

    fn commander() -> FleetCommander {
        let mut c = FleetCommander::new(dummy_client(), vec!["42".to_string()]);
        c.register_repo("alpha", "/tmp/alpha");
        c
    }

    fn admin() -> User {
        User {
            id: "42".to_string(),
            name: "Wyatt".to_string(),
            username: Some("wyatt".to_string()),
            is_admin: true,
        }
    }

    fn rando() -> User {
        User {
            id: "777".to_string(),
            name: "Stranger".to_string(),
            username: None,
            is_admin: false,
        }
    }

    /// Dispatch a cycle for `alpha` as if a `work` had succeeded, minus the
    /// network: enqueue → dispatch → mark running → fake session.
    fn dispatch_running(c: &mut FleetCommander, task: &str) {
        c.state.router.enqueue("alpha", task);
        let active = c.state.registry.active_names();
        c.state.router.dispatch_next(&active).expect("dispatch");
        assert!(c.state.router.mark_running("alpha"));
        c.in_flight.insert(
            "alpha".to_string(),
            InFlight {
                task: task.to_string(),
                session: "ses_fake".to_string(),
            },
        );
    }

    #[test]
    fn parse_readonly_verbs() {
        assert_eq!(
            std::mem::discriminant(&FleetCommander::parse("fleet status").unwrap()),
            std::mem::discriminant(&FleetCmd::Status)
        );
        assert!(matches!(
            FleetCommander::parse("/fleet repos"),
            Some(FleetCmd::Repos)
        ));
        assert!(matches!(
            FleetCommander::parse("/fleet review"),
            Some(FleetCmd::Review)
        ));
        assert!(matches!(
            FleetCommander::parse("fleet"),
            Some(FleetCmd::Help)
        ));
    }

    #[test]
    fn parse_work_collect_decide() {
        let Some(FleetCmd::Work { repo, task }) =
            FleetCommander::parse("/fleet work alpha fix the flaky test")
        else {
            panic!("work should parse");
        };
        assert_eq!(repo, "alpha");
        assert_eq!(task, "fix the flaky test");

        let Some(FleetCmd::Collect {
            repo,
            ok,
            passed,
            reason,
        }) = FleetCommander::parse("fleet collect alpha ok build,tests -- merged to main")
        else {
            panic!("collect ok should parse");
        };
        assert_eq!(repo, "alpha");
        assert!(ok);
        assert_eq!(passed, vec![GateId::Build, GateId::Tests]);
        assert_eq!(reason, "merged to main");

        let Some(FleetCmd::Collect {
            ok: false, reason, ..
        }) = FleetCommander::parse("/fleet collect alpha fail worktree blew up")
        else {
            panic!("collect fail should parse");
        };
        assert_eq!(reason, "worktree blew up");

        assert!(matches!(
            FleetCommander::parse("/fleet approve alpha proofs"),
            Some(FleetCmd::Decide {
                approved: true,
                gate: GateId::Proofs,
                ..
            })
        ));
        assert!(matches!(
            FleetCommander::parse("/fleet deny alpha audit"),
            Some(FleetCmd::Decide {
                approved: false,
                gate: GateId::Audit,
                ..
            })
        ));
    }

    #[test]
    fn parse_bad_args_yield_help_not_none() {
        // Addressed to the fleet → always Some (usage text), never None.
        for line in [
            "fleet work alpha",
            "/fleet collect alpha maybe",
            "fleet collect alpha ok notagate",
            "/fleet approve alpha",
            "/fleet frobnicate",
        ] {
            assert!(
                matches!(FleetCommander::parse(line), Some(FleetCmd::Help)),
                "{line} should fall back to Help"
            );
        }
    }

    #[test]
    fn parse_rejects_non_fleet_lines() {
        assert!(FleetCommander::parse("what is the fleet status of shipping").is_none());
        assert!(FleetCommander::parse("").is_none());
        assert!(FleetCommander::parse("/status").is_none());
    }

    #[tokio::test]
    async fn mutating_verbs_require_admin() {
        let mut c = commander();
        // Non-admin on a mutating verb → Unauthorized.
        let err = c
            .execute(
                FleetCmd::Work {
                    repo: "alpha".into(),
                    task: "x".into(),
                },
                &rando(),
            )
            .await
            .expect_err("non-admin work must be denied");
        assert!(err.to_string().contains("requires an admin"), "{err}");
        // Same sender, readonly verb → allowed.
        let out = c
            .execute(FleetCmd::Status, &rando())
            .await
            .expect("readonly ok");
        assert!(out.contains("alpha"), "{out}");
        // Admin on a mutating verb passes the gate (fails later on lane state, not auth).
        let err = c
            .execute(
                FleetCmd::Collect {
                    repo: "alpha".into(),
                    ok: true,
                    passed: vec![],
                    reason: String::new(),
                },
                &admin(),
            )
            .await
            .expect_err("no in-flight cycle to collect");
        assert!(!err.to_string().contains("requires an admin"), "{err}");
    }

    #[tokio::test]
    async fn work_requires_known_active_repo() {
        let mut c = commander();
        let err = c
            .execute(
                FleetCmd::Work {
                    repo: "ghost".into(),
                    task: "x".into(),
                },
                &admin(),
            )
            .await
            .expect_err("unknown repo");
        assert!(err.to_string().contains("unknown repo"), "{err}");

        assert!(
            c.state.registry.set_active("alpha", false),
            "deactivate alpha"
        );
        let err = c
            .execute(
                FleetCmd::Work {
                    repo: "alpha".into(),
                    task: "x".into(),
                },
                &admin(),
            )
            .await
            .expect_err("inactive repo");
        assert!(err.to_string().contains("inactive"), "{err}");
    }

    #[tokio::test]
    async fn work_on_busy_lane_queues_instead_of_dispatching() {
        let mut c = commander();
        dispatch_running(&mut c, "first task");
        let out = c
            .execute(
                FleetCmd::Work {
                    repo: "alpha".into(),
                    task: "second task".into(),
                },
                &admin(),
            )
            .await
            .expect("queue reply");
        assert!(out.contains("queued"), "{out}");
        assert_eq!(c.state.router.queue_len("alpha"), 1);
    }

    #[test]
    fn collect_fail_requeues_the_in_flight_task() {
        let mut c = commander();
        dispatch_running(&mut c, "the precious task");
        let out = c
            .collect("alpha", false, &[], "worktree blew up")
            .expect("collect fail");
        assert!(out.contains("requeued"), "{out}");
        // The popped task text is preserved by the commander.
        assert_eq!(c.state.router.queue_len("alpha"), 1);
        assert!(!c
            .state
            .router
            .lane("alpha")
            .is_some_and(CyclePhase::occupies_lane));
        assert!(!c.in_flight.contains_key("alpha"));
    }

    #[test]
    fn collect_ok_with_all_gates_completes() {
        let mut c = commander();
        dispatch_running(&mut c, "ship it");
        let out = c
            .collect("alpha", true, &GateId::all(), "everything green")
            .expect("collect ok");
        assert!(out.contains("completed"), "{out}");
        assert!(matches!(
            c.state.router.lane("alpha"),
            Some(CyclePhase::Completed)
        ));
        assert!(c.review.pending().is_empty());
    }

    #[test]
    fn collect_ok_blocking_policy_flags_first_unmet_gate() {
        let mut c = commander();
        dispatch_running(&mut c, "ship it");
        let out = c
            .collect("alpha", true, &[GateId::Build], "build green only")
            .expect("collect ok");
        assert!(out.contains("requeued") || out.contains("flagged"), "{out}");
        // Blocking policy (skip_on_finish=false): unmet gates → lane requeued.
        assert!(!c
            .state
            .router
            .lane("alpha")
            .is_some_and(CyclePhase::occupies_lane));
    }

    #[test]
    fn collect_ok_skip_policy_populates_review() {
        let mut c = commander();
        c.set_gate_policy(GatePolicy {
            hard_gates: GateId::all().to_vec(),
            skip_on_finish: true,
        });
        dispatch_running(&mut c, "ship it");
        let out = c
            .collect("alpha", true, &[GateId::Build], "moved fast")
            .expect("collect ok");
        assert!(out.contains("flagged"), "{out}");
        assert!(matches!(
            c.state.router.lane("alpha"),
            Some(CyclePhase::GatesSkipped)
        ));
        let pending = c.review.pending();
        assert_eq!(pending.len(), 4, "tests/clippy/proofs/audit flagged");
        assert!(pending
            .iter()
            .all(|r| r.status == GateStatus::AwaitingReview));
    }

    #[test]
    fn decide_resolves_review_records() {
        let mut c = commander();
        c.set_gate_policy(GatePolicy {
            hard_gates: GateId::all().to_vec(),
            skip_on_finish: true,
        });
        dispatch_running(&mut c, "ship it");
        c.collect("alpha", true, &[], "all skipped")
            .expect("collect ok");
        assert_eq!(c.review.pending().len(), 5);

        assert!(c
            .decide("alpha", GateId::Tests, true)
            .expect("approve")
            .contains("approved"));
        assert!(c
            .decide("alpha", GateId::Clippy, false)
            .expect("deny")
            .contains("rejected"));
        assert_eq!(c.review.pending().len(), 3);

        let err = c
            .decide("alpha", GateId::Tests, true)
            .expect_err("already resolved");
        assert!(err.to_string().contains("no pending"), "{err}");
    }

    #[test]
    fn persistence_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "clawdius-fleet-cmds-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
        ));
        {
            let mut c =
                FleetCommander::new(dummy_client(), vec!["42".to_string()]).with_state_path(&path);
            c.register_repo("alpha", "/tmp/alpha");
            c.set_gate_policy(GatePolicy {
                hard_gates: GateId::all().to_vec(),
                skip_on_finish: true,
            });
            dispatch_running(&mut c, "ship it");
            c.collect("alpha", true, &[GateId::Build], "moved fast")
                .expect("collect ok");
        }
        let reopened =
            FleetCommander::new(dummy_client(), vec!["42".to_string()]).with_state_path(&path);
        assert!(matches!(
            reopened.state.router.lane("alpha"),
            Some(CyclePhase::GatesSkipped)
        ));
        assert_eq!(reopened.review.pending().len(), 4);
        assert!(reopened.state.registry.get("alpha").is_some());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gate_labels_round_trip() {
        for gate in GateId::all() {
            assert_eq!(parse_gate(gate.label()), Some(gate));
        }
        assert_eq!(parse_gate("nope"), None);
    }
}
