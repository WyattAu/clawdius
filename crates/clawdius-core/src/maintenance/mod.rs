//! 24/7 maintenance loop: the durable backlog and the cycle ledger.
//!
//! The loop's whole state lives in two files so it survives restarts,
//! crashes and context loss:
//!
//! ```text
//! .clawdius/maintenance/backlog.json   tasks + statuses (mutable state)
//! .clawdius/maintenance/ledger.jsonl   append-only record of every cycle
//! ```
//!
//! A cycle picks the highest-priority pending task, runs it in an
//! isolated worktree through the dev pipeline (LLM stages + deterministic
//! gates), and records what happened. Green work becomes a branch + PR;
//! red work is discarded. Nothing here touches the main working tree.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What kind of maintenance a task is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskKind {
    /// Clear lint/format debt until the gates are green.
    Lint,
    /// Upgrade patch/minor dependencies that still pass the gates.
    Deps,
    /// Close documentation gaps (stale docs, missing sections).
    Docs,
    /// Add tests for uncovered public surface.
    Tests,
    /// Fix a specific reported issue.
    Issue,
    /// Anything else, driven by the task's own goal text.
    Custom,
}

impl TaskKind {
    /// Stable slug (used in branch names and journal ids).
    #[must_use]
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Lint => "lint",
            Self::Deps => "deps",
            Self::Docs => "docs",
            Self::Tests => "tests",
            Self::Issue => "issue",
            Self::Custom => "custom",
        }
    }

    /// The standing goal for this kind, used when a task omits one.
    #[must_use]
    pub fn default_goal(&self) -> &'static str {
        match self {
            Self::Lint => {
                "Make the deterministic gates green: cargo fmt, clippy with -D warnings, \
                 and the test suite. Fix the underlying causes; do not weaken lints, delete \
                 tests, or add blanket allows without a stated reason. The pipeline runs \
                 the full gates after your edits, so do not run the whole workspace test \
                 suite yourself."
            },
            Self::Deps => {
                "Upgrade dependencies that have newer patch or minor releases, one at a time, \
                 keeping the gates green. Do not cross major versions. If an upgrade breaks \
                 the build, revert that upgrade and record why."
            },
            Self::Docs => {
                "Bring the documentation in line with the code: fix stale commands, missing \
                 flags, and broken cross-references. Prefer small accurate edits over rewrites."
            },
            Self::Tests => {
                "Add focused tests for untested public behaviour. Every new test must fail \
                 before the change and pass after; no assertion-free tests."
            },
            Self::Issue => "Fix the reported issue at its root cause and add a regression test.",
            Self::Custom => "Complete the task as described, ending with the gates green.",
        }
    }
}

/// Where a task stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Waiting for a cycle.
    Pending,
    /// A cycle is running it right now.
    Running,
    /// Gates went green; the work was committed to a branch.
    Done,
    /// The cycle failed; see the ledger for the reason.
    Failed,
    /// Deliberately not run (superseded, duplicate, out of scope).
    Skipped,
}

/// One maintenance task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Stable id, e.g. `lint-2026-11-13`.
    pub id: String,
    pub kind: TaskKind,
    /// Goal text; falls back to the kind's standing goal when empty.
    #[serde(default)]
    pub goal: String,
    /// Higher runs first; equal priorities run in file order.
    #[serde(default)]
    pub priority: u8,
    pub status: TaskStatus,
}

impl Task {
    /// A new pending task with the kind's standing goal.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: TaskKind, priority: u8) -> Self {
        Self {
            id: id.into(),
            kind,
            goal: kind.default_goal().to_string(),
            priority,
            status: TaskStatus::Pending,
        }
    }

    /// The goal to hand the engine (never empty).
    #[must_use]
    pub fn effective_goal(&self) -> &str {
        if self.goal.trim().is_empty() {
            self.kind.default_goal()
        } else {
            &self.goal
        }
    }
}

/// The durable task queue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Backlog {
    #[serde(default)]
    pub tasks: Vec<Task>,
}

impl Backlog {
    /// Load from `path`; a missing file is an empty backlog, not an error.
    ///
    /// # Errors
    ///
    /// Propagates read and JSON-parse failures (a corrupt backlog must be
    /// visible, never silently replaced).
    pub fn load(path: &Path) -> crate::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                crate::Error::Config(format!("backlog {} is corrupt: {e}", path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(crate::Error::Config(format!(
                "cannot read backlog {}: {e}",
                path.display()
            ))),
        }
    }

