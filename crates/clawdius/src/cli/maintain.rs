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
use clawdius_core::config::LlmConfig;
use clawdius_core::maintenance::model_quality::{Attribution, ModelQuality};
use clawdius_core::maintenance::{
    now_rfc3339, Autonomy, Backlog, CycleOutcome, CycleRecord, Ledger, Task, TaskKind, TaskStatus,
};
use clawdius_core::orchestrator::fallback::{FallbackCoordinator, FallbackPolicy};
use clawdius_core::orchestrator::pipeline::{Pipeline, StageKind};
use serde_json::{json, Value};
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

/// Longest provider reset hint the loop will sleep through. A hint
/// beyond this is a daily quota reset: sleeping would wedge the task for
/// hours (live: 5 attempts x 30 min against a 16 h hint), so the cycle
/// fails fast and the task is retried after the reset.
const MAX_HONORED_WAIT: Duration = Duration::from_mins(10);

/// Whether a provider reset hint is short enough to sleep through.
/// Longer hints are daily quota resets: the cycle fails fast instead of
/// sleeping for hours.
fn should_honor_wait(wait: Duration) -> bool {
    wait <= MAX_HONORED_WAIT
}

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
    autonomy: String,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let repo = repo_root()?;
    let backlog_path = backlog_path.unwrap_or_else(|| default_backlog(&repo));
    let autonomy: Autonomy = autonomy.parse().map_err(|e| anyhow!("{e}"))?;
    let ledger = Ledger::new(repo.join(".clawdius/maintenance/ledger.jsonl"));
    let worktrees = worktree_root.unwrap_or_else(|| repo.join(".clawdius/worktrees"));

    if dry_run {
        let backlog = Backlog::load(&backlog_path)?;
        print_backlog(&backlog, &backlog_path);
        println!("\nLedger: {}", ledger.path().display());
        println!("Worktrees: {}", worktrees.display());
        println!("Autonomy: {autonomy}");
        return Ok(());
    }

    {
        // Persist so restarts keep the operator's choice; the backlog is
        // re-read every cycle and would otherwise reset it. After the
        // dry-run branch: a read-only command must not write state.
        let mut b = Backlog::load(&backlog_path)?;
        if b.autonomy != autonomy {
            b.autonomy = autonomy;
            b.save(&backlog_path)?;
        }
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
    println!("  autonomy:  {autonomy}");

    // Single instance per backlog: the lock file is held for the whole
    // process lifetime; a second loop exits here instead of racing the
    // first for tasks.
    let Some(_loop_lock) = acquire_loop_lock(&backlog_path) else {
        println!(
            "another maintain loop holds {} — exiting",
            backlog_path.with_extension("lock").display()
        );
        return Ok(());
    };
    write_heartbeat(interval_secs);
    println!();

    // Only providers the stages can actually authenticate against enter
    // the rotation; a missing key is not a rescue, it is a dead end.
    let llm_config = super::load_config(config_path.as_deref())
        .map(|c| c.llm)
        .unwrap_or_default();
    let mut coordinator = FallbackCoordinator::new(FallbackPolicy {
        chain: provider_chain(&provider, &llm_config),
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

        // Anything still `running` comes from a crashed instance (this
        // process sets `running` only for the task it is executing now).
        let recovered = backlog.reset_stale_running();
        if !recovered.is_empty() {
            println!(
                "recovered {} task(s) left running by a dead cycle: {}",
                recovered.len(),
                recovered.join(", ")
            );
            ledger.append(&CycleRecord {
                at: now_rfc3339(),
                task_id: recovered.join(","),
                kind: TaskKind::Custom,
                outcome: CycleOutcome::Idle,
                provider: provider.clone(),
                branch: None,
                pr_url: None,
                note: "reset to pending after an interrupted cycle".to_string(),
                duration_ms: 0,
                tool_calls: 0,
                files_changed: 0,
            })?;
            backlog.save(&backlog_path)?;
        }

        if backlog.next_pending().is_none() {
            if replenish {
                if let Some(task) = replenish_task(&mut backlog, &repo, true)? {
                    println!(
                        "backlog dry — enqueued `{}` ({})",
                        task.id,
                        task.kind.slug()
                    );
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
                    tool_calls: 0,
                    files_changed: 0,
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
        // Safety net: the cycle must not touch the primary tree. Snapshot
        // it so anything that leaks is detected and reverted.
        let tree_before = dirty_paths(&repo);
        let outcome = run_task_with_rotation(
            &task,
            &repo,
            &worktrees,
            &mut coordinator,
            &provider,
            autonomy,
            model.as_deref(),
            no_pr,
            config_path.as_deref(),
        )
        .await;

        let (
            mut cycle_outcome,
            mut note,
            branch,
            pr_url,
            outcome_tool_calls,
            outcome_files_changed,
            quota_blocked,
        ) = match outcome {
            Ok(cycle) => (
                cycle.outcome,
                cycle.note,
                cycle.branch,
                cycle.pr_url,
                cycle.tool_calls,
                cycle.files_changed,
                cycle.quota_blocked,
            ),
            Err(e) => (
                CycleOutcome::Error,
                format!("{e:#}"),
                None,
                None,
                0,
                0,
                false,
            ),
        };

        // A stage that resolved the git project root instead of the
        // worktree (opencode does exactly this) would write to the main
        // tree. Revert those files and say so.
        let leaked = revert_leaks(&repo, &tree_before);
        if !leaked.is_empty() {
            println!(
                "    !! {} file(s) written outside the worktree — reverted: {}",
                leaked.len(),
                leaked.join(", ")
            );
            note = format!(
                "{note} [leak: {} file(s) reverted outside the worktree: {}]",
                leaked.len(),
                leaked.join(", ")
            );
            if cycle_outcome == CycleOutcome::Green {
                // The green commit was made in the worktree; a leak means
                // the cycle's evidence is not trustworthy.
                cycle_outcome = CycleOutcome::Error;
            }
        }

        let status = status_for(cycle_outcome, quota_blocked);
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
            tool_calls: outcome_tool_calls,
            files_changed: outcome_files_changed,
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

        // A quota-blocked cycle requeued its task. Park until the reset:
        // the chain's other entries are usually unconfigured, and letting
        // the rotation reach them spends the task on a guaranteed config
        // error (live: "API key not set for Anthropic" on a quota-red).
        if quota_blocked {
            let wait = coordinator.ledger.soonest_reset().map_or_else(
                || Duration::from_secs(interval_secs),
                |d| d.max(Duration::from_secs(interval_secs)),
            );
            println!(
                "    provider quota blocked — task requeued, parking {}s",
                wait.as_secs()
            );
            sleep_or_stop(wait.as_secs());
            continue;
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
    /// Tool calls the model made (from the preserved run journal).
    tool_calls: u32,
    /// Files the cycle changed on the branch.
    files_changed: u32,
    /// The cycle failed only because the provider's quota is spent until a
    /// known reset: the task is requeued (not failed) and the loop parks
    /// until the reset instead of rotating to unconfigured providers.
    quota_blocked: bool,
}

/// `--model` names a model in the *preferred* provider's namespace:
/// `auto` is a router alias, `glm-4.5-flash` a Z.AI id. Handing it to a
/// rotated-in provider is meaningless and actively harmful — genai does
/// not recognise the name and falls back to the Ollama adapter (live:
/// `model 'auto' not found` from localhost:11434 after a Google rotation).
/// Rotated providers use their own default model.
fn model_for_attempt<'a>(
    preferred: &str,
    provider: &str,
    model: Option<&'a str>,
) -> Option<&'a str> {
    if provider == preferred {
        model
    } else {
        None
    }
}

/// Run one task, rotating providers on quota errors.
async fn run_task_with_rotation(
    task: &Task,
    repo: &Path,
    worktrees: &Path,
    coordinator: &mut FallbackCoordinator,
    preferred: &str,
    autonomy: Autonomy,
    model: Option<&str>,
    no_pr: bool,
    config_path: Option<&Path>,
) -> Result<CycleReport> {
    let mut last_error: Option<String> = None;

    // Quality-scored routing: `--model auto:quality` resolves to the
    // best non-quarantined model from the loop's own outcome history.
    let mut quality = ModelQuality::load(&repo.join(".clawdius/maintenance/model-quality.json"));
    let resolved_model: Option<String> = if model.as_deref() == Some("auto:quality") {
        let best = quality.best_model(preferred);
        if best.is_none() {
            println!("    auto:quality: no proven model yet — falling back to provider default");
        }
        best.map(|(_, m)| m)
    } else {
        model.map(String::from)
    };
    let model_ref: Option<&str> = resolved_model.as_deref();

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
                    quota_blocked: false,
                    branch: None,
                    pr_url: None,
                    tool_calls: 0,
                    files_changed: 0,
                })
            },
        };

        let attempt_model = model_for_attempt(preferred, &provider, model_ref);
        match attempt_model {
            Some(m) => println!("    attempt {attempt}: provider `{provider}` model `{m}`"),
            None => println!(
                "    attempt {attempt}: provider `{provider}` (default model — `{}` is {preferred}'s)",
                model.unwrap_or("-")
            ),
        }
        match run_task_once(
            task,
            repo,
            worktrees,
            &provider,
            attempt_model,
            autonomy,
            no_pr,
            config_path,
        )
        .await
        {
            Ok(report) => {
                coordinator.note_solved(&provider);
                // Attribute the outcome to the model that ran the cycle:
                // green = clean, quota/transport = neutral (the model never
                // ran), anything else = defective.
                let attribution = match report.outcome {
                    CycleOutcome::Green => Attribution::Clean,
                    _ if report.quota_blocked => Attribution::Neutral,
                    _ => Attribution::Defective,
                };
                if let Some(m) = attempt_model {
                    quality.record(&provider, m, attribution);
                }
                return Ok(report);
            },
            Err(e) => {
                let text = format!("{e:#}");
                if let Some(m) = attempt_model {
                    let attr = if is_rate_limit(&text) {
                        Attribution::Neutral
                    } else {
                        Attribution::Defective
                    };
                    quality.record(&provider, m, attr);
                }
                if is_rate_limit(&text) {
                    // Honour the provider's own reset window when it gives
                    // one; free tiers reset in under a minute while the
                    // policy cooldown is minutes.
                    if let Some(wait) = parse_retry_delay(&text) {
                        // A hint longer than the cycle interval is a daily
                        // quota reset, not a transient throttle. Sleeping
                        // through it (capped at 30 min per attempt, times
                        // the attempt budget) wedged a task in `running`
                        // for hours while the ledger said idle. Fail the
                        // cycle promptly instead; the next replenish or
                        // rotation retries after the reset.
                        if !should_honor_wait(wait) {
                            // `limit: 0` is not a spent daily quota: the
                            // model has no free-tier allocation at all
                            // (live: gemini-4-argon answered every probe
                            // with limit: 0 for ~12 h of requeue-parks).
                            // The task stays requeued, but the note must
                            // point at the real fix: switch --model.
                            let no_allocation = text.contains("limit: 0");
                            if no_allocation {
                                println!(
                                    "    `{provider}` model has no free-tier allocation (limit: 0) — switch --model"
                                );
                            } else {
                                println!(
                                    "    `{provider}` quota exhausted — resets in {}s; not sleeping, failing the cycle",
                                    wait.as_secs()
                                );
                            }
                            coordinator.note_limited_for(&provider, wait);
                            return Ok(CycleReport {
                                outcome: CycleOutcome::Red,
                                note: if no_allocation {
                                    format!(
                                        "provider `{provider}` model has no free-tier allocation (limit: 0); switch --model — task kept pending"
                                    )
                                } else {
                                    format!(
                                        "provider `{provider}` quota exhausted (reset hint exceeds {}s); task retried after the reset",
                                        MAX_HONORED_WAIT.as_secs()
                                    )
                                },
                                quota_blocked: true,
                                branch: None,
                                pr_url: None,
                                tool_calls: 0,
                                files_changed: 0,
                            });
                        }
                        // The provider told us when it will be back. Wait
                        // for it: rotating to the next provider in the
                        // chain is usually worse, because the chain's other
                        // entries may be unconfigured or out of credit.
                        println!(
                            "    `{provider}` rate-limited — waiting {}s (provider hint), then retrying",
                            wait.as_secs()
                        );
                        coordinator.note_limited_for(&provider, wait);
                        std::thread::sleep(wait + Duration::from_secs(2));
                    } else {
                        println!("    `{provider}` rate-limited — rotating");
                        coordinator.note_limited(&provider);
                    }
                } else {
                    // Not a quota problem: retrying the same provider with
                    // the same inputs cannot help. Report the task as RED,
                    // land it in the estate findings feed (watchers commit
                    // and surface it), and move on to the next task.
                    coordinator.note_failed();
                    let findings = repo.join(".clawdius/maintenance/findings.jsonl");
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&findings)
                    {
                        use std::io::Write;
                        let _ = writeln!(
                            f,
                            "{}",
                            json!({
                                "at": now_rfc3339(),
                                "task": task.id,
                                "provider": provider,
                                "note": text.chars().take(300).collect::<String>(),
                            })
                        );
                    }
                    return Ok(CycleReport {
                        outcome: CycleOutcome::Red,
                        note: text,
                        quota_blocked: false,
                        branch: None,
                        pr_url: None,
                        tool_calls: 0,
                        files_changed: 0,
                    });
                }
                last_error = Some(text);
            },
        }
    }

    Ok(CycleReport {
        outcome: CycleOutcome::Error,
        note: last_error.unwrap_or_else(|| "all attempts failed".to_string()),
        quota_blocked: false,
        branch: None,
        pr_url: None,
        tool_calls: 0,
        files_changed: 0,
    })
}

