//! opencode CLI driver: spawn-and-continue headless runs with
//! worktree-per-task isolation.
//!
//! This is the fleet's *process* transport for opencode — one
//! `opencode run` per task, optionally continuing an existing session,
//! isolated in a dedicated git worktree so parallel tasks never share
//! a checkout. The sibling [`crate::fleet`] server transport (the
//! `opencode_client` lane) attaches to *already running* `opencode
//! serve` processes instead; the two are complementary:
//!
//! ```text
//! CLI driver (here)   spawn per task · worktree isolation · sessions persist
//! server client       attach to serve · event stream · session dashboards
//! ```
//!
//! Everything is deterministic and unit-tested against a fake
//! `opencode` script — no network, no real binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// One fleet task for the CLI driver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetTask {
    /// Stable task slug — names the worktree (`fleet/<slug>`).
    pub slug: String,
    /// The prompt handed to opencode.
    pub prompt: String,
    /// opencode agent (`build`, `plan`, …). `None` = opencode default.
    pub agent: Option<String>,
    /// Model override (`provider/model`). `None` = opencode default.
    pub model: Option<String>,
    /// Existing session to continue; `None` = fresh session.
    pub session: Option<String>,
    /// Hard wall-clock cap for the run.
    pub timeout: Duration,
}

/// What one driver run produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverReport {
    /// Task slug this report belongs to.
    pub slug: String,
    /// Whether opencode exited 0.
    pub ok: bool,
    /// Full stdout of the run.
    pub output: String,
    /// Session id (`--format json`), when captured — feeds
    /// `FleetTask::session` to continue exactly here.
    pub session_id: Option<String>,
    /// Wall-clock duration.
    pub duration_ms: u64,
    /// Worktree the task ran in (when the task requested isolation).
    pub worktree: Option<PathBuf>,
}

/// Outcome of collecting a finished task branch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectReport {
    /// Task slug that was collected.
    pub slug: String,
    /// Branch that was merged (`fleet/<slug>`).
    pub branch: String,
    /// Repo HEAD before the merge.
    pub head_before: String,
    /// Repo HEAD after the merge.
    pub head_after: String,
}

/// Driver for headless `opencode run` tasks with worktree isolation.
pub struct OpencodeDriver {
    /// opencode binary (env `OPENCODE_BIN` overrides).
    pub bin: PathBuf,
    /// Repository root — parent of the worktree pool.
    pub repo: PathBuf,
    /// Directory holding per-task worktrees (default `<repo>/.fleet-worktrees`).
    pub worktree_root: PathBuf,
    /// Default timeout when a task doesn't set one.
    pub default_timeout: Duration,
}

impl OpencodeDriver {
    /// Create a driver for `repo`.
    #[must_use]
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        let repo = repo.into();
        Self {
            bin: std::env::var_os("OPENCODE_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("opencode")),
            worktree_root: repo.join(".fleet-worktrees"),
            repo,
            default_timeout: Duration::from_secs(15 * 60),
        }
    }

