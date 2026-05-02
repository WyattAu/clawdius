use super::{OutputFormat, TimelineCommands};

use std::path::{Path, PathBuf};

pub(super) async fn handle_timeline(
    action: TimelineCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::timeline::{CheckpointId, TimelineManager};

    let workspace_root = std::env::current_dir()?;
    let db_path = workspace_root.join(".clawdius/timeline.db");

    let mut manager = TimelineManager::new(&db_path, workspace_root.clone())?;

    match action {
        TimelineCommands::Create { name, description } => {
            let checkpoint_id = if let Some(desc) = description {
                manager
                    .create_checkpoint_with_description(&name, &desc)
                    .await?
            } else {
                manager.create_checkpoint(&name).await?
            };

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "checkpoint_id": checkpoint_id.0,
                        "name": name,
                        "status": "created"
                    })
                );
            } else {
                println!("✓ Timeline checkpoint created");
                println!("  ID: {}", checkpoint_id.0);
                println!("  Name: {name}");
            }
        },

        TimelineCommands::List => {
            let checkpoints: Vec<clawdius_core::timeline::CheckpointInfo> =
                manager.list_checkpoints()?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&checkpoints)?);
            } else if checkpoints.is_empty() {
                println!("No timeline checkpoints found");
            } else {
                println!("Timeline checkpoints:\n");
                for (i, checkpoint) in checkpoints.iter().enumerate() {
                    println!("{}. {}", i + 1, checkpoint.name);
                    println!("   ID: {}", checkpoint.id.0);
                    if let Some(ref desc) = checkpoint.description {
                        println!("   Description: {desc}");
                    }
                    println!("   Created: {}", checkpoint.timestamp);
                    println!("   Files: {}", checkpoint.files_count);
                    println!("   Size: {} bytes", checkpoint.total_size);
                    println!();
                }
            }
        },

        TimelineCommands::Watch {
            debounce_secs,
            ignore,
            max_per_hour,
        } => {
            use clawdius_core::timeline::WatcherConfig;
            use tokio::signal;

            let mut config = WatcherConfig {
                debounce_interval: std::time::Duration::from_secs(debounce_secs),
                max_checkpoints_per_hour: max_per_hour,
                ..Default::default()
            };

            for pattern in ignore {
                config.ignore_patterns.push(pattern);
            }

            let watcher = manager.create_watcher(config.clone());

            println!("Starting file watcher for timeline auto-checkpointing...");
            println!("  Workspace: {}", workspace_root.display());
            println!("  Debounce: {debounce_secs}s");
            println!("  Max checkpoints/hour: {max_per_hour}");
            println!();
            println!("Press Ctrl+C to stop");
            println!();

            let (tx, mut rx) = tokio::sync::mpsc::channel::<(
                Vec<PathBuf>,
                clawdius_core::timeline::ChangeKind,
            )>(100);

            let db_path_clone = db_path.clone();
            let workspace_root_clone = workspace_root.clone();

            let callback = move |paths: Vec<PathBuf>, kind: clawdius_core::timeline::ChangeKind| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send((paths, kind)).await;
                    Ok(())
                }
            };

            watcher.watch(callback).await?;

            let watch_handle = tokio::task::spawn(async move {
                while let Some((paths, kind)) = rx.recv().await {
                    let name = format!("auto-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));

                    let kind_str = match kind {
                        clawdius_core::timeline::ChangeKind::Created => "created",
                        clawdius_core::timeline::ChangeKind::Modified => "modified",
                        clawdius_core::timeline::ChangeKind::Deleted => "deleted",
                        clawdius_core::timeline::ChangeKind::Any => "changed",
                    };

                    let description =
                        format!("Auto-checkpoint: {} file(s) {}", paths.len(), kind_str);

                    let db = db_path_clone.clone();
                    let ws = workspace_root_clone.clone();

                    tokio::task::spawn_blocking(move || {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(async {
                            match TimelineManager::new(&db, ws) {
                                Ok(mut mgr) => {
                                    match mgr
                                        .create_checkpoint_with_description(&name, &description)
                                        .await
                                    {
                                        Ok(_) => {
                                            println!(
                                                "[{}] Checkpoint '{}' created for {} file(s)",
                                                chrono::Local::now().format("%H:%M:%S"),
                                                name,
                                                paths.len()
                                            );
                                        },
                                        Err(e) => {
                                            eprintln!(
                                                "[{}] Failed to create checkpoint: {}",
                                                chrono::Local::now().format("%H:%M:%S"),
                                                e
                                            );
                                        },
                                    }
                                },
                                Err(e) => {
                                    eprintln!(
                                        "[{}] Failed to create timeline manager: {}",
                                        chrono::Local::now().format("%H:%M:%S"),
                                        e
                                    );
                                },
                            }
                        });
                    })
                    .await
                    .ok();
                }
            });

            signal::ctrl_c().await?;
            println!("\nStopping file watcher...");
            watcher.stop().await;
            watch_handle.abort();
        },

        TimelineCommands::Rollback { checkpoint_id } => {
            let id = CheckpointId::from_string(checkpoint_id.clone());

            if let Some(checkpoint) = manager.get_checkpoint(&id)? {
                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "checkpoint_id": checkpoint_id,
                            "name": checkpoint.name,
                            "files_count": checkpoint.files_count,
                            "status": "rolling back"
                        })
                    );
                } else {
                    println!("Rolling back to checkpoint: {checkpoint_id}");
                    println!("  Name: {}", checkpoint.name);
                    println!("  Created: {}", checkpoint.timestamp);
                    println!("  Files: {}", checkpoint.files_count);
                    println!();
                }

                manager.rollback(&id).await?;

                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "checkpoint_id": checkpoint_id,
                            "status": "rolled back"
                        })
                    );
                } else {
                    println!("✓ Checkpoint restored successfully");
                }
            } else {
                anyhow::bail!("Checkpoint not found: {checkpoint_id}");
            }
        },

        TimelineCommands::Diff { from, to } => {
            let from_id = CheckpointId::from_string(from.clone());
            let to_id = CheckpointId::from_string(to.clone());

            let diff = manager.diff(&from_id, &to_id)?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                println!("Diff from {from} to {to}\n");
                println!("Summary:");
                println!("  Files changed: {}", diff.summary.total_files);
                println!("  Additions: {}", diff.summary.total_additions);
                println!("  Deletions: {}", diff.summary.total_deletions);
                println!();

                if diff.files_changed.is_empty() {
                    println!("No differences found");
                } else {
                    println!("Changes:");
                    for file_diff in &diff.files_changed {
                        let prefix = match file_diff.change_type {
                            clawdius_core::timeline::FileChangeType::Added => "+",
                            clawdius_core::timeline::FileChangeType::Modified => "~",
                            clawdius_core::timeline::FileChangeType::Deleted => "-",
                        };
                        println!(
                            "  {} {} (+{}, -{})",
                            prefix,
                            file_diff.path.display(),
                            file_diff.additions,
                            file_diff.deletions
                        );
                    }
                }
            }
        },

        TimelineCommands::History { file } => {
            let history: Vec<clawdius_core::timeline::FileVersion> =
                manager.get_file_history(&file)?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else if history.is_empty() {
                println!("No history found for file: {}", file.display());
            } else {
                println!("History for {}:\n", file.display());
                for version in &history {
                    println!("  Version {} ({})", version.version, version.timestamp);
                    println!("    Checkpoint: {}", version.checkpoint_id.0);
                    println!("    Size: {} bytes", version.size);
                    println!("    Hash: {}", version.checksum);
                    println!();
                }
            }
        },

        TimelineCommands::Delete { checkpoint_id } => {
            let id = CheckpointId::from_string(checkpoint_id.clone());
            manager.delete_checkpoint(&id)?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "checkpoint_id": checkpoint_id,
                        "status": "deleted"
                    })
                );
            } else {
                println!("✓ Checkpoint deleted: {checkpoint_id}");
            }
        },

        TimelineCommands::Cleanup { keep } => {
            let deleted = manager.cleanup_old_checkpoints(keep)?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "deleted_count": deleted,
                        "kept_count": keep,
                        "status": "cleaned up"
                    })
                );
            } else {
                println!("✓ Cleaned up {deleted} old checkpoint(s), keeping {keep} most recent");
            }
        },
    }

    Ok(())
}
