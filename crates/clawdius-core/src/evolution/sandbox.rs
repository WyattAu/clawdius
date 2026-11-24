//! Island evaluation isolation over the agentic worktree manager.
//!
//! Islands evaluate candidates concurrently; un-sandboxed subprocess
//! evaluators all inherit the process cwd and stomp each other's side
//! effects (build dirs, generated files, checkouts). The sandbox gives each
//! evaluation its own git worktree: the evaluator subprocess runs with the
//! worktree as cwd, the program temp file lives inside it, and the whole
//! tree is removed after scoring — nothing leaks into the shared checkout
//! and no two islands can collide.
//!
//! The design intent documented in [`super`] ("islands map onto fleet
//! worktree isolation") lands here: [`crate::agentic::WorktreeManager`]
//! owns worktree lifecycle; this module adapts it to the [`Evaluator`]
//! contract.

use std::path::PathBuf;
use std::time::Duration;

use crate::agentic::WorktreeManager;

use super::evaluator::{run_evaluator_subprocess, Evaluated, Evaluator};
use super::program::Program;

/// Evaluate programs inside per-evaluation git worktrees.
///
/// Wraps a subprocess evaluator (`argv` + timeout, same contract as
/// [`super::evaluator::CommandEvaluator`]) and runs each evaluation in a
/// fresh worktree created from the repo's HEAD.
pub struct SandboxedCommandEvaluator {
    manager: WorktreeManager,
    argv: Vec<String>,
    timeout: Duration,
}

impl SandboxedCommandEvaluator {
    /// Sandbox `argv` evaluations to worktrees of `repo_root`.
    ///
    /// # Errors
    ///
    /// [`crate::agentic::WorktreeError`] when the manager cannot be built
    /// over `repo_root`.
    pub fn new(
        repo_root: impl Into<PathBuf>,
        argv: Vec<String>,
        timeout: Duration,
    ) -> Result<Self, crate::agentic::WorktreeError> {
        Ok(Self {
            manager: WorktreeManager::new(repo_root.into())?,
            argv,
            timeout,
        })
    }
}

