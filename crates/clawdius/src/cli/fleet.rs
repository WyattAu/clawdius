//! Fleet CLI: run opencode tasks in isolated worktrees, collect, and
//! publish them — the operator's terminal entry into the fleet driver
//! ([`clawdius_core::fleet`]).
//!
//! The gateway/chat surfaces (Telegram `work` etc., #100) sit on top of
//! the same primitives; this module is the human-in-the-loop path.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::FleetCommands;
use clawdius_core::fleet::opencode_driver::{FleetTask, OpencodeDriver};

/// Build the driver for a repo.
fn driver_for(repo: impl Into<PathBuf>) -> OpencodeDriver {
    OpencodeDriver::new(repo.into())
}

/// Print a header line and the driver's report.
fn print_report(slug: &str, ok: bool, output: &str, duration_ms: u64) {
    println!(
        "fleet: task `{slug}` {} in {duration_ms}ms",
        if ok { "OK" } else { "FAILED" }
    );
    if !output.trim().is_empty() {
        let tail: String = output
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n  ");
        println!("  last lines:\n  {tail}");
    }
}

/// `fleet log <slug>`: the last assistant message from the task's
/// session log (written by the driver on every finish, including
/// timeouts).
fn handle_log(repo: &Path, slug: &str) -> anyhow::Result<()> {
    let driver = driver_for(repo);
    let path = driver.session_log_path(slug);
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        anyhow::anyhow!(
            "no session log for `{slug}` ({}): {e} — the task may not have finished",
            path.display()
        )
    })?;
    let mut last_text = None;
    for line in raw.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if v.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = v.pointer("/part/text").and_then(|t| t.as_str()) {
                    if !t.trim().is_empty() {
                        last_text = Some(t.to_string());
                    }
                }
            }
        }
    }
    match last_text {
        Some(text) => println!("{text}"),
        None => anyhow::bail!(
            "session log for `{slug}` has no assistant text ({} lines)",
            raw.lines().count()
        ),
    }
    Ok(())
}

/// `fleet sessions`: the session index — every finished task with its
/// outcome, duration, and session id.
fn handle_sessions(repo: &Path) -> anyhow::Result<()> {
    let driver = driver_for(repo);
    let index_path = driver.repo.join(".fleet-sessions").join("index.json");
    let raw = std::fs::read_to_string(&index_path)
        .map_err(|e| anyhow::anyhow!("no session index ({}): {e}", index_path.display()))?;
    let index: std::collections::BTreeMap<String, clawdius_core::fleet::SessionIndexEntry> =
        serde_json::from_str(&raw)?;
    if index.is_empty() {
        println!("no finished tasks yet");
        return Ok(());
    }
    println!("{:<34} {:>6} {:>9}  session", "slug", "ok", "secs");
    for (slug, entry) in &index {
        println!(
            "{:<34} {:>6} {:>9}  {}",
            slug,
            if entry.ok { "ok" } else { "FAIL" },
            entry.duration_ms / 1000,
            entry.session_id.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

pub(super) fn handle_fleet(action: FleetCommands) -> anyhow::Result<()> {
    match action {
        FleetCommands::Work {
            repo,
            slug,
            prompt,
            agent,
            model,
            timeout_secs,
            no_publish,
            keep,
        } => {
            let prompt = if prompt.is_empty() {
                anyhow::bail!("no prompt given — pass it after `--` or as trailing arguments")
            } else {
                prompt.join(" ")
            };
            let driver = driver_for(&repo);
            let task = FleetTask {
                slug: slug.clone(),
                prompt,
                agent,
                model,
                session: None,
                timeout: Duration::from_secs(timeout_secs.unwrap_or(900)),
            };
            println!("fleet: running `{slug}` in a worktree…");
            let report = driver.run(&task)?;
            print_report(&slug, report.ok, &report.output, report.duration_ms);

            if !report.ok {
                // Keep the worktree for inspection; nothing to collect.
                anyhow::bail!(
                    "task failed — worktree kept at {}",
                    report
                        .worktree
                        .as_deref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
            }

            let collect = driver.collect(&slug)?;
            println!(
                "fleet: collected `{slug}` ({} -> {})",
                &collect.head_before[..8.min(collect.head_before.len())],
                &collect.head_after[..8.min(collect.head_after.len())]
            );

            if !no_publish {
                let title = format!("fleet/{slug}: automated task");
                let body = format!(
                    "Automated fleet task `{slug}`.\n\nRan via opencode in {}.",
                    report
                        .worktree
                        .as_deref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
                let published = driver.publish(&slug, "origin", "main", &title, &body);
                match published {
                    Ok(p) => match p.pr_url {
                        Some(url) => println!("fleet: PR opened: {url}"),
                        None => println!(
                            "fleet: pushed {} (gh unavailable — open the PR by hand)",
                            p.slug
                        ),
                    },
                    Err(e) => println!("fleet: publish skipped: {e}"),
                }
            }

            if !keep {
                driver.discard(&slug)?;
                println!("fleet: worktree discarded");
            }
        },

        FleetCommands::Log { repo, slug } => handle_log(&repo, &slug)?,
        FleetCommands::Sessions { repo } => handle_sessions(&repo)?,
        FleetCommands::Collect { slug } => {
            let driver = driver_for(std::env::current_dir()?);
            let report = driver.collect(&slug)?;
            println!(
                "fleet: collected `{}` ({} -> {})",
                report.slug,
                &report.head_before[..8.min(report.head_before.len())],
                &report.head_after[..8.min(report.head_after.len())]
            );
        },

        FleetCommands::Publish {
            slug,
            remote,
            base,
            title,
            body,
        } => {
            let driver = driver_for(std::env::current_dir()?);
            let title = title.unwrap_or_else(|| format!("fleet/{slug}: automated task"));
            let body = body.unwrap_or_else(|| format!("Automated fleet task `{slug}`."));
            let report = driver.publish(&slug, &remote, &base, &title, &body)?;
            match report.pr_url {
                Some(url) => println!("fleet: PR opened: {url}"),
                None => println!("fleet: pushed `{slug}` (gh unavailable — open the PR by hand)"),
            }
        },

        FleetCommands::Status => {
            let cwd = std::env::current_dir()?;
            let driver = driver_for(&cwd);
            let pool = &driver.worktree_root;
            if !pool.is_dir() {
                println!("fleet: no worktree pool at {}", pool.display());
                return Ok(());
            }
            let mut entries: Vec<_> = std::fs::read_dir(pool)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            entries.sort();
            for path in entries {
                let slug = path.file_name().map(|n| n.to_string_lossy().into_owned());
                let Some(slug) = slug else { continue };
                // Branch state: ahead/behind vs the repo's current branch.
                let out = std::process::Command::new("git")
                    .current_dir(&path)
                    .args(["rev-list", "--left-right", "--count", "HEAD...HEAD"])
                    .output();
                let _ = out; // rev-list against itself is a placeholder; branch list is the datum
                println!("fleet: {slug}");
            }
            let branches = std::process::Command::new("git")
                .current_dir(&cwd)
                .args(["branch", "--list", "fleet/*"])
                .output()?;
            for line in String::from_utf8_lossy(&branches.stdout).lines() {
                let b = line.trim_start_matches("* ").trim();
                if !b.is_empty() {
                    println!("fleet: branch {b}");
                }
            }
        },
    }
    Ok(())
}
