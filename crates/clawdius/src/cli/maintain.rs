//! `clawdius maintain` — the 24/7 maintenance loop.
//!
//! Each cycle takes the highest-priority pending task from the durable
//! backlog, runs it in an isolated git worktree through the dev pipeline
//! (LLM stages + deterministic gates), and records the result:
//!
//! - **green** → commit on branch `maint/<id>`, push, open a PR
//! - **red** → discard the worktree and branch, record why
//! - **rate-limited** → rotate to the next provider in the chain; when
//!   every provider is cooling down, sleep until the soonest reset
//!
//! The main working tree is never touched. State lives in
//! `.clawdius/maintenance/backlog.json` and `.clawdius/maintenance/ledger.jsonl`,
//! so the loop survives restarts, crashes and context loss.

use anyhow::{anyhow, Context, Result};
use clawdius_core::maintenance::{
    now_rfc3339, Backlog, CycleOutcome, CycleRecord, Ledger, Task, TaskKind, TaskStatus,
};
use clawdius_core::orchestrator::fallback::{FallbackCoordinator, FallbackPolicy};
use clawdius_core::orchestrator::pipeline::{Pipeline, StageKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Kinds the loop enqueues for itself when the backlog runs dry.
const REPLENISH_ROTATION: [TaskKind; 4] = [
    TaskKind::Lint,
    TaskKind::Deps,
    TaskKind::Docs,
    TaskKind::Tests,
];

/// How many provider rotations one task gets before it is marked failed.
const MAX_PROVIDER_ROTATIONS: usize = 3;

pub(super) async fn handle_maintain(
    backlog_path: Option<PathBuf>,
    once: bool,
    dry_run: bool,
    max_cycles: Option<usize>,
    interval_secs: u64,
    provider: String,
    model: Option<String>,
    worktree_root: Option<PathBuf>,
    no_pr: bool,
    replenish: bool,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let repo = repo_root()?;
    let backlog_path = backlog_path.unwrap_or_else(|| default_backlog(&repo));
    let ledger = Ledger::new(repo.join(".clawdius/maintenance/ledger.jsonl"));
    let worktrees = worktree_root.unwrap_or_else(|| repo.join(".clawdius/worktrees"));

    if dry_run {
        let backlog = Backlog::load(&backlog_path)?;
        print_backlog(&backlog, &backlog_path);
        println!("\nLedger: {}", ledger.path().display());
        println!("Worktrees: {}", worktrees.display());
        return Ok(());
    }

    println!("Maintenance loop — repo {}", repo.display());
    println!("  backlog:   {}", backlog_path.display());
    println!("  ledger:    {}", ledger.path().display());
    println!("  worktrees: {}", worktrees.display());
    println!(
        "  provider:  {provider}{}",
        model
            .as_deref()
            .map(|m| format!(" / {m}"))
            .unwrap_or_default()
    );
    println!(
        "  mode:      {}{}",
        if once { "single cycle" } else { "continuous" },
        max_cycles
            .map(|n| format!(", max {n} cycles"))
            .unwrap_or_default()
    );
    println!();

    let mut coordinator = FallbackCoordinator::new(FallbackPolicy {
        chain: provider_chain(&provider),
        max_attempts: MAX_PROVIDER_ROTATIONS,
        cooldown: Duration::from_mins(5),
    });

    let mut cycles = 0usize;

    loop {
        cycles += 1;
        if let Some(max) = max_cycles {
            if cycles > max {
                break;
            }
        }

        // Re-read the backlog every cycle: humans and other agents edit it.
        let mut backlog = Backlog::load(&backlog_path)?;

        if backlog.next_pending().is_none() {
            if replenish {
                if let Some(task) = replenish_task(&backlog) {
                    println!(
                        "backlog dry — enqueued `{}` ({})",
                        task.id,
                        task.kind.slug()
                    );
                    backlog.push(task)?;
                    backlog.save(&backlog_path)?;
                }
            }
            if backlog.next_pending().is_none() {
                let (pending, done, failed, other) = backlog.counts();
                println!(
                    "[{cycles}] idle — nothing pending (done {done}, failed {failed}, other {other}, pending {pending})"
                );
                ledger.append(&CycleRecord {
                    at: now_rfc3339(),
                    task_id: "-".to_string(),
                    kind: TaskKind::Custom,
                    outcome: CycleOutcome::Idle,
                    provider: provider.clone(),
                    branch: None,
                    pr_url: None,
                    note: "no pending task".to_string(),
                    duration_ms: 0,
                })?;
                if once {
                    break;
                }
                sleep_or_stop(interval_secs);
                continue;
            }
        }

        let task = backlog
            .next_pending()
            .cloned()
            .ok_or_else(|| anyhow!("backlog changed under us"))?;

        println!(
            "\n=== [{cycles}] task `{}` ({}) priority {} ===",
            task.id,
            task.kind.slug(),
            task.priority
        );
        backlog.set_status(&task.id, TaskStatus::Running);
        backlog.save(&backlog_path)?;

        let started = Instant::now();
        let outcome = run_task_with_rotation(
            &task,
            &repo,
            &worktrees,
            &mut coordinator,
            model.as_deref(),
            no_pr,
            config_path.as_deref(),
        )
        .await;

        let (cycle_outcome, note, branch, pr_url) = match outcome {
            Ok(cycle) => (cycle.outcome, cycle.note, cycle.branch, cycle.pr_url),
            Err(e) => (CycleOutcome::Error, format!("{e:#}"), None, None),
        };

        let status = match cycle_outcome {
            CycleOutcome::Green => TaskStatus::Done,
            CycleOutcome::Red | CycleOutcome::Error => TaskStatus::Failed,
            CycleOutcome::Idle => TaskStatus::Pending,
        };
        backlog.set_status(&task.id, status);
        backlog.save(&backlog_path)?;

        let record = CycleRecord {
            at: now_rfc3339(),
            task_id: task.id.clone(),
            kind: task.kind,
            outcome: cycle_outcome,
            provider: coordinator
                .ledger
                .pick_available(&coordinator.policy.chain)
                .cloned()
                .unwrap_or_else(|| provider.clone()),
            branch,
            pr_url,
            note: note.clone(),
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        ledger.append(&record)?;

        println!(
            "=== [{}] {} — {} ({:.1}s) ===",
            cycles,
            task.id,
            match cycle_outcome {
                CycleOutcome::Green => "GREEN",
                CycleOutcome::Red => "RED",
                CycleOutcome::Idle => "IDLE",
                CycleOutcome::Error => "ERROR",
            },
            started.elapsed().as_secs_f64()
        );
        if !note.is_empty() {
            println!("    {note}");
        }

        if once {
            break;
        }

        // If every provider is cooling down, park until the soonest reset
        // rather than burning the interval on guaranteed failures.
        if let Some(resume_in) = coordinator.ledger.soonest_reset() {
            if coordinator
                .ledger
                .pick_available(&coordinator.policy.chain)
                .is_none()
            {
                let wait = resume_in.max(Duration::from_secs(interval_secs));
                println!(
                    "    all providers rate-limited — parking for {}s",
                    wait.as_secs()
                );
                std::thread::sleep(wait);
                continue;
            }
        }

        sleep_or_stop(interval_secs);
    }

    let backlog = Backlog::load(&backlog_path)?;
    let (pending, done, failed, other) = backlog.counts();
    println!(
        "\nStopped after {cycles} cycle(s). Backlog: {pending} pending, {done} done, {failed} failed, {other} other."
    );
    println!("Ledger: {}", ledger.path().display());
    Ok(())
}

/// One cycle's report.
struct CycleReport {
    outcome: CycleOutcome,
    note: String,
    branch: Option<String>,
    pr_url: Option<String>,
}

/// Run one task, rotating providers on quota errors.
async fn run_task_with_rotation(
    task: &Task,
    repo: &Path,
    worktrees: &Path,
    coordinator: &mut FallbackCoordinator,
    model: Option<&str>,
    no_pr: bool,
    config_path: Option<&Path>,
) -> Result<CycleReport> {
    let mut last_error: Option<String> = None;

    for attempt in 1..=MAX_PROVIDER_ROTATIONS {
        let provider = match coordinator.next() {
            clawdius_core::orchestrator::fallback::FallbackDecision::Try(p) => p,
            clawdius_core::orchestrator::fallback::FallbackDecision::Park(wait) => {
                println!("    providers cooling down — waiting {}s", wait.as_secs());
                std::thread::sleep(wait);
                continue;
            },
            clawdius_core::orchestrator::fallback::FallbackDecision::Escalate => {
                return Ok(CycleReport {
                    outcome: CycleOutcome::Error,
                    note: format!(
                        "attempt budget spent after {} attempt(s): {}",
                        attempt - 1,
                        last_error.unwrap_or_else(|| "unknown".to_string())
                    ),
                    branch: None,
                    pr_url: None,
                })
            },
        };

        println!("    attempt {attempt}: provider `{provider}`");
        match run_task_once(task, repo, worktrees, &provider, model, no_pr, config_path).await {
            Ok(report) => {
                coordinator.note_solved(&provider);
                return Ok(report);
            },
            Err(e) => {
                let text = format!("{e:#}");
                if is_rate_limit(&text) {
                    println!("    `{provider}` rate-limited — rotating");
                    coordinator.note_limited(&provider);
                } else {
                    coordinator.note_failed();
                }
                last_error = Some(text);
            },
        }
    }

    Ok(CycleReport {
        outcome: CycleOutcome::Error,
        note: last_error.unwrap_or_else(|| "all attempts failed".to_string()),
        branch: None,
        pr_url: None,
    })
}

/// One task attempt in a fresh worktree.
async fn run_task_once(
    task: &Task,
    repo: &Path,
    worktrees: &Path,
    provider: &str,
    model: Option<&str>,
    no_pr: bool,
    config_path: Option<&Path>,
) -> Result<CycleReport> {
    let branch = format!("maint/{}", sanitize(&task.id));
    let wt = worktrees.join(sanitize(&task.id));

    // Start clean: an interrupted previous run may have left a worktree.
    if wt.exists() {
        let _ = Command::new("git")
            .args(["-C", &repo.to_string_lossy()])
            .args(["worktree", "remove", "--force", &wt.to_string_lossy()])
            .output();
        let _ = std::fs::remove_dir_all(&wt);
    }
    let _ = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["branch", "-D", &branch])
        .output();

    std::fs::create_dir_all(worktrees)?;
    let added = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args([
            "worktree",
            "add",
            "-b",
            &branch,
            &wt.to_string_lossy(),
            "HEAD",
        ])
        .output()
        .context("git worktree add")?;
    if !added.status.success() {
        return Err(anyhow!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&added.stderr).trim()
        ));
    }

    let result = run_pipeline_in(&wt, task, provider, model, config_path).await;

    // Preserve the cycle's evidence before the worktree disappears: the
    // auto-run journal and the pipeline's stage deliverables are the only
    // record of what the model actually did (live-caught: a red cycle
    // removed the worktree and took the journal with it, so the spin was
    // undiagnosable).
    preserve_artifacts(&wt, repo, &task.id);

    let cleanup = |green: bool| {
        let _ = Command::new("git")
            .args(["-C", &repo.to_string_lossy()])
            .args(["worktree", "remove", "--force", &wt.to_string_lossy()])
            .output();
        if !green {
            let _ = Command::new("git")
                .args(["-C", &repo.to_string_lossy()])
                .args(["branch", "-D", &branch])
                .output();
        }
    };

    match result {
        Ok(()) => {
            // Commit the green work on the branch.
            let _ = Command::new("git")
                .args(["-C", &wt.to_string_lossy()])
                .args(["add", "-A"])
                .output();
            let commit = Command::new("git")
                .args(["-C", &wt.to_string_lossy()])
                .args(["-c", "user.name=clawdius-maint"])
                .args(["-c", "user.email=maint@clawdius.local"])
                .args([
                    "commit",
                    "-m",
                    &format!("maint({}): {}", task.kind.slug(), task.id),
                    "-m",
                    &format!(
                        "Automated maintenance cycle.\n\nGoal: {}",
                        task.effective_goal()
                    ),
                ])
                .output()?;
            if !commit.status.success() {
                let stderr = String::from_utf8_lossy(&commit.stderr).trim().to_string();
                cleanup(false);
                return Ok(CycleReport {
                    outcome: CycleOutcome::Red,
                    note: format!("gates green but commit failed: {stderr}"),
                    branch: None,
                    pr_url: None,
                });
            }

            // Push and open a PR when a remote exists.
            let push_ok = Command::new("git")
                .args(["-C", &wt.to_string_lossy()])
                .args(["push", "-u", "origin", &branch])
                .output()
                .is_ok_and(|o| o.status.success());

            let pr_url = if push_ok && !no_pr {
                open_pr(repo, &branch, task)
            } else {
                None
            };

            cleanup(true);
            Ok(CycleReport {
                outcome: CycleOutcome::Green,
                note: if push_ok {
                    format!("committed to `{branch}`")
                } else {
                    format!("committed to local branch `{branch}` (no remote push)")
                },
                branch: Some(branch),
                pr_url,
            })
        },
        Err(e) => {
            cleanup(false);
            let text = format!("{e:#}");
            Ok(CycleReport {
                outcome: CycleOutcome::Red,
                note: text,
                branch: None,
                pr_url: None,
            })
        },
    }
}