impl Evaluator for SandboxedCommandEvaluator {
    fn evaluate(&self, program: &Program) -> Evaluated {
        let session = match self
            .manager
            .create_worktree(&format!("evo-eval {}", program.id))
        {
            Ok(s) => s,
            Err(e) => {
                let mut ev = Evaluated::single("score", f64::NEG_INFINITY);
                ev.artifacts = Some(format!("sandbox worktree creation failed: {e}"));
                return ev;
            },
        };

        let evaluated = run_evaluator_subprocess(
            &self.argv,
            self.timeout,
            program,
            Some(&session.worktree_path),
        );

        // Best-effort teardown: a leftover worktree is visible to
        // `WorktreeManager::cleanup_all` and must not fail the evaluation.
        if let Err(e) = self.manager.remove_worktree(&session) {
            tracing::warn!(error = %e, session = %session.id, "sandbox worktree removal failed");
        }
        evaluated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One git repo per test: init + an initial commit so worktree add has
    /// a HEAD.
    fn temp_repo(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "evo-sandbox-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        // Hermetic: ambient global/system git config (hooks, signing) and
        // /tmp pressure are the suspected flake sources (#116) — isolate
        // config the same way the maintain test does.
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&root)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_AUTHOR_NAME", "evo")
                .env("GIT_AUTHOR_EMAIL", "evo@test")
                .env("GIT_COMMITTER_NAME", "evo")
                .env("GIT_COMMITTER_EMAIL", "evo@test")
                .output()
                .expect("git runs")
        };
        let ok = |step: &str, out: &std::process::Output| {
            assert!(
                out.status.success(),
                "git {step} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        ok("init", &git(&["init", "-q"]));
        std::fs::write(root.join("README.md"), "sandbox test\n").expect("seed file");
        ok("add", &git(&["add", "."]));
        ok("commit", &git(&["commit", "-q", "-m", "seed"]));
        root
    }

    fn program(code: &str) -> Program {
        let seed = Program::new("fn main() {}\n", uuid::Uuid::new_v4());
        Program::child_of(&seed, code.to_string(), uuid::Uuid::new_v4(), 1)
    }

    fn cleanup(root: &PathBuf) {
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluate_runs_inside_a_worktree_and_cleans_up() {
        let repo = temp_repo("isolate");
        // The evaluator reports the cwd it ran in through `artifacts` —
        // the only channel that survives worktree teardown.
        let argv = vec![
            "bash".to_string(),
            "-c".to_string(),
            "printf '{\"metrics\":{\"score\":1.0},\"artifacts\":\"%s\"}' \"$(pwd)\"".to_string(),
            "bash".to_string(),
        ];
        let sandbox =
            SandboxedCommandEvaluator::new(&repo, argv, Duration::from_secs(30)).expect("sandbox");

        let ev = sandbox.evaluate(&program("fn child() {}\n"));
        assert_eq!(ev.metrics.get("score"), Some(&1.0));
        let artifacts = ev.artifacts.expect("pwd captured");
        assert!(
            artifacts.contains(".clawdius-worktrees"),
            "evaluator cwd was outside the sandbox: {artifacts}"
        );
        assert!(
            !artifacts.contains(&repo.to_string_lossy().as_ref()),
            "evaluator cwd was the shared repo root: {artifacts}"
        );

        // Teardown: no sprint worktrees left behind.
        assert!(
            sandbox.manager.list_worktrees().expect("list").is_empty(),
            "worktree leaked after evaluation"
        );
        cleanup(&repo);
    }

    #[test]
    fn program_temp_file_lives_inside_the_sandbox() {
        let repo = temp_repo("tmpfile");
        // The evaluator receives the program path as $1; prove it is inside
        // the worktree by relaying its parent directory through artifacts.
        let argv = vec![
            "bash".to_string(),
            "-c".to_string(),
            "dir=$(dirname \"$1\"); printf '{\"metrics\":{\"score\":2.0},\"artifacts\":\"%s\"}' \"$dir\""
                .to_string(),
            "bash".to_string(),
        ];
        let sandbox =
            SandboxedCommandEvaluator::new(&repo, argv, Duration::from_secs(30)).expect("sandbox");

        let ev = sandbox.evaluate(&program("fn x() {}\n"));
        assert_eq!(ev.metrics.get("score"), Some(&2.0));
        let artifacts = ev.artifacts.expect("tmp dir captured");
        assert!(
            artifacts.contains(".clawdius-worktrees"),
            "program temp file escaped the sandbox: {artifacts}"
        );
        cleanup(&repo);
    }

    #[test]
    fn evaluator_failure_scores_negative_infinity() {
        let repo = temp_repo("fail");
        let argv = vec![
            "bash".to_string(),
            "-c".to_string(),
            "exit 3".to_string(),
            "bash".to_string(),
        ];
        let sandbox =
            SandboxedCommandEvaluator::new(&repo, argv, Duration::from_secs(30)).expect("sandbox");
        let ev = sandbox.evaluate(&program("fn x() {}\n"));
        assert_eq!(ev.metrics.get("score"), Some(&f64::NEG_INFINITY));
        assert!(sandbox.manager.list_worktrees().expect("list").is_empty());
        cleanup(&repo);
    }

    #[test]
    fn non_repo_fails_sandbox_construction() {
        // A directory that is not a git repo: the manager validates the
        // repo at construction, so the sandbox fails fast instead of
        // failing per-evaluation.
        let root = std::env::temp_dir().join(format!(
            "evo-sandbox-nogit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let argv = vec!["bash".to_string(), "-c".to_string(), "true".to_string()];
        assert!(
            SandboxedCommandEvaluator::new(&root, argv, Duration::from_secs(30)).is_err(),
            "non-repo must fail construction"
        );
        cleanup(&root);
    }

    #[test]
    fn two_evaluations_do_not_collide() {
        let repo = temp_repo("collide");
        let argv = vec![
            "bash".to_string(),
            "-c".to_string(),
            "printf '{\"metrics\":{\"score\":%s}}' \"$1\" >/dev/null; printf '{\"metrics\":{\"score\":0.5}}'"
                .to_string(),
            "bash".to_string(),
        ];
        let sandbox =
            SandboxedCommandEvaluator::new(&repo, argv, Duration::from_secs(30)).expect("sandbox");
        let a = sandbox.evaluate(&program("fn a() {}\n"));
        let b = sandbox.evaluate(&program("fn b() {}\n"));
        assert_eq!(a.metrics.get("score"), Some(&0.5));
        assert_eq!(b.metrics.get("score"), Some(&0.5));
        assert!(sandbox.manager.list_worktrees().expect("list").is_empty());
        cleanup(&repo);
    }
}