    /// Ensure the task's worktree exists and return its path.
    ///
    /// Creates `<worktree_root>/<slug>` on branch `fleet/<slug>` from
    /// HEAD on first use; reuses the existing worktree afterwards so
    /// follow-up tasks on the same slug continue in place.
    ///
    /// # Errors
    ///
    /// Fails when git itself fails (bad repo, refusal to create the
    /// branch, …) — the git stderr is carried in the error text.
    pub fn prepare_worktree(&self, slug: &str) -> Result<PathBuf, crate::Error> {
        let path = self.worktree_root.join(slug);
        if path.is_dir() {
            return Ok(path);
        }
        std::fs::create_dir_all(&self.worktree_root)
            .map_err(|e| crate::Error::Llm(format!("worktree root create failed: {e}")))?;
        let branch = format!("fleet/{slug}");
        let out = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "add"])
            .arg(&path)
            .arg("-b")
            .arg(&branch)
            .output()
            .map_err(|e| crate::Error::Llm(format!("git worktree spawn failed: {e}")))?;
        if !out.status.success() {
            // Branch may already exist from an earlier pool — retry
            // without -b (attaches the existing branch).
            let retry = Command::new("git")
                .current_dir(&self.repo)
                .args(["worktree", "add"])
                .arg(&path)
                .arg(&branch)
                .output()
                .map_err(|e| crate::Error::Llm(format!("git worktree spawn failed: {e}")))?;
            if !retry.status.success() {
                return Err(crate::Error::Llm(format!(
                    "git worktree add failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
        }
        Ok(path)
    }

    /// Run one fleet task: optionally prepare its worktree, spawn
    /// `opencode run`, enforce the timeout, and capture the report.
    ///
    /// # Errors
    ///
    /// Spawn/IO failures, git worktree failures, and the timeout
    /// surface as [`crate::Error::Llm`]. A non-zero opencode exit is
    /// *not* an error — it is `DriverReport { ok: false, .. }`.
    pub fn run(&self, task: &FleetTask) -> Result<DriverReport, crate::Error> {
        let timeout = if task.timeout == Duration::ZERO {
            self.default_timeout
        } else {
            task.timeout
        };
        let worktree = self.prepare_worktree(&task.slug)?;
        let report = self.run_in(
            &task.slug,
            &worktree,
            &task.prompt,
            task.session.as_deref(),
            task.agent.as_deref(),
            task.model.as_deref(),
            timeout,
        )?;
        Ok(DriverReport {
            worktree: Some(worktree),
            ..report
        })
    }

    /// Continue an existing session in a fixed working directory
    /// (no worktree management) — the follow-up path for reports whose
    /// session was captured elsewhere.
    ///
    /// # Errors
    ///
    /// Same as [`Self::run`].
    pub fn continue_session(
        &self,
        slug: &str,
        session: &str,
        workdir: &Path,
        prompt: &str,
        timeout: Duration,
    ) -> Result<DriverReport, crate::Error> {
        self.run_in(slug, workdir, prompt, Some(session), None, None, timeout)
    }

    /// Core spawn: `opencode run --archived --format json [-s session]
    /// [--agent a] [--model m] <prompt>` with a hard timeout.
    #[allow(clippy::too_many_arguments)]
    fn run_in(
        &self,
        slug: &str,
        workdir: &Path,
        prompt: &str,
        session: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        timeout: Duration,
    ) -> Result<DriverReport, crate::Error> {
        let start = Instant::now();
        let mut cmd = Command::new(&self.bin);
        cmd.current_dir(workdir)
            .arg("run")
            .arg("--archived")
            .arg("--format")
            .arg("json")
            .stdin(Stdio::null());
        if let Some(session) = session {
            cmd.args(["-s", session]);
        }
        if let Some(agent) = agent {
            cmd.args(["--agent", agent]);
        }
        if let Some(model) = model {
            cmd.args(["--model", model]);
        }
        cmd.arg(prompt);

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| crate::Error::Llm(format!("opencode spawn failed: {e}")))?;

        // Poll with a deadline: std has no blocking-wait timeout, and
        // the escalator's timeout field is decorative — this one is
        // enforced. On expiry the child is killed (its worktree stays
        // for inspection).
        let deadline = Instant::now() + timeout;
        let exited;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exited = Some(status);
                    break;
                },
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(crate::Error::Llm(format!(
                        "opencode task `{slug}` exceeded timeout of {timeout:?}"
                    )));
                },
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(crate::Error::Llm(format!("opencode wait failed: {e}"))),
            }
        }
        let out = child
            .wait_with_output()
            .map_err(|e| crate::Error::Llm(format!("opencode wait failed: {e}")))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let session_id = extract_session_id(&stdout);
        Ok(DriverReport {
            slug: slug.to_string(),
            ok: exited.is_some_and(|s| s.success()),
            output: stdout,
            session_id,
            duration_ms: start.elapsed().as_millis() as u64,
            worktree: None,
        })
    }

    /// Collect a finished task: fast-forward the main repo's current
    /// branch to `fleet/<slug>`.
    ///
    /// Preconditions (each surfaces as an error when violated):
    /// - the main checkout is clean (no uncommitted changes),
    /// - the merge is a fast-forward; a diverged target needs a rebase
    ///   of the worktree branch first (deliberate: no machine-authored
    ///   merge-commit resolutions).
    ///
    /// # Errors
    ///
    /// Git failures, dirty checkouts, missing branches, and
    /// non-fast-forward targets surface as [`crate::Error::Llm`].
    pub fn collect(&self, slug: &str) -> Result<CollectReport, crate::Error> {
        let branch = format!("fleet/{slug}");
        let git = |args: &[&str]| -> Result<std::process::Output, crate::Error> {
            Command::new("git")
                .current_dir(&self.repo)
                .args(args)
                .output()
                .map_err(|e| crate::Error::Llm(format!("git spawn failed: {e}")))
        };
        // Main checkout must be clean of MODIFIED tracked files (the
        // worktree pool dir is untracked and never blocks a merge).
        let status = git(&["status", "--porcelain", "--untracked-files=no"])?;
        if !status.stdout.is_empty() {
            return Err(crate::Error::Llm(
                "main checkout has uncommitted changes — commit or stash before collect".into(),
            ));
        }
        // Branch must exist.
        if !git(&[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])?
        .status
        .success()
        {
            return Err(crate::Error::Llm(format!(
                "branch `{branch}` does not exist"
            )));
        }
        let head_before = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"])?.stdout)
            .trim()
            .to_string();
        let merge = git(&["merge", "--ff-only", &branch])?;
        if !merge.status.success() {
            return Err(crate::Error::Llm(format!(
                "target diverged from `{branch}` — rebase the worktree branch and retry: {}",
                String::from_utf8_lossy(&merge.stderr).trim()
            )));
        }
        let head_after = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"])?.stdout)
            .trim()
            .to_string();
        Ok(CollectReport {
            slug: slug.to_string(),
            branch,
            head_before,
            head_after,
        })
    }

    /// Remove a task's worktree and branch (post-collect cleanup, or
    /// abandonment). Force flags are used because worktrees accumulate
    /// untracked build artifacts.
    ///
    /// # Errors
    ///
    /// The branch delete failing surfaces as [`crate::Error::Llm`];
    /// a missing worktree is tolerated (idempotent abandonment).
    pub fn discard(&self, slug: &str) -> Result<(), crate::Error> {
        let path = self.worktree_root.join(slug);
        let branch = format!("fleet/{slug}");
        let _ = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "remove", "--force"])
            .arg(&path)
            .output();
        let out = Command::new("git")
            .current_dir(&self.repo)
            .args(["branch", "-D", &branch])
            .output()
            .map_err(|e| crate::Error::Llm(format!("git spawn failed: {e}")))?;
        if !out.status.success() {
            return Err(crate::Error::Llm(format!(
                "branch delete failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }
}

/// Pull the opencode session id out of a `--format json` stdout.
///
/// Handles both `sessionID` (opencode v1+) and `sessionId` spellings.
#[must_use]
pub fn extract_session_id(stdout: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(stdout).ok()?;
    v.get("sessionID")
        .or_else(|| v.get("sessionId"))
        .and_then(|s| s.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a fake `opencode` script that records its argv + cwd and
    /// prints a canned JSON payload; returns the script path and the
    /// recording file path.
    fn fake_opencode(dir: &tempfile::TempDir, payload: &str, extra: &str) -> (PathBuf, PathBuf) {
        let record = dir.path().join("argv.json");
        let record_arg = record.display().to_string();
        let script = dir.path().join("fake-opencode.sh");
        let nl = '\n';
        let body = format!(
            "#!/bin/sh{nl}pwd > {record_arg}{nl}printf '%s{nl}' \"$@\" >> {record_arg}{nl}{extra}{nl}echo '{payload}'{nl}"
        );
        std::fs::write(&script, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        (script, record)
    }

    fn temp_git_repo(dir: &tempfile::TempDir) -> PathBuf {
        let repo = dir.path().join("repo");
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

    fn task(slug: &str, prompt: &str) -> FleetTask {
        FleetTask {
            slug: slug.to_string(),
            prompt: prompt.to_string(),
            agent: Some("build".into()),
            model: None,
            session: None,
            timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn driver_runs_in_worktree_and_captures_session() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let (fake, record) = fake_opencode(&dir, r#"{"sessionID":"ses_abc"}"#, "");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(10),
        };
        let report = driver.run(&task("fix-tests", "fix it")).unwrap();
        assert!(report.ok);
        assert_eq!(report.session_id.as_deref(), Some("ses_abc"));
        let wt = report.worktree.expect("worktree recorded");
        assert!(wt.is_dir());
        assert!(wt.ends_with(".fleet-worktrees/fix-tests"));

        // The run happened INSIDE the worktree, with the right flags.
        let argv = std::fs::read_to_string(&record).unwrap();
        assert!(
            argv.contains("fix-tests"),
            "pwd line should show the worktree path"
        );
        assert!(argv.contains("--archived"));
        assert!(argv.contains("--format"));
        assert!(argv.contains("fix it"));
        // Branch `fleet/fix-tests` exists in the repo.
        let branches = Command::new("git")
            .current_dir(&repo)
            .args(["branch", "--list", "fleet/fix-tests"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&branches.stdout).contains("fleet/fix-tests"));
    }

    #[test]
    fn continuation_passes_session_flag() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let (fake, record) = fake_opencode(&dir, r#"{"sessionID":"ses_next"}"#, "");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(10),
        };
        let workdir = driver.prepare_worktree("cont").unwrap();
        let report = driver
            .continue_session(
                "cont",
                "ses_abc",
                &workdir,
                "next step",
                Duration::from_secs(10),
            )
            .unwrap();
        assert!(report.ok);
        assert_eq!(report.session_id.as_deref(), Some("ses_next"));
        let argv = std::fs::read_to_string(&record).unwrap();
        assert!(argv.contains("ses_abc"), "continuation must pass -s <id>");
        assert!(argv.contains("next step"));
    }

    #[test]
    fn worktree_prepare_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let driver = OpencodeDriver::new(&repo);
        let p1 = driver.prepare_worktree("slug-a").unwrap();
        let p2 = driver.prepare_worktree("slug-a").unwrap();
        assert_eq!(p1, p2);
        assert!(p1.is_dir());
    }

    #[test]
    fn timeout_is_enforced_and_child_killed() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        // A fake that sleeps far past the deadline.
        let (fake, _record) = fake_opencode(&dir, "{}", "sleep 30");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(30),
        };
        let mut t = task("slow", "never finishes");
        t.timeout = Duration::from_millis(300);
        let err = driver.run(&t).unwrap_err();
        assert!(err.to_string().contains("exceeded timeout"), "got: {err}");
    }

    #[test]
    fn collect_fast_forwards_and_reports_heads() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let (fake, _record) = fake_opencode(&dir, r#"{"sessionID":"ses_c"}"#, "");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(10),
        };
        let wt = driver.prepare_worktree("feat").unwrap();
        std::fs::write(wt.join("g.txt"), "work").unwrap();
        let git_wt = |args: &[&str]| {
            Command::new("git")
                .current_dir(&wt)
                .args(args)
                .output()
                .unwrap()
        };
        git_wt(&["add", "."]);
        git_wt(&["commit", "-q", "-m", "feat: g"]);

        let report = driver.collect("feat").unwrap();
        assert_eq!(report.branch, "fleet/feat");
        assert_ne!(report.head_before, report.head_after);
        assert!(repo.join("g.txt").is_file());
        let again = driver.collect("feat").unwrap();
        assert_eq!(again.head_before, again.head_after);
    }

    #[test]
    fn collect_rejects_dirty_main_and_diverged_target() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let (fake, _record) = fake_opencode(&dir, "{}", "");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(10),
        };
        driver.prepare_worktree("feat").unwrap();

        // Dirty main checkout -> refused (a MODIFIED tracked file; an
        // untracked one does not block a merge).
        std::fs::write(repo.join("f.txt"), "dirty").unwrap();
        let err = driver.collect("feat").unwrap_err();
        assert!(
            err.to_string().contains("uncommitted changes"),
            "got: {err}"
        );
        Command::new("git")
            .current_dir(&repo)
            .args(["checkout", "--", "f.txt"])
            .output()
            .unwrap();

        // Diverged target -> refused (non-fast-forward): BOTH sides
        // advance. (Only moving main keeps fleet/feat an ancestor and
        // `--ff-only` is a legal no-op "already up to date".)
        let git = |args: &[&str], cwd: &PathBuf| {
            Command::new("git")
                .current_dir(cwd)
                .args(args)
                .output()
                .unwrap()
        };
        let wt = driver.worktree_root.join("feat");
        std::fs::write(wt.join("w.txt"), "work").unwrap();
        git(&["add", "."], &wt);
        git(&["commit", "-q", "-m", "feat: w"], &wt);
        git(
            &["commit", "-q", "--allow-empty", "-m", "main moved on"],
            &repo,
        );
        let err = driver.collect("feat").unwrap_err();
        assert!(err.to_string().contains("diverged"), "got: {err}");
    }

    #[test]
    fn discard_removes_worktree_and_branch() {
        let dir = tempfile::tempdir().unwrap();
        let repo = temp_git_repo(&dir);
        let (fake, _record) = fake_opencode(&dir, "{}", "");
        let driver = OpencodeDriver {
            bin: fake,
            repo: repo.clone(),
            worktree_root: repo.join(".fleet-worktrees"),
            default_timeout: Duration::from_secs(10),
        };
        let wt = driver.prepare_worktree("gone").unwrap();
        assert!(wt.is_dir());
        driver.discard("gone").unwrap();
        assert!(!wt.exists());
        assert!(driver.discard("gone").is_err());
    }

    #[test]
    fn session_id_handles_both_spellings_and_garbage() {
        assert_eq!(
            extract_session_id(r#"{"sessionID":"ses_a"}"#).as_deref(),
            Some("ses_a")
        );
        assert_eq!(
            extract_session_id(r#"{"sessionId":"ses_b"}"#).as_deref(),
            Some("ses_b")
        );
        assert_eq!(extract_session_id("not json at all"), None);
        assert_eq!(extract_session_id(r#"{"other":1}"#), None);
    }
}