/// Copy a worktree's `.clawdius/` run artifacts into the main repo.
fn preserve_artifacts(worktree: &Path, repo: &Path, task_id: &str) {
    let src = worktree.join(".clawdius");
    if !src.exists() {
        return;
    }
    let dest = repo
        .join(".clawdius/maintenance/runs")
        .join(sanitize(task_id));
    for sub in ["runs", "pipeline"] {
        let from = src.join(sub);
        if from.is_dir() {
            let _ = copy_dir(&from, &dest.join(sub));
        }
    }
}

/// Recursively copy `from` into `to` (best effort; artifacts are evidence,
/// not correctness).
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Run the dev pipeline with `workdir` as the process cwd.
async fn run_pipeline_in(
    workdir: &Path,
    task: &Task,
    provider: &str,
    model: Option<&str>,
    config_path: Option<&Path>,
) -> Result<()> {
    let mut pipeline = Pipeline::dev_loop(task.effective_goal());
    // Maintenance tasks are small; two loops is plenty and bounds cost.
    pipeline.max_loops = 2;
    pipeline
        .validate()
        .map_err(|e| anyhow!("invalid pipeline: {e}"))?;
    debug_assert!(pipeline.stages.iter().any(|s| s.kind == StageKind::Lint));

    let previous = std::env::current_dir()?;
    std::env::set_current_dir(workdir)
        .with_context(|| format!("cannot enter worktree {}", workdir.display()))?;
    let result = super::pipeline::run_pipeline_in(
        workdir.to_path_buf(),
        &pipeline,
        provider.to_string(),
        model.map(String::from),
        config_path.map(Path::to_path_buf),
    )
    .await;
    std::env::set_current_dir(&previous).ok();
    result
}

