use super::{CheckpointCommands, OutputFormat};

use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter};
use std::path::PathBuf;

pub(super) async fn handle_checkpoint(
    action: CheckpointCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::checkpoint::CheckpointManager;
    use clawdius_core::output::{CheckpointInfo, CheckpointResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let workspace_root = std::env::current_dir()?;
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone())?;

    let result: CheckpointResult = match action {
        CheckpointCommands::Create {
            description,
            session,
        } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());

            match manager
                .create_checkpoint(&session_id, description.clone(), None)
                .await
            {
                Ok(checkpoint) => CheckpointResult::success("create")
                    .with_checkpoint_id(checkpoint.id.clone())
                    .with_session_id(session_id)
                    .with_description(description)
                    .with_file_count(checkpoint.files.len()),
                Err(e) => CheckpointResult::error("create", e.to_string()),
            }
        },

        CheckpointCommands::List {
            session,
            verbose: _,
        } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());

            match manager.list_checkpoints(&session_id) {
                Ok(checkpoints) => {
                    let cp_infos: Vec<CheckpointInfo> = checkpoints
                        .iter()
                        .map(|cp| CheckpointInfo {
                            id: cp.id.clone(),
                            description: cp.description.clone(),
                            timestamp: cp.timestamp,
                            file_count: manager
                                .get_checkpoint(&cp.id)
                                .ok()
                                .flatten()
                                .map_or(0, |c| c.files.len()),
                        })
                        .collect();

                    CheckpointResult::success("list")
                        .with_session_id(session_id)
                        .with_checkpoints(cp_infos)
                },
                Err(e) => CheckpointResult::error("list", e.to_string()),
            }
        },

        CheckpointCommands::Restore { checkpoint_id } => {
            match manager.get_checkpoint(&checkpoint_id)? {
                Some(checkpoint) => match manager.restore_checkpoint(&checkpoint_id).await {
                    Ok(()) => CheckpointResult::success("restore")
                        .with_checkpoint_id(checkpoint_id)
                        .with_description(checkpoint.description)
                        .with_file_count(checkpoint.files.len()),
                    Err(e) => CheckpointResult::error("restore", e.to_string()),
                },
                None => CheckpointResult::error(
                    "restore",
                    format!("Checkpoint not found: {checkpoint_id}"),
                ),
            }
        },

        CheckpointCommands::Compare {
            checkpoint_id1,
            checkpoint_id2,
        } => match manager.compare_checkpoints(&checkpoint_id1, &checkpoint_id2) {
            Ok(diff) => CheckpointResult::success("compare")
                .with_checkpoint_id(format!("{checkpoint_id1} vs {checkpoint_id2}"))
                .with_file_count(diff.file_diffs.len()),
            Err(e) => CheckpointResult::error("compare", e.to_string()),
        },

        CheckpointCommands::Delete { checkpoint_id } => {
            match manager.delete_checkpoint(&checkpoint_id) {
                Ok(()) => CheckpointResult::success("delete").with_checkpoint_id(checkpoint_id),
                Err(e) => CheckpointResult::error("delete", e.to_string()),
            }
        },

        CheckpointCommands::Show { checkpoint_id } => {
            match manager.get_checkpoint(&checkpoint_id)? {
                Some(checkpoint) => CheckpointResult::success("show")
                    .with_checkpoint_id(checkpoint_id)
                    .with_description(checkpoint.description)
                    .with_session_id(checkpoint.session_id)
                    .with_file_count(checkpoint.files.len()),
                None => CheckpointResult::error(
                    "show",
                    format!("Checkpoint not found: {checkpoint_id}"),
                ),
            }
        },

        CheckpointCommands::Cleanup { session, keep } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());
            match manager.cleanup_old_checkpoints(&session_id, keep) {
                Ok(deleted) => CheckpointResult::success("cleanup")
                    .with_session_id(session_id)
                    .with_file_count(deleted),
                Err(e) => CheckpointResult::error("cleanup", e.to_string()),
            }
        },

        CheckpointCommands::Timeline { session } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());
            match manager.get_timeline(&session_id) {
                Ok(timeline) => {
                    let cp_infos: Vec<CheckpointInfo> = timeline
                        .checkpoints
                        .iter()
                        .map(|cp| CheckpointInfo {
                            id: cp.id.clone(),
                            description: cp.description.clone(),
                            timestamp: cp.timestamp,
                            file_count: cp.file_count,
                        })
                        .collect();

                    CheckpointResult::success("timeline")
                        .with_session_id(session_id)
                        .with_checkpoints(cp_infos)
                },
                Err(e) => CheckpointResult::error("timeline", e.to_string()),
            }
        },
    };

    formatter.format_checkpoint_result(&mut io::stdout(), &result)?;

    Ok(())
}
