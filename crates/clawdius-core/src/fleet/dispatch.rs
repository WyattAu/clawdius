//! Fleet executor: couples the [`FleetRouter`] task queues with the
//! [`OpencodeDriver`] process transport under a fleet-wide concurrency
//! cap.
//!
//! Flow:
//!
//! ```text
//! enqueue(repo, task) ──► FleetRouter queues
//! dispatch_pending()  ──► router.dispatch_next() per free slot
//!                         └─ thread: driver.run(FleetTask) ─┐
//! collect_ready()/join() ◄── mpsc DriverReport outcomes ───┘
//! caller                ──► router.report_finish/failure
//! ```
//!
//! The executor owns *dispatch and collection*; it deliberately does
//! NOT drive `report_finish`/`report_failure` — gate evaluation is the
//! caller's (central agent's) decision once it sees the outcomes.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use super::opencode_driver::{FleetTask, OpencodeDriver};
use tracing::warn;

use super::gates::{GateDecision, GateId, GatePolicy, ReviewQueue};
use super::router::{DispatchDecision, FleetRouter};

/// One finished (or failed-to-run) task handed back to the caller.
#[derive(Debug, Clone)]
pub enum TaskOutcome {
    /// The driver ran the task to completion (opencode may still have
    /// exited non-zero — check `report.ok`).
    Done(crate::fleet::opencode_driver::DriverReport),
    /// The driver itself failed (spawn error, timeout) — task slug +
    /// error text.
    Failed(String, String),
}

/// Routes queued tasks to the opencode CLI driver under a cap.
pub struct FleetExecutor {
    /// Task queues + lane state (also driven by the caller for gates).
    pub router: FleetRouter,
    /// Shared driver (worktree pool + bin config).
    driver: Arc<OpencodeDriver>,
    /// Fleet-wide cap on concurrent opencode runs.
    pub max_parallel: usize,
    /// Gates the fleet requires of a run. Exit-0 convention: the agent
    /// was instructed to run these and a zero exit reports them passed.
    pub required_gates: Vec<GateId>,
    /// Tasks currently dispatched but not yet collected.
    inflight: usize,
    /// Outcomes that arrived but were not yet returned to the caller.
    pending: VecDeque<TaskOutcome>,
    /// Worker channel: sender cloned into workers, receiver here.
    tx: Sender<TaskOutcome>,
    rx: Receiver<TaskOutcome>,
}

