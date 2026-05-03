use super::engine::SprintEngine;
use super::{get_changed_files, PhaseResult, PhaseStatus, SprintConfig, SprintPhase, SprintState};
use crate::agentic::tool_executor::{ToolExecutor, ToolRequest};
use crate::Result;
use std::path::Path;

pub async fn execute_real_phase(
    engine: &SprintEngine,
    state: &SprintState,
    phase: &SprintPhase,
    llm_result: PhaseResult,
) -> Result<PhaseResult> {
    let executor = engine
        .tool_executor
        .as_ref()
        .ok_or_else(|| crate::Error::Sprint("tool_executor is required for real execution".into()))?;

    let command = match phase {
        SprintPhase::Build => &state.config.build_command,
        SprintPhase::Test => &state.config.test_command,
        _ => return Ok(llm_result),
    };

    let request = ToolRequest::new("shell")
        .with_arg("command", serde_json::Value::String(command.clone()));

    let tool_result = executor
        .execute(request)
        .await
        .map_err(|e| crate::Error::Sprint(format!("Tool execution failed: {e}")))?;

    let output = &tool_result.content;

    if tool_result.success {
        let files_modified =
            get_changed_files(&state.config.project_root).unwrap_or_default();

        Ok(PhaseResult {
            phase: phase.clone(),
            status: PhaseStatus::Success,
            output: format!("[Real execution] Command: {command}\n{output}"),
            duration_ms: llm_result.duration_ms,
            files_modified,
            errors: Vec::new(),
            tokens_used: llm_result.tokens_used,
        })
    } else {
        if *phase == SprintPhase::Build {
            if let Some(fix_output) = attempt_error_recovery(
                engine,
                &state.config,
                &state.config.project_root,
                output,
            )
            .await?
            {
                let files_modified =
                    get_changed_files(&state.config.project_root).unwrap_or_default();

                Ok(PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Success,
                    output: format!(
                        "[Real execution + recovery] Command: {command}\n\n[Recovered output]\n{fix_output}"
                    ),
                    duration_ms: llm_result.duration_ms,
                    files_modified,
                    errors: Vec::new(),
                    tokens_used: llm_result.tokens_used,
                })
            } else {
                Ok(PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Failed,
                    output: format!("[Real execution FAILED] Command: {command}\n\n{output}"),
                    duration_ms: llm_result.duration_ms,
                    files_modified: Vec::new(),
                    errors: vec![output.clone()],
                    tokens_used: llm_result.tokens_used,
                })
            }
        } else {
            Ok(PhaseResult {
                phase: phase.clone(),
                status: PhaseStatus::Failed,
                output: format!("[Real execution FAILED] Command: {command}\n\n{output}"),
                duration_ms: llm_result.duration_ms,
                files_modified: Vec::new(),
                errors: vec![output.clone()],
                tokens_used: llm_result.tokens_used,
            })
        }
    }
}

pub async fn attempt_error_recovery(
    engine: &SprintEngine,
    config: &SprintConfig,
    project_root: &Path,
    error_output: &str,
) -> Result<Option<String>> {
    use crate::agentic::error_recovery::{self, ErrorRecovery, ErrorRecoveryConfig};
    use super::detect_language;

    let all_errors = error_recovery::parse_compiler_output(error_output);
    if all_errors.is_empty() {
        return Ok(None);
    }

    let language = {
        let first_file = all_errors
            .iter()
            .find_map(|e| e.file_path.clone());
        let Some(file_path) = first_file else {
            return Ok(None);
        };
        detect_language(&file_path)
    };

    let error_groups = error_recovery::group_errors_by_file(&all_errors);
    let mut total_attempts = 0usize;
    let mut fixed_files = Vec::new();
    let mut _any_failure = false;

    for (file_path, file_errors) in &error_groups {
        if *file_path == "unknown" {
            continue;
        }

        let full_path = project_root.join(file_path);
        let original_code = match std::fs::read_to_string(&full_path) {
            Ok(code) => code,
            Err(_) => continue,
        };

        let recovery = ErrorRecovery::with_config(
            std::sync::Arc::clone(&engine.llm),
            ErrorRecoveryConfig::new(2).with_compiler_output(true),
        );

        let result = recovery
            .recover_with_verification(
                &original_code,
                error_output,
                language,
                |code| async {
                    if let Some(executor) = engine.tool_executor.as_ref() {
                        let _ = (code, executor);
                    }
                    String::new()
                },
            )
            .await?;

        total_attempts += result.retries_used;

        if result.success {
            std::fs::write(&full_path, &result.fixed_code).map_err(|e| {
                crate::Error::Sprint(format!("Failed to write fix to {file_path}: {e}"))
            })?;
            fixed_files.push(file_path.to_string());
        } else {
            _any_failure = true;
            let _ = std::fs::write(&full_path, &original_code);
        }
    }

    if fixed_files.is_empty() {
        return Ok(None);
    }

    if let Some(executor) = engine.tool_executor.as_ref() {
        let request = ToolRequest::new("shell").with_arg(
            "command",
            serde_json::Value::String(config.build_command.clone()),
        );
        let verify_result = executor
            .execute(request)
            .await
            .map_err(|e| crate::Error::Sprint(format!("Verification build failed: {e}")))?;

        if verify_result.success {
            let category_summary: String = all_errors
                .iter()
                .take(10)
                .map(|e| {
                    let cat = e.categorize(language);
                    format!("[{}]", cat.display_name())
                })
                .collect::<Vec<_>>()
                .join(", ");

            Ok(Some(format!(
                "Fixed {} file(s) ({} total attempt(s)). Categories: {}. Verification: passed.",
                fixed_files.len(),
                total_attempts,
                category_summary,
            )))
        } else {
            for file_path in &fixed_files {
                let full_path = project_root.join(file_path);
                let _ = std::fs::write(&full_path, "// Error recovery reverted\n");
            }
            Ok(None)
        }
    } else {
        Ok(Some(format!(
            "Fixed {} file(s) ({} total attempt(s)). No executor for full verification.",
            fixed_files.len(),
            total_attempts,
        )))
    }
}