    /// Persist atomically (write temp, rename) so a crash cannot truncate.
    ///
    /// # Errors
    ///
    /// Propagates serialization and filesystem failures.
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::Error::Config(format!("cannot create {}: {e}", parent.display()))
            })?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::Error::Config(format!("cannot serialize backlog: {e}")))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .map_err(|e| crate::Error::Config(format!("cannot write {}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| crate::Error::Config(format!("cannot replace {}: {e}", path.display())))
    }

    /// Highest-priority pending task (ties broken by file order).
    #[must_use]
    pub fn next_pending(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Pending)
            .max_by_key(|(i, t)| (t.priority, std::cmp::Reverse(*i)))
            .map(|(_, t)| t)
    }

    /// Set a task's status by id; returns whether the id existed.
    pub fn set_status(&mut self, id: &str, status: TaskStatus) -> bool {
        match self.tasks.iter_mut().find(|t| t.id == id) {
            Some(task) => {
                task.status = status;
                true
            },
            None => false,
        }
    }

    /// Reset every `running` task back to `pending` and return their ids.
    ///
    /// A crash mid-cycle leaves its task marked `running` forever, and
    /// `next_pending` skips it — the queue silently starves (live-caught:
    /// one killed cycle left the backlog dry for every later cycle).
    /// The runner calls this at cycle start, when any `running` task is
    /// by definition from a dead instance.
    pub fn reset_stale_running(&mut self) -> Vec<String> {
        let mut recovered = Vec::new();
        for task in &mut self.tasks {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Pending;
                recovered.push(task.id.clone());
            }
        }
        recovered
    }

    /// Append a task (ids must be unique).
    ///
    /// # Errors
    ///
    /// Returns an error when the id already exists.
    pub fn push(&mut self, task: Task) -> crate::Result<()> {
        if self.tasks.iter().any(|t| t.id == task.id) {
            return Err(crate::Error::Config(format!(
                "duplicate task id '{}'",
                task.id
            )));
        }
        self.tasks.push(task);
        Ok(())
    }

    /// Counts per status, for the status line.
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut pending = 0;
        let mut done = 0;
        let mut failed = 0;
        let mut other = 0;
        for task in &self.tasks {
            match task.status {
                TaskStatus::Pending => pending += 1,
                TaskStatus::Done => done += 1,
                TaskStatus::Failed => failed += 1,
                TaskStatus::Running | TaskStatus::Skipped => other += 1,
            }
        }
        (pending, done, failed, other)
    }
}

/// How a cycle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CycleOutcome {
    /// Gates green, work committed to a branch.
    Green,
    /// Gates still red after the loop budget and escalation.
    Red,
    /// No pending task.
    Idle,
    /// The cycle could not run (provider/quota/tooling failure).
    Error,
}

/// One append-only ledger line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleRecord {
    /// RFC 3339 UTC timestamp.
    pub at: String,
    pub task_id: String,
    pub kind: TaskKind,
    pub outcome: CycleOutcome,
    pub provider: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub duration_ms: u64,
}

/// The append-only cycle ledger.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Open (or create on first append) a ledger at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Append one record as a JSON line.
    ///
    /// # Errors
    ///
    /// Propagates serialization and filesystem failures.
    pub fn append(&self, record: &CycleRecord) -> crate::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::Error::Config(format!("cannot create {}: {e}", parent.display()))
            })?;
        }
        let mut line = serde_json::to_string(record)
            .map_err(|e| crate::Error::Config(format!("cannot serialize cycle: {e}")))?;
        line.push('\n');
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| {
                crate::Error::Config(format!("cannot open {}: {e}", self.path.display()))
            })?;
        file.write_all(line.as_bytes()).map_err(|e| {
            crate::Error::Config(format!("cannot append {}: {e}", self.path.display()))
        })
    }

    /// Read every record (skipping unparseable lines, which are reported
    /// as a count so corruption is visible without losing the rest).
    ///
    /// # Errors
    ///
    /// Propagates read failures.
    pub fn read(&self) -> crate::Result<(Vec<CycleRecord>, usize)> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(e) => {
                return Err(crate::Error::Config(format!(
                    "cannot read {}: {e}",
                    self.path.display()
                )))
            },
        };
        let mut records = Vec::new();
        let mut skipped = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<CycleRecord>(line) {
                Ok(r) => records.push(r),
                Err(_) => skipped += 1,
            }
        }
        Ok((records, skipped))
    }

    /// Path this ledger writes to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Current UTC timestamp in RFC 3339 form.