/// Open a PR for a green branch; returns the URL when `gh` succeeds.
fn open_pr(repo: &Path, branch: &str, task: &Task) -> Option<String> {
    let output = Command::new("gh")
        .current_dir(repo)
        .args(["pr", "create", "--head", branch, "--fill", "--title"])
        .arg(format!("maint({}): {}", task.kind.slug(), task.id))
        .args(["--body"])
        .arg(format!(
            "Automated maintenance cycle.\n\nGoal:\n\n{}\n\nGates: fmt, clippy -D warnings, tests.\n",
            task.effective_goal()
        ))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

/// Enqueue one standing task per day when the backlog is dry.
fn replenish_task(backlog: &Backlog) -> Option<Task> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for kind in REPLENISH_ROTATION {
        let id = format!("{}-{today}", kind.slug());
        if !backlog.tasks.iter().any(|t| t.id == id) {
            let mut task = Task::new(id, kind, 5);
            task.goal = kind.default_goal().to_string();
            return Some(task);
        }
    }
    None
}

fn print_backlog(backlog: &Backlog, path: &Path) {
    println!("Backlog: {}", path.display());
    let (pending, done, failed, other) = backlog.counts();
    println!("  {pending} pending, {done} done, {failed} failed, {other} other\n");
    if backlog.tasks.is_empty() {
        println!("  (empty — the loop will enqueue standing tasks when it runs)");
        return;
    }
    println!("  {:<28} {:<8} {:>8}  status", "id", "kind", "priority");
    for task in &backlog.tasks {
        println!(
            "  {:<28} {:<8} {:>8}  {:?}",
            task.id,
            task.kind.slug(),
            task.priority,
            task.status
        );
    }
    if let Some(next) = backlog.next_pending() {
        println!("\nNext up: {} ({})", next.id, next.kind.slug());
    }
}