/// One task attempt in a fresh worktree.
async fn run_task_once(
    task: &Task,
    repo: &Path,
    worktrees: &Path,
    provider: &str,
    model: Option<&str>,
    autonomy: Autonomy,
    no_pr: bool,
    config_path: Option<&Path>,
) -> Result<CycleReport> {
    let branch = format!("maint/{}", sanitize(&task.id));
    let wt = worktrees.join(sanitize(&task.id));

    reset_worktree(repo, &wt, &branch, worktrees)?;
    let base = current_head(repo);

    let result = run_pipeline_in(&wt, task, provider, model, config_path).await;

    // Preserve the cycle's evidence before the worktree disappears: the
    // auto-run journal and the pipeline's stage deliverables are the only
    // record of what the model actually did (live-caught: a red cycle
    // removed the worktree and took the journal with it, so the spin was
    // undiagnosable).
    preserve_artifacts(&wt, repo, &task.id);
    let tool_calls = count_journal_tool_calls(&wt);

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
            // Commit the green work on the branch — or record a no-op:
            // gates green on an untouched tree is a satisfied task (live:
            // the deps cycle found everything current, and "nothing to
            // commit" exits non-zero with its message on STDOUT, which
            // read as an empty stderr failure).
            // The model may have committed inside the tool loop despite
            // the prompt (live: the deps cycle committed a schemars bump,
            // the empty staged set read as "nothing to change", and the
            // branch — with the work — was deleted). Count its commits
            // before calling the cycle a no-op.
            let model_commits = commits_since(base.as_deref(), &wt);
            let _ = Command::new("git")
                .args(["-C", &wt.to_string_lossy()])
                .args(["add", "-A", "--"])
                .args([".", ":(exclude).clawdius", ":(exclude).clawdius/**"])
                .output();
            let staged = dirty_paths(&wt);
            if staged.is_empty() && model_commits == 0 {
                cleanup(false);
                return Ok(CycleReport {
                    outcome: CycleOutcome::Green,
                    note: format!("gates green; nothing to change ({tool_calls} tool calls)"),
                    quota_blocked: false,
                    branch: None,
                    pr_url: None,
                    tool_calls,
                    files_changed: 0,
                });
            }
            if staged.is_empty() && model_commits > 0 {
                println!("    model made {model_commits} commit(s) inside the tool loop");
            }
            // Diff-sanity gate: the compile gates cannot see prose. Every
            // rule here is a live defect with a PR number.
            let problems = diff_sanity_violations(&wt, staged.iter());
            if !problems.is_empty() {
                cleanup(false);
                return Ok(CycleReport {
                    outcome: CycleOutcome::Red,
                    note: format!(
                        "diff-sanity gate: {} — gates cannot see prose, so this \
                         runs before commit",
                        problems.join("; ")
                    ),
                    quota_blocked: false,
                    branch: None,
                    pr_url: None,
                    tool_calls,
                    files_changed: 0,
                });
            }
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
                let stdout = String::from_utf8_lossy(&commit.stdout).trim().to_string();
                cleanup(false);
                return Ok(CycleReport {
                    outcome: CycleOutcome::Red,
                    note: format!(
                        "gates green but commit failed: {stderr}{}",
                        if stdout.is_empty() {
                            String::new()
                        } else {
                            format!(" | stdout: {}", first_line(&stdout))
                        }
                    ),
                    quota_blocked: false,
                    branch: None,
                    pr_url: None,
                    tool_calls,
                    files_changed: 0,
                });
            }

            let files_changed = count_changed_files(repo, base.as_deref(), &wt);

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

            // Autonomy: queue auto-merge when the level and the diff both
            // allow it. `gh pr merge --auto` lets GitHub wait for the PR's
            // own checks, so the decision here is cheap and reversible
            // (a human can cancel auto-merge on the PR page). Protected
            // paths disable it at every level: CI/security config and the
            // proof estate are never touched by an unattended merge.
            let mut note_extra = String::new();
            let changed: Vec<String> = dirty_paths(repo)
                .into_iter()
                .chain(dirty_paths(&wt))
                .collect();
            if pr_url.is_some() && autonomy.may_auto_merge(task.kind) {
                if Autonomy::diff_has_protected_paths(&changed) {
                    note_extra = "; protected paths — human merge".to_string();
                } else {
                    let merged = Command::new("gh")
                        .current_dir(repo)
                        .args(["pr", "merge", &branch, "--auto", "--merge"])
                        .output()
                        .is_ok_and(|o| o.status.success());
                    note_extra = if merged {
                        "; auto-merge queued".to_string()
                    } else {
                        "; auto-merge not queued (gh failed)".to_string()
                    };
                }
            }

            cleanup(true);
            Ok(CycleReport {
                outcome: CycleOutcome::Green,
                note: if push_ok {
                    format!("committed to `{branch}` ({files_changed} file(s) changed){note_extra}")
                } else {
                    format!(
                        "committed to local branch `{branch}` ({files_changed} file(s), no remote push)"
                    )
                },
                quota_blocked: false,
                branch: Some(branch),
                pr_url,
                tool_calls,
                files_changed,
            })
        },
        Err(e) => {
            cleanup(false);
            // Propagate, not Red: the caller distinguishes a quota failure
            // (rotate/park) from a genuine task failure (RED).
            Err(e)
        },
    }
}