#[must_use]
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, kind: TaskKind, priority: u8) -> Task {
        Task::new(id, kind, priority)
    }

    #[test]
    fn backlog_selects_highest_priority_then_file_order() {
        let mut b = Backlog::default();
        b.push(task("low", TaskKind::Docs, 1)).unwrap();
        b.push(task("high", TaskKind::Lint, 9)).unwrap();
        b.push(task("mid-a", TaskKind::Tests, 5)).unwrap();
        b.push(task("mid-b", TaskKind::Deps, 5)).unwrap();

        assert_eq!(b.next_pending().map(|t| t.id.as_str()), Some("high"));

        b.set_status("high", TaskStatus::Done);
        assert_eq!(b.next_pending().map(|t| t.id.as_str()), Some("mid-a"));

        b.set_status("mid-a", TaskStatus::Failed);
        assert_eq!(b.next_pending().map(|t| t.id.as_str()), Some("mid-b"));

        b.set_status("mid-b", TaskStatus::Skipped);
        assert_eq!(b.next_pending().map(|t| t.id.as_str()), Some("low"));

        b.set_status("low", TaskStatus::Done);
        assert!(b.next_pending().is_none());
    }

    #[test]
    fn backlog_roundtrips_through_disk_and_rejects_duplicate_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/backlog.json");

        let mut b = Backlog::default();
        b.push(task("a", TaskKind::Lint, 1)).unwrap();
        b.push(task("b", TaskKind::Issue, 2)).unwrap();
        b.set_status("a", TaskStatus::Done);
        b.save(&path).unwrap();

        let loaded = Backlog::load(&path).unwrap();
        assert_eq!(loaded.tasks.len(), 2);
        assert_eq!(loaded.tasks[0].status, TaskStatus::Done);
        assert_eq!(loaded.next_pending().map(|t| t.id.as_str()), Some("b"));

        let mut dup = loaded;
        assert!(dup.push(task("a", TaskKind::Docs, 0)).is_err());
    }

    #[test]
    fn missing_backlog_is_empty_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let b = Backlog::load(&dir.path().join("absent.json")).unwrap();
        assert!(b.tasks.is_empty());
        assert_eq!(b.counts(), (0, 0, 0, 0));
    }

    #[test]
    fn corrupt_backlog_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(Backlog::load(&path).is_err());
    }

    #[test]
    fn ledger_appends_and_reads_records() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(dir.path().join("ledger.jsonl"));
        let rec = CycleRecord {
            at: now_rfc3339(),
            task_id: "t1".into(),
            kind: TaskKind::Lint,
            outcome: CycleOutcome::Green,
            provider: "google".into(),
            branch: Some("maint/t1".into()),
            pr_url: None,
            note: "gates green".into(),
            duration_ms: 1234,
        };
        ledger.append(&rec).unwrap();
        ledger.append(&rec).unwrap();

        let (records, skipped) = ledger.read().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(skipped, 0);
        assert_eq!(records[0].task_id, "t1");
        assert_eq!(records[1].outcome, CycleOutcome::Green);
    }

    #[test]
    fn ledger_reports_corrupt_lines_without_losing_good_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(&path, "{\"garbage\":true}\n").unwrap();
        let ledger = Ledger::new(&path);
        let (records, skipped) = ledger.read().unwrap();
        assert!(records.is_empty());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn effective_goal_falls_back_to_the_kind_default() {
        let mut t = task("x", TaskKind::Deps, 0);
        t.goal.clear();
        assert!(t.effective_goal().contains("Upgrade dependencies"));

        let mut t = task("y", TaskKind::Custom, 0);
        t.goal = "  do the thing  ".into();
        assert_eq!(t.effective_goal(), "  do the thing  ");
    }

    #[test]
    fn every_kind_has_a_slug_and_a_standing_goal() {
        for kind in [
            TaskKind::Lint,
            TaskKind::Deps,
            TaskKind::Docs,
            TaskKind::Tests,
            TaskKind::Issue,
            TaskKind::Custom,
        ] {
            assert!(!kind.slug().is_empty());
            assert!(kind.default_goal().len() > 20);
        }
    }

    #[test]
    fn stale_running_tasks_recover_to_pending() {
        let mut b = Backlog::default();
        b.push(task("stuck", TaskKind::Lint, 9)).unwrap();
        b.push(task("fresh", TaskKind::Docs, 1)).unwrap();
        b.set_status("stuck", TaskStatus::Running);

        let recovered = b.reset_stale_running();
        assert_eq!(recovered, vec!["stuck".to_string()]);
        assert_eq!(
            b.next_pending().map(|t| t.id.as_str()),
            Some("stuck"),
            "a recovered task must be selectable again"
        );
        // Idempotent: a clean backlog recovers nothing.
        assert!(b.reset_stale_running().is_empty());
    }

    #[test]
    fn counts_track_every_status() {
        let mut b = Backlog::default();
        for (i, status) in [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Done,
            TaskStatus::Failed,
            TaskStatus::Skipped,
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = task(&format!("t{i}"), TaskKind::Custom, 0);
            t.status = status;
            b.push(t).unwrap();
        }
        assert_eq!(b.counts(), (1, 1, 1, 2));
    }
}