impl FleetExecutor {
    /// Create an executor over `driver` with a concurrency cap
    /// (floored at 1).
    #[must_use]
    pub fn new(router: FleetRouter, driver: Arc<OpencodeDriver>, max_parallel: usize) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            router,
            driver,
            max_parallel: max_parallel.max(1),
            required_gates: GateId::all().to_vec(),
            inflight: 0,
            pending: VecDeque::new(),
            tx,
            rx,
        }
    }

    /// Enqueue a task for a repo (delegates to the router).
    pub fn enqueue(&mut self, repo: &str, prompt: &str) {
        self.router.enqueue(repo, prompt);
    }

    /// Dispatch queued work while both the fleet cap and router lanes
    /// allow. Each dispatch runs on its own thread. Returns how many
    /// tasks were dispatched.
    ///
    /// `active_repos` is passed straight to
    /// [`FleetRouter::dispatch_next`].
    pub fn dispatch_pending(&mut self, active_repos: &[String]) -> usize {
        let mut dispatched = 0;
        while self.inflight < self.max_parallel {
            let Some(DispatchDecision { repo, task }) = self.router.dispatch_next(active_repos)
            else {
                break;
            };
            self.router.mark_running(&repo);
            let driver = Arc::clone(&self.driver);
            let tx = self.tx.clone();
            let slug = repo.clone();
            std::thread::spawn(move || {
                let fleet_task = FleetTask {
                    slug: slug.clone(),
                    prompt: task,
                    agent: None,
                    model: None,
                    session: None,
                    timeout: Duration::ZERO, // driver default
                };
                let outcome = match driver.run(&fleet_task) {
                    Ok(report) => TaskOutcome::Done(report),
                    Err(e) => TaskOutcome::Failed(slug, e.to_string()),
                };
                let _ = tx.send(outcome);
            });
            self.inflight += 1;
            dispatched += 1;
        }
        dispatched
    }

    /// Poll finished runs without blocking. Returns every outcome that
    /// has arrived so far.
    pub fn collect_ready(&mut self) -> Vec<TaskOutcome> {
        self.drain_rx(false)
    }

    /// Block until every in-flight task reports, then return all
    /// outcomes (arrived-early ones included).
    pub fn join(&mut self) -> Vec<TaskOutcome> {
        self.drain_rx(true)
    }

    /// Number of tasks dispatched but not yet collected.
    #[must_use]
    pub fn inflight(&self) -> usize {
        self.inflight
    }

    /// Walk collected outcomes and drive the router lanes + gates.
    ///
    /// Convention: an opencode exit of 0 reports the required gates as
    /// passed (the agent's prompt instructs it to run verification and
    /// state DONE); a non-zero exit or a driver failure requeues the
    /// repo's lane. Gate decisions are returned so the caller can act
    /// on them (review queue contents, notifications).
    pub fn apply_outcomes(
        &mut self,
        policy: &GatePolicy,
        review: &mut ReviewQueue,
    ) -> Vec<(String, GateDecision)> {
        let mut decisions = Vec::new();
        for outcome in self.collect_ready() {
            match outcome {
                TaskOutcome::Done(report) => {
                    let repo = report.slug.clone();
                    if report.ok {
                        if let Some(decision) = self.router.report_finish(
                            &repo,
                            &self.required_gates,
                            "opencode run exited 0 (agent ran verification)",
                            policy,
                            review,
                        ) {
                            decisions.push((repo, decision));
                        }
                    } else {
                        // Execution failure: the lane requeues silently —
                        // a failed run is not a gate finish.
                        self.router.report_failure(&repo);
                    }
                }
                TaskOutcome::Failed(slug, err) => {
                    self.router.report_failure(&slug);
                    warn!("fleet task `{slug}` failed to run: {err}");
                }
            }
        }
        decisions
    }


    /// Drain the channel; when `wait` is set, blocks until `inflight`
    /// reaches zero first.
    fn drain_rx(&mut self, wait: bool) -> Vec<TaskOutcome> {
        if wait {
            while self.inflight > 0 {
                match self.rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(outcome) => {
                        self.inflight = self.inflight.saturating_sub(1);
                        self.pending.push_back(outcome);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }
        loop {
            match self.rx.try_recv() {
                Ok(outcome) => {
                    self.inflight = self.inflight.saturating_sub(1);
                    self.pending.push_back(outcome);
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        self.pending.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    /// A fake `opencode` script that prints a canned JSON payload.
    fn fake_opencode(dir: &tempfile::TempDir, payload: &str, extra: &str) -> PathBuf {
        let script = dir.path().join(format!(
            "fake-{}.sh",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nl = '\n';
        let body = format!(
            "#!/bin/sh{nl}pwd >> {p}{nl}{extra}{nl}echo '{payload}'{nl}",
            p = "/dev/null"
        );
        std::fs::write(&script, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }

    fn temp_git_repo(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        let repo = dir.path().join(name);
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .current_dir(&repo)
                .args(args)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(repo.join("f.txt"), "x").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);
        repo
    }

    fn executor_for(dir: &tempfile::TempDir, repos: &[&str], cap: usize) -> FleetExecutor {
        let repo = temp_git_repo(dir, "repo");
        let fake = fake_opencode(dir, r#"{"sessionID":"ses_x"}"#, "");
        let worktree_root = repo.join(".fleet-worktrees");
        let driver = Arc::new(OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root,
            default_timeout: Duration::from_secs(30),
        });
        let mut ex = FleetExecutor::new(FleetRouter::new(), driver, cap);
        for r in repos {
            ex.enqueue(r, "do the thing");
        }
        ex
    }

    #[test]
    fn dispatches_up_to_cap_and_collects_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut ex = executor_for(&dir, &["alpha", "beta", "gamma"], 2);
        let active: Vec<String> = vec!["alpha".into(), "beta".into(), "gamma".into()];
        assert_eq!(ex.dispatch_pending(&active), 2, "cap 2 -> 2 dispatched");
        assert_eq!(ex.inflight(), 2);
        // The third repo stays queued until a slot frees.
        assert_eq!(ex.router.queue_len("gamma"), 1);
        let outcomes = ex.join();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .all(|o| matches!(o, TaskOutcome::Done(r) if r.ok)));
        // Slot freed: dispatch the rest.
        assert_eq!(ex.dispatch_pending(&active), 1);
        let outcomes = ex.join();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(ex.inflight(), 0);
    }

    #[test]
    fn lane_discipline_one_task_per_repo() {
        let dir = tempfile::tempdir().unwrap();
        let mut ex = executor_for(&dir, &["solo"], 4);
        ex.enqueue("solo", "second task");
        let active = vec!["solo".to_string()];
        // Cap 4 but the lane is busy after the first dispatch: exactly 1.
        assert_eq!(ex.dispatch_pending(&active), 1);
        assert_eq!(ex.dispatch_pending(&active), 0, "lane must not double-run");
        ex.join();
        // After collection the caller drives finish/failure; simulate a
        // failure requeue, then the lane frees and the queued task runs.
        assert!(ex.router.report_failure("solo"));
        assert_eq!(ex.dispatch_pending(&active), 1);
        assert_eq!(ex.join().len(), 1);
    }

    #[test]
    fn driver_failure_surfaces_as_failed_outcome() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir, "repo");
        let mut ex = FleetExecutor::new(
            FleetRouter::new(),
            Arc::new(OpencodeDriver {
                bin: PathBuf::from("/nonexistent/opencode-missing"),
                repo: repo.clone(),
                worktree_root: repo.join(".fleet-worktrees"),
                default_timeout: Duration::from_secs(5),
            }),
            2,
        );
        ex.enqueue("r1", "t");
        assert_eq!(ex.dispatch_pending(&["r1".to_string()]), 1);
        let outcomes = ex.join();
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0], TaskOutcome::Failed(_, _)));
    }
}