fn default_backlog(repo: &Path) -> PathBuf {
    repo.join(".clawdius/maintenance/backlog.json")
}

fn provider_chain(preferred: &str) -> Vec<String> {
    let mut chain = vec![preferred.to_string()];
    for fallback in ["google", "anthropic", "deepseek", "zai"] {
        if fallback != preferred {
            chain.push(fallback.to_string());
        }
    }
    chain
}

fn is_rate_limit(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "429",
        "rate limit",
        "rate-limit",
        "quota",
        "insufficient balance",
        "resource_exhausted",
        "too many requests",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn sleep_or_stop(secs: u64) {
    if secs == 0 {
        return;
    }
    println!("    sleeping {secs}s");
    std::thread::sleep(Duration::from_secs(secs));
}

/// Branch-safe slug from a task id.
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Repository root via git, falling back to the process cwd.
fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }
    Ok(std::env::current_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_detection_covers_the_real_error_shapes() {
        assert!(is_rate_limit(
            "LLM error: Request failed with status code '429 Too Many Requests'"
        ));
        assert!(is_rate_limit("Insufficient Balance"));
        assert!(is_rate_limit("RESOURCE_EXHAUSTED: quota exceeded"));
        assert!(!is_rate_limit("semantic loop detected"));
        assert!(!is_rate_limit("compile error: expected `;`"));
    }

    #[test]
    fn provider_chain_starts_with_preference_and_has_no_duplicates() {
        let chain = provider_chain("google");
        assert_eq!(chain[0], "google");
        let mut seen = std::collections::HashSet::new();
        for p in &chain {
            assert!(seen.insert(p.clone()), "duplicate provider {p}");
        }
    }

    #[test]
    fn sanitize_makes_branch_safe_slugs() {
        assert_eq!(sanitize("lint-2026-11-13"), "lint-2026-11-13");
        assert_eq!(sanitize("issue/123 fix"), "issue-123-fix");
        assert_eq!(sanitize("a:b*c"), "a-b-c");
    }

    #[test]
    fn replenish_enqueues_one_standing_task_per_kind_per_day() {
        let mut backlog = Backlog::default();
        let first = replenish_task(&backlog).expect("first replenish");
        assert_eq!(first.kind, TaskKind::Lint);
        backlog.push(first.clone()).unwrap();

        let second = replenish_task(&backlog).expect("second replenish");
        assert_eq!(second.kind, TaskKind::Deps);
        backlog.push(second).unwrap();

        // Every kind for today exists -> nothing left to enqueue.
        for kind in REPLENISH_ROTATION {
            let id = format!("{}-{}", kind.slug(), chrono::Utc::now().format("%Y-%m-%d"));
            if !backlog.tasks.iter().any(|t| t.id == id) {
                backlog.push(Task::new(id, kind, 5)).unwrap();
            }
        }
        assert!(replenish_task(&backlog).is_none());
    }
}