/// Diff-sanity gate: catches the "green but wrong" classes the compile
/// gates cannot see. Each rule is a live defect with a PR number:
///
/// - plan dumps: new files under `docs/plans/` (#105 — a hand-typed
///   "dependency upgrade plan" table with a fabricated date, zero real
///   work),
/// - markdown table problems (#104: a row whose pipe count disagrees
///   with its header, and #103: a header plus separator with no data
///   rows)
///
/// Deliberately narrow: these three produced four closed PRs; anything
/// broader would gate honest edits on heuristic guesses.
fn diff_sanity_violations(wt: &Path, staged: impl Iterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut problems = Vec::new();
    let paths: Vec<String> = staged.map(|p| p.as_ref().to_string()).collect();

    // Rule 1: plan dumps. The standing goals forbid scratch/plan files;
    // a new file under docs/plans/ is always one.
    for path in &paths {
        if path.starts_with("docs/plans/") {
            problems.push(format!("plan dump created: {path}"));
        }
    }

    // Rules 2-3 apply to changed markdown files, read from the worktree
    // (the diff alone lacks the surrounding table context).
    for path in &paths {
        let is_md = std::path::Path::new(path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !is_md {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(wt.join(path)) else {
            continue;
        };
        for problem in table_problems(&content) {
            problems.push(format!("{path}: {problem}"));
        }
    }
    problems
}

/// Markdown table problems in the given file content: rows disagreeing
/// with their table header pipe count, and header plus separator tables
/// with no data rows. Fence-aware: pipes inside code fences are not
/// table rows.
fn table_problems(content: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut in_fence = false;
    let mut rows: Vec<(usize, usize)> = Vec::new(); // (line no, pipe count)
    let mut header: Option<(usize, usize)> = None;
    let check_table = |header: &mut Option<(usize, usize)>,
                       rows: &mut Vec<(usize, usize)>,
                       problems: &mut Vec<String>| {
        if let Some((hline, hpipes)) = *header {
            if rows.is_empty() {
                problems.push(format!("line {hline}: table has no data rows"));
            } else {
                for (line, pipes) in rows.iter() {
                    if *pipes != hpipes {
                        problems.push(format!(
                            "line {line}: table row has {pipes} pipes, header has {hpipes}"
                        ));
                    }
                }
            }
        }
        header.take();
        rows.clear();
    };
    for (i, line) in content.lines().enumerate() {
        let line_no = i + 1;
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            check_table(&mut header, &mut rows, &mut problems);
            continue;
        }
        if in_fence {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let pipes = trimmed.matches('|').count();
            // separator row: only pipes, dashes and colons after trimming
            // spaces (the naive trim matches on the outer pipes and keeps
            // the middle ones, misreading `|---|---|` as data).
            let compact: String = trimmed.chars().filter(|c| *c != ' ').collect();
            let is_separator = compact.chars().all(|c| c == '|' || c == '-' || c == ':');
            if header.is_none() {
                header = Some((line_no, pipes));
            } else if is_separator {
                // separator row — keep the header
            } else {
                rows.push((line_no, pipes));
            }
        } else if !trimmed.is_empty() {
            check_table(&mut header, &mut rows, &mut problems);
        }
    }
    check_table(&mut header, &mut rows, &mut problems);
    problems
}

/// Prepare a fresh worktree at `wt` on a new `branch`.
///
/// A killed cycle can leave three kinds of debris, and the order matters:
/// the worktree registration outlives a deleted directory, and a branch
/// that is still "checked out" in that stale registration cannot be
/// deleted — so `worktree remove` (dir gone), then `worktree prune`
/// (clears the registration), and only then `branch -D` (`4887838`-style
/// cleanup, learned live: without the prune, `worktree add -b` aborts
/// with "branch already exists" and the task burns as RED in 0.1 s).
fn reset_worktree(repo: &Path, wt: &Path, branch: &str, worktrees: &Path) -> Result<()> {
    let repo_arg = repo.to_string_lossy().to_string();
    let wt_arg = wt.to_string_lossy().to_string();

    let _ = Command::new("git")
        .args(["-C", &repo_arg, "worktree", "remove", "--force", &wt_arg])
        .output();
    let _ = std::fs::remove_dir_all(wt);
    let _ = Command::new("git")
        .args(["-C", &repo_arg, "worktree", "prune"])
        .output();
    let _ = Command::new("git")
        .args(["-C", &repo_arg, "branch", "-D", branch])
        .output();

    std::fs::create_dir_all(worktrees)?;
    let added = Command::new("git")
        .args(["-C", &repo_arg])
        .args(["worktree", "add", "-b", branch, &wt_arg, "HEAD"])
        .output()
        .context("git worktree add")?;
    if !added.status.success() {
        return Err(anyhow!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&added.stderr).trim()
        ));
    }
    Ok(())
}

/// The repo's current HEAD sha (None when it cannot be determined).
fn current_head(repo: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// How many commits `branch` (checked out in `wt`) has beyond `base`.
fn commits_since(base: Option<&str>, wt: &Path) -> u32 {
    let Some(base) = base else {
        return 0;
    };
    let out = Command::new("git")
        .args([
            "-C",
            &wt.to_string_lossy(),
            "rev-list",
            "--count",
            &format!("{base}..HEAD"),
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}

/// Tool calls recorded in the worktree's preserved auto journal.
fn count_journal_tool_calls(worktree: &Path) -> u32 {
    let path = worktree.join(".clawdius/runs/auto-latest.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return 0;
    };
    u32::try_from(
        text.lines()
            .filter(|l| l.contains("\"t\":\"tool_call\""))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// Files changed on the branch relative to the cycle's base commit.
fn count_changed_files(_repo: &Path, base: Option<&str>, wt: &Path) -> u32 {
    let Some(base) = base else {
        return 0;
    };
    let out = Command::new("git")
        .args([
            "-C",
            &wt.to_string_lossy(),
            "diff",
            "--name-only",
            &format!("{base}..HEAD"),
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => u32::try_from(
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count(),
        )
        .unwrap_or(u32::MAX),
        _ => 0,
    }
}

/// First non-empty line of `text`.
fn first_line(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

/// A goal may be one long line; the PR title needs the first sentence,
/// capped. Live: flash-lite's default goal is a 600-char single line and
/// became the whole PR title.
fn first_sentence_capped(text: &str, cap: usize) -> String {
    let line = first_line(text);
    // Sentences end at . or ; — colons introduce lists, not sentence ends.
    let end = line.find(['.', ';']).map_or(line.len(), |i| i + 1);
    let mut s: String = line.chars().take(end).collect();
    if s.chars().count() > cap {
        s = s.chars().take(cap - 1).collect();
        s.push('…');
    }
    s
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
    let mut pipeline = Pipeline::dev_loop(task.effective_goal().as_str());
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
        // No opencode escalation inside a worktree: opencode resolves the
        // project via git and would edit the main tree.
        false,
    )
    .await;
    std::env::set_current_dir(&previous).ok();
    result
}

/// Open a PR for a green branch; returns the URL when `gh` succeeds.
fn open_pr(repo: &Path, branch: &str, task: &Task) -> Option<String> {
    let title = format!(
        "maint({}): {}",
        task.kind.slug(),
        first_sentence_capped(task.effective_goal().as_str(), 70)
    );
    // Reuse an open PR for this branch: a re-run rotation on the same
    // task id must not pile up duplicate PRs whose heads go stale
    // (live: aileron #21-23 pointed at replaced refs and became
    // undiffable).
    let existing = Command::new("gh")
        .current_dir(repo)
        .args([
            "pr", "list", "--head", branch, "--state", "open", "--limit", "1", "--json", "url",
        ])
        .output()
        .ok();
    if let Some(existing) = existing {
        if let Ok(list) = serde_json::from_slice::<Value>(&existing.stdout) {
            if let Some(url) = list
                .as_array()
                .and_then(|a| a.first())
                .and_then(|p| p.get("url"))
                .and_then(Value::as_str)
            {
                return Some(url.to_string());
            }
        }
    }
    let output = Command::new("gh")
        .current_dir(repo)
        .args(["pr", "create", "--head", branch, "--fill", "--title"])
        .arg(&title)
        .args(["--body"])
        .arg(format!(
            "Automated maintenance cycle.\n\nGoal:\n\n{}\n\nGates: fmt, clippy -D warnings, tests.\n",
            task.effective_goal()
        ))
        .output()
        .ok()?;
    if !output.status.success() {
        // Failure-as-finding: a PR that cannot open silently degrades the
        // whole autonomy ladder (live: aileron's rotation pushed branches
        // but no PRs appeared, and nothing surfaced why). Land the gh
        // stderr in the findings feed before giving up.
        let findings = repo.join(".clawdius/maintenance/findings.jsonl");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&findings)
        {
            use std::io::Write;
            let _ = writeln!(
                f,
                "{}",
                json!({
                    "at": now_rfc3339(),
                    "task": task.id,
                    "provider": "github-pr",
                    "note": format!(
                        "gh pr create failed for {branch}: {}",
                        String::from_utf8_lossy(&output.stderr).trim().chars().take(250).collect::<String>()
                    ),
                })
            );
        }
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
/// Deterministic "would this kind find work?" probes. Only the kinds
/// with a cheap, sound check are probed; docs/tests staleness is
/// semantic, so those kinds always enqueue (their no-op is honest).
fn kind_has_work(kind: TaskKind, repo: &Path) -> bool {
    match kind {
        TaskKind::Lint => !fmt_clean(repo),
        TaskKind::Deps => updates_available(repo),
        _ => true,
    }
}

/// `cargo fmt --check` exits 0 when the tree is already formatted.
fn fmt_clean(repo: &Path) -> bool {
    Command::new("cargo")
        .args(["fmt", "--check"])
        .current_dir(repo)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// `cargo update --dry-run` lists compatible updates as "Updating ..." on
/// stderr (the index line excluded — it is not an update).
fn updates_available(repo: &Path) -> bool {
    Command::new("cargo")
        .args(["update", "--dry-run"])
        .current_dir(repo)
        .output()
        .is_ok_and(|o| {
            let out = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            o.status.success()
                && out
                    .lines()
                    .any(|l| l.starts_with("    Updating ") && !l.contains("crates.io index"))
        })
}

/// How many times a failed task is requeued before the loop leaves it
/// failed. One retry per task per day: enough to recover from a transient
/// provider or environment fault, not enough to spin on a broken goal.
const MAX_TASK_ATTEMPTS: u32 = 2;

/// Give the loop its next unit of work when nothing is pending.
///
/// Order: a kind today's rotation has not run yet, then a retry of a
/// task that failed today. Without the retry the loop idles from the
/// moment today's four kinds are spent — live: eleven hours idle with a
/// failed `docs-2026-11-15` sitting in the backlog, one cycle after the
/// defect that failed it was fixed.
fn replenish_task(backlog: &mut Backlog, repo: &Path, probe: bool) -> Result<Option<Task>> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for kind in REPLENISH_ROTATION {
        let id = format!("{}-{today}", kind.slug());
        if !backlog.tasks.iter().any(|t| t.id == id) {
            // A deterministic "is there work?" probe keeps the rotation
            // honest: a deps task over a lockfile refreshed an hour ago
            // runs gates for twenty minutes to conclude no-op. Fail open:
            // a probe that cannot run never blocks a task.
            if probe && !kind_has_work(kind, repo) {
                continue;
            }
            let mut task = Task::new(id, kind, 5);
            task.goal = kind.default_goal().to_string();
            backlog.push(task.clone())?;
            return Ok(Some(task));
        }
    }
    let retryable = backlog
        .tasks
        .iter_mut()
        .filter(|t| {
            t.status == TaskStatus::Failed
                && t.attempts < MAX_TASK_ATTEMPTS
                && t.id.ends_with(&today)
        })
        .max_by_key(|t| t.priority);
    let Some(retryable) = retryable else {
        return Ok(None);
    };
    retryable.status = TaskStatus::Pending;
    retryable.attempts += 1;
    Ok(Some(retryable.clone()))
}

/// Backlog status for a finished cycle. A quota-blocked failure is the
/// provider's problem, not the task's: requeue it for after the reset.
const fn status_for(outcome: CycleOutcome, quota_blocked: bool) -> TaskStatus {
    match outcome {
        CycleOutcome::Green => TaskStatus::Done,
        CycleOutcome::Idle => TaskStatus::Pending,
        CycleOutcome::Red | CycleOutcome::Error if quota_blocked => TaskStatus::Pending,
        CycleOutcome::Red | CycleOutcome::Error => TaskStatus::Failed,
    }
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

/// Preferred provider first, then configured fallbacks.
///
/// A fallback with no key cannot rescue a cycle: rotating into it turns a
/// transient throttle into a config error that reads like a task failure
/// (live: a Z.AI overload rotated into an exhausted Google key and the
/// cycle reported RED instead of retrying). `ResolvedLlmConfig` is the
/// same resolution the stage uses, so "configured" here means exactly
/// "the stage would find a key".
fn provider_chain(preferred: &str, llm: &LlmConfig) -> Vec<String> {
    let mut chain = vec![preferred.to_string()];
    for fallback in ["google", "anthropic", "deepseek", "zai"] {
        if fallback == preferred {
            continue;
        }
        if clawdius_core::llm::ResolvedLlmConfig::from_config(llm, fallback).is_ok() {
            chain.push(fallback.to_string());
        }
    }
    chain
}

/// Extract the provider's own reset window from an error body.
///
/// Recognises the shapes seen live: `"retryDelay": "37s"` (Gemini),
/// `retry in 37` / `retry after 37` (many gateways), and
/// `Please retry in 37.037196929s`. Capped at 30 minutes so a bogus
/// number cannot park the loop for a day.
fn parse_retry_delay(text: &str) -> Option<Duration> {
    let lower = text.to_lowercase();
    let after = |needle: &str| lower.find(needle).map(|i| &lower[i..]);
    let seconds = after("\"retrydelay\"")
        .or_else(|| after("retry in "))
        .or_else(|| after("retry after "))
        .and_then(first_number)?;
    let wait = Duration::from_secs(seconds.max(1));
    Some(wait.min(Duration::from_mins(30)))
}

/// First unsigned integer at the start of `text` (after skipping
/// non-digits), fractional part ignored.
fn first_number(text: &str) -> Option<u64> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Quota, rate-limit AND capacity errors: all transient provider states
/// that waiting or rotating can fix, unlike a compile failure.
///
/// Capacity shapes were added live: Gemini returned 503 UNAVAILABLE
/// "currently experiencing high demand … Please try again later", which
/// the quota-only matcher let through as a task failure.
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
        "503",
        "service unavailable",
        "unavailable",
        "high demand",
        "overloaded",
        "try again later",
        // Gateway/upstream shapes: a router or gateway in front of the
        // provider reports the provider's hiccup as 502/504. Live: the
        // FreeLLMAPI router turned one empty GLM completion into a 502
        // "provider-side failure", which the matcher let through as a
        // task failure.
        "502",
        "504",
        "bad gateway",
        "gateway timeout",
        "upstream_failed",
        "provider_error",
        "empty completion",
        "empty_completion",
        // Transport shapes: a failed *connection* is the most transient
        // class there is. Live: one reqwest "error sending request for
        // url" turned the deps cycle RED while a direct probe 30 seconds
        // later answered 200 in 0.6s.
        "error sending request",
        "connection refused",
        "connection reset",
        "connection closed",
        "timed out",
        "timeout",
        "os error",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn sleep_or_stop(secs: u64) {
    if secs == 0 {
        return;
    }
    println!("    sleeping {secs}s");
    write_heartbeat(secs);
    std::thread::sleep(Duration::from_secs(secs));
}

/// Liveness heartbeat for external watchers (estate-watch). Written at
/// loop start and before every sleep: a heartbeat older than roughly two
/// intervals means the loop is wedged or dead, not parked.
fn write_heartbeat(next_wake_secs: u64) {
    let Some(repo) = repo_root().ok() else {
        return;
    };
    let path = repo.join(".clawdius/maintenance/heartbeat");
    let now = chrono::Utc::now().timestamp();
    let body = format!("{{\"at\": {now}, \"next_wake_secs\": {next_wake_secs}}}\n");
    if std::fs::write(&path, body).is_err() {
        println!("    (could not write heartbeat at {})", path.display());
    }
}

/// Exclusive single-instance lock over one backlog. Returns the lock
/// file (held for the process lifetime) or `None` when another loop is
/// already running: two live loops on one backlog would both claim
/// tasks, and `reset_stale_running` would unmark the winner's in-flight
/// work. std `File::try_lock` — no new dependencies.
fn acquire_loop_lock(backlog_path: &Path) -> Option<std::fs::File> {
    let lock_path = backlog_path.with_extension("lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(_) => None,
    }
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

/// Paths currently dirty in `repo` (tracked modifications and untracked
/// files alike), from `git status --porcelain`.
fn dirty_paths(repo: &Path) -> std::collections::BTreeSet<String> {
    let output = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "status", "--porcelain"])
        .output();
    match output {
        Ok(out) => parse_porcelain(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => std::collections::BTreeSet::new(),
    }
}

/// Parse `git status --porcelain` output into a set of paths.
///
/// Rename lines (`R  old -> new`) report the destination, because that is
/// the path that exists on disk now.
fn parse_porcelain(text: &str) -> std::collections::BTreeSet<String> {
    let mut paths = std::collections::BTreeSet::new();
    for line in text.lines() {
        if line.len() <= 3 {
            continue;
        }
        let rest = line[3..].trim();
        let path = rest.split(" -> ").last().unwrap_or(rest);
        paths.insert(path.trim_matches('"').to_string());
    }
    paths
}

/// Revert tracked files that became dirty during the cycle.
///
/// Untracked files are reported but never deleted: removing a file we did
/// not create is worse than leaving it for a human.
fn revert_leaks(repo: &Path, before: &std::collections::BTreeSet<String>) -> Vec<String> {
    let after = dirty_paths(repo);
    let mut reverted = Vec::new();
    for path in after.difference(before) {
        let tracked = Command::new("git")
            .args([
                "-C",
                &repo.to_string_lossy(),
                "ls-files",
                "--error-unmatch",
                path,
            ])
            .output()
            .is_ok_and(|o| o.status.success());
        if tracked {
            let _ = Command::new("git")
                .args(["-C", &repo.to_string_lossy(), "checkout", "--", path])
                .output();
            reverted.push(path.clone());
        }
    }
    reverted
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

    /// Regression test for the live failure: a deleted worktree directory
    /// leaves a stale registration that pins the branch; the reset must
    /// still produce a fresh worktree on that branch name.
    #[test]
    fn worktree_reset_survives_a_stale_registration() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        // `git -c` options must come before the subcommand. The closures
        // are hermetic: ambient global/system config (hooks, signing,
        // maintenance) and the shared /tmp tmpfs are the suspected flake
        // sources (#116) — isolate config, and carry stderr into the
        // asserts so a recurrence names its cause instead of panicking
        // with a bare `assertion failed`.
        let git = |args: &[&str]| {
            Command::new("git")
                .args(["-C", repo.to_str().unwrap()])
                .args(["-c", "user.email=t@t", "-c", "user.name=t"])
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_TERMINAL_PROMPT", "0")
                .args(args)
                .output()
                .unwrap()
        };
        let ok = |step: &str, out: &std::process::Output| {
            assert!(
                out.status.success(),
                "git {step} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        ok("init", &git(&["init", "-q"]));
        std::fs::write(repo.join("f.txt"), "x").unwrap();
        ok("add", &git(&["add", "-A"]));
        ok("commit", &git(&["commit", "-qm", "seed"]));

        let wt = repo.join("wt");
        let add = |branch: &str| {
            Command::new("git")
                .args(["-C", repo.to_str().unwrap()])
                .args([
                    "worktree",
                    "add",
                    "-b",
                    branch,
                    wt.to_str().unwrap(),
                    "HEAD",
                ])
                .output()
                .unwrap()
        };
        let added = add("maint/x");
        assert!(
            added.status.success(),
            "first worktree add failed: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        // Simulate a killed cycle: directory gone, registration stale.
        std::fs::remove_dir_all(&wt).unwrap();

        reset_worktree(repo, &wt, "maint/x", &repo.join("worktrees"))
            .expect("reset must clear the stale registration and re-create the worktree");
        assert!(wt.join(".git").exists() || wt.exists());
    }

    #[test]
    fn no_op_cycles_are_green_not_red() {
        // "nothing to commit" exits non-zero with its message on stdout;
        // the empty staged set is what actually decides the outcome.
        let staged = parse_porcelain("");
        assert!(staged.is_empty());
        assert_eq!(
            first_line("On branch main\nnothing to commit"),
            "On branch main"
        );
        assert_eq!(first_line("\n\n"), "");
    }

    #[test]
    fn porcelain_parser_handles_renames_quotes_and_untracked() {
        let text = " M src/lib.rs\n?? .clawdius/maintenance/backlog.json\nR  old.rs -> new.rs\nA  added.rs\n";
        let paths = parse_porcelain(text);
        assert!(paths.contains("src/lib.rs"));
        assert!(paths.contains(".clawdius/maintenance/backlog.json"));
        assert!(paths.contains("new.rs"), "renames report the destination");
        assert!(!paths.contains("old.rs"));
        assert!(paths.contains("added.rs"));
        assert_eq!(paths.len(), 4);
    }

    #[test]
    fn porcelain_parser_ignores_short_and_empty_lines() {
        assert!(parse_porcelain("").is_empty());
        assert!(parse_porcelain("\n\n").is_empty());
        assert!(parse_porcelain("M ").is_empty());
    }

    #[test]
    fn rate_limit_detection_covers_the_real_error_shapes() {
        assert!(is_rate_limit(
            "LLM error: Request failed with status code '429 Too Many Requests'"
        ));
        assert!(is_rate_limit("Insufficient Balance"));
        assert!(is_rate_limit("RESOURCE_EXHAUSTED: quota exceeded"));
        // Capacity errors live-caught on 2026-11-14 (Gemini 503).
        assert!(is_rate_limit(
            "Request failed with status code '503 Service Unavailable'. \
             This model is currently experiencing high demand."
        ));
        assert!(is_rate_limit("model is overloaded, please retry"));
        // Gateway shapes live-caught on 2026-11-15 (router 502).
        assert!(is_rate_limit(
            "Request failed with status code '502 Bad Gateway'. \"code\":\"upstream_failed\""
        ));
        assert!(is_rate_limit("empty completion from GLM-4.5 Flash"));
        // Transport errors live-caught on 2026-11-17 (reqwest send failure).
        assert!(is_rate_limit(
            "Reqwest error: error sending request for url (https://api.z.ai/api/paas/v4/chat/completions)"
        ));
        assert!(is_rate_limit("connection refused (os error 111)"));
        assert!(!is_rate_limit("semantic loop detected"));
        assert!(!is_rate_limit("compile error: expected `;`"));
        assert!(!is_rate_limit("error[E0308]: mismatched types"));
    }

    #[test]
    fn long_reset_hints_fail_fast_instead_of_sleeping() {
        // Live: gemini-4 free tier returned retryDelay=57923s (~16h).
        // Sleeping through it (30-min cap x 3 attempts) wedged a task in
        // `running` for hours while the ledger said idle.
        let sixteen_hours = Duration::from_secs(57_923);
        assert!(!should_honor_wait(sixteen_hours));
        assert!(should_honor_wait(Duration::from_secs(37)));
        assert!(should_honor_wait(MAX_HONORED_WAIT));
        assert!(!should_honor_wait(
            MAX_HONORED_WAIT + Duration::from_secs(1)
        ));
    }

    #[test]
    fn quota_reds_requeue_and_hard_reds_fail() {
        use clawdius_core::maintenance::TaskStatus;
        assert_eq!(status_for(CycleOutcome::Red, true), TaskStatus::Pending);
        assert_eq!(status_for(CycleOutcome::Error, true), TaskStatus::Pending);
        assert_eq!(status_for(CycleOutcome::Red, false), TaskStatus::Failed);
        assert_eq!(status_for(CycleOutcome::Green, false), TaskStatus::Done);
        assert_eq!(status_for(CycleOutcome::Idle, false), TaskStatus::Pending);
    }

    #[test]
    fn pr_titles_take_the_first_sentence_and_cap_the_rest() {
        let goal = "Upgrade dependencies that have newer patch or minor releases, one at \
                    a time, keeping the gates green. Do not cross major versions.";
        // The first sentence exceeds the cap: the cap wins, ellipsized.
        assert_eq!(
            first_sentence_capped(goal, 70),
            "Upgrade dependencies that have newer patch or minor releases, one at …"
        );
        // A short sentence passes through untouched.
        assert_eq!(
            first_sentence_capped("Fix the flaky gate: it reorders.", 200),
            "Fix the flaky gate: it reorders."
        );
    }

    #[test]
    fn retry_delay_reads_the_shapes_providers_actually_send() {
        let gemini = r#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"37s"}]}}"#;
        assert_eq!(parse_retry_delay(gemini), Some(Duration::from_secs(37)));

        let prose = "Please retry in 37.037196929s.";
        assert_eq!(parse_retry_delay(prose), Some(Duration::from_secs(37)));

        let gateway = "Rate limited. Retry after 12 seconds.";
        assert_eq!(parse_retry_delay(gateway), Some(Duration::from_secs(12)));

        assert_eq!(parse_retry_delay("compile error: expected `;`"), None);
    }

    #[test]
    fn retry_delay_is_capped_so_a_bogus_hint_cannot_park_the_loop() {
        let absurd = r#"{"retryDelay":"86400s"}"#;
        assert_eq!(parse_retry_delay(absurd), Some(Duration::from_mins(30)));
    }

    #[test]
    fn provider_chain_starts_with_preference_and_has_no_duplicates() {
        let chain = provider_chain("google", &LlmConfig::default());
        assert_eq!(chain[0], "google");
        let mut seen = std::collections::HashSet::new();
        for p in &chain {
            assert!(seen.insert(p.clone()), "duplicate provider {p}");
        }
    }

    #[test]
    fn provider_chain_skips_providers_with_no_key() {
        // Live: a Z.AI throttle rotated into Google, whose key was
        // exhausted, and the cycle died as a task failure. Unconfigured
        // providers must never enter the rotation.
        let llm = LlmConfig {
            deepseek: Some(clawdius_core::config::ProviderConfig {
                model: None,
                api_key_env: None,
                api_key: Some("sk-test".into()),
                base_url: None,
            }),
            ..LlmConfig::default()
        };
        let chain = provider_chain("zai", &llm);
        assert_eq!(chain[0], "zai");
        assert!(chain.contains(&"deepseek".to_string()));
        // Every fallback is one the stages can authenticate against (the
        // preferred provider is the operator's choice and stays first).
        for p in chain.iter().skip(1) {
            assert!(
                clawdius_core::llm::ResolvedLlmConfig::from_config(&llm, p).is_ok(),
                "{p} entered the chain without credentials"
            );
        }
        // A provider with no config and no env key stays out. Skipped if
        // the test environment exports one, since env wins over config.
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            assert!(!chain.contains(&"anthropic".to_string()));
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
        let first = replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .expect("first replenish");
        assert_eq!(first.kind, TaskKind::Lint);
        assert_eq!(backlog.tasks.len(), 1, "replenish must persist the task");

        let second = replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .expect("second replenish");
        assert_eq!(second.kind, TaskKind::Deps);

        // Every kind for today exists -> nothing left to enqueue.
        for kind in REPLENISH_ROTATION {
            let id = format!("{}-{}", kind.slug(), chrono::Utc::now().format("%Y-%m-%d"));
            if !backlog.tasks.iter().any(|t| t.id == id) {
                backlog.push(Task::new(id, kind, 5)).unwrap();
            }
        }
        assert!(replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .is_none());
    }

    #[test]
    fn replenish_retries_todays_failed_task_once_then_gives_up() {
        // Live: the loop idled for eleven hours with a failed
        // `docs-2026-11-15` in the backlog, one cycle after the defect
        // that failed it was fixed.
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let mut backlog = Backlog::default();
        for kind in REPLENISH_ROTATION {
            let id = format!("{}-{today}", kind.slug());
            let mut task = Task::new(id, kind, 5);
            if kind == TaskKind::Docs {
                task.status = TaskStatus::Failed;
            } else {
                task.status = TaskStatus::Done;
            }
            backlog.push(task).unwrap();
        }

        let retry = replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .expect("failed task is retried");
        assert_eq!(retry.kind, TaskKind::Docs);
        assert_eq!(retry.status, TaskStatus::Pending);
        assert_eq!(retry.attempts, 1);
        // The retry is what the scheduler now sees.
        assert_eq!(
            backlog.next_pending().map(|t| t.id.clone()),
            Some(retry.id.clone())
        );

        // Second failure exhausts the budget: no third run.
        for task in &mut backlog.tasks {
            if task.id == retry.id {
                task.status = TaskStatus::Failed;
            }
        }
        let again = replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .expect("second retry");
        assert_eq!(again.attempts, MAX_TASK_ATTEMPTS);
        for task in &mut backlog.tasks {
            if task.id == again.id {
                task.status = TaskStatus::Failed;
            }
        }
        assert!(replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .is_none());
    }

    #[test]
    fn replenish_ignores_failures_from_previous_days() {
        // Yesterday's failure is stale: the standing rotation recreates
        // the kind today, so retrying it would duplicate work.
        let mut backlog = Backlog::default();
        for kind in REPLENISH_ROTATION {
            let id = format!("{}-2020-01-01", kind.slug());
            let mut task = Task::new(id, kind, 5);
            task.status = TaskStatus::Failed;
            backlog.push(task).unwrap();
        }
        let task = replenish_task(&mut backlog, Path::new("."), false)
            .expect("replenish")
            .expect("today's rotation is enqueued");
        assert_eq!(task.kind, TaskKind::Lint);
        assert!(task
            .id
            .ends_with(&chrono::Utc::now().format("%Y-%m-%d").to_string()));
    }

    #[test]
    fn loop_lock_is_exclusive_and_reusable_after_release() {
        let dir = std::env::temp_dir().join(format!("maintain-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let backlog = dir.join("backlog.json");
        let first = acquire_loop_lock(&backlog).expect("first acquisition");
        assert!(
            acquire_loop_lock(&backlog).is_none(),
            "second loop must not acquire while the first holds it"
        );
        drop(first);
        assert!(
            acquire_loop_lock(&backlog).is_some(),
            "release must allow re-acquisition"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn heartbeat_records_now_and_next_wake() {
        let dir =
            std::env::temp_dir().join(format!("maintain-heartbeat-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hb = dir.join("heartbeat");
        // write_heartbeat resolves the repo via repo_root(); replicate the
        // write here against an explicit path instead.
        let now = chrono::Utc::now().timestamp();
        std::fs::write(&hb, format!("{{\"at\": {now}, \"next_wake_secs\": 300}}\n")).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hb).unwrap()).unwrap();
        assert_eq!(parsed["next_wake_secs"], 300);
        assert!((parsed["at"].as_i64().unwrap() - now).abs() < 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn table_gate_catches_the_live_defect_classes() {
        // Well-formed table: no problems.
        let good = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert!(table_problems(good).is_empty());
        // #104: a replaced row with a broken pipe count.
        let mangled = "| Command | Description |\n|---|---|\n| :x | X |\n| :exit | Exit the TUI |doc` | Generate documentation |\n";
        let problems = table_problems(mangled);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("5 pipes, header has 3"),
            "{problems:?}"
        );
        // #103: header + separator, no data rows.
        let empty = "# Dependency Upgrade Log\n\n| Date | Dep | Result | Notes |\n|------|-----|--------|-------|\n";
        let problems = table_problems(empty);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("no data rows"));
        // Pipes inside a fenced block are not table rows.
        let fenced = "```\n| not | a | table |\n```\n";
        assert!(table_problems(fenced).is_empty());
    }

    #[test]
    fn plan_dumps_are_rejected() {
        // #105: docs/plans/dependency-upgrade-plan.md with a fabricated date.
        let problems = diff_sanity_violations(
            Path::new("/nonexistent"),
            ["docs/plans/dependency-upgrade-plan.md", "README.md"].into_iter(),
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("plan dump"));
    }

    #[test]
    fn autonomy_ladder_gates_auto_merge() {
        assert!(!Autonomy::Manual.may_auto_merge(TaskKind::Lint));
        assert!(Autonomy::Mechanical.may_auto_merge(TaskKind::Lint));
        assert!(Autonomy::Mechanical.may_auto_merge(TaskKind::Tests));
        assert!(!Autonomy::Mechanical.may_auto_merge(TaskKind::Deps));
        assert!(Autonomy::Full.may_auto_merge(TaskKind::Deps));
        // Protected paths disable it at every level.
        assert!(Autonomy::diff_has_protected_paths(&[
            ".github/workflows/ci.yml".into()
        ]));
        assert!(Autonomy::diff_has_protected_paths(&[
            "supply-chain/imports.lock".into()
        ]));
        assert!(Autonomy::diff_has_protected_paths(&[
            ".specs/02_architecture/proofs/host.lean".into()
        ]));
        assert!(Autonomy::diff_has_protected_paths(&["Cargo.toml".into()]));
        assert!(!Autonomy::diff_has_protected_paths(&[
            "docs/maintenance-loop.md".into()
        ]));
    }

    #[test]
    fn autonomy_parses_the_operator_alphabet() {
        use std::str::FromStr;
        assert_eq!(Autonomy::from_str("manual").unwrap(), Autonomy::Manual);
        assert_eq!(
            Autonomy::from_str("mechanical").unwrap(),
            Autonomy::Mechanical
        );
        assert_eq!(Autonomy::from_str("l2").unwrap(), Autonomy::Full);
        assert!(Autonomy::from_str("yolo").is_err());
    }

    #[test]
    fn the_model_flag_only_applies_to_the_preferred_provider() {
        // Live: after a router failure the loop rotated to `google` and
        // still passed `auto`, which genai resolved to the Ollama adapter.
        assert_eq!(
            model_for_attempt("openai-compatible", "openai-compatible", Some("auto")),
            Some("auto")
        );
        assert_eq!(
            model_for_attempt("openai-compatible", "google", Some("auto")),
            None
        );
        assert_eq!(
            model_for_attempt("google", "zai", Some("gemini-3.5-flash")),
            None
        );
        assert_eq!(model_for_attempt("google", "google", None), None);
    }
}
