use super::{OutputFormat, load_config};

use anyhow::Context;

pub(super) async fn handle_generate(
    prompt: String,
    files: Option<String>,
    mode: String,
    trust: String,
    test_strategy: Option<String>,
    max_iterations: u32,
    dry_run: bool,
    provider: String,
    model: Option<String>,
    timeout_secs: Option<u64>,
    config_path: Option<std::path::PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use crate::tool_executor::CliToolExecutor;
    use clawdius_core::agentic::{
        AgenticSystem, ApplyWorkflow, GenerationMode, TaskContext, TaskRequest,
        TestExecutionStrategy, TrustLevel,
    };
    use clawdius_core::llm::{create_provider, LlmConfig};
    use clawdius_core::timeout::TimeoutGuard;

    // Set up timeout if specified
    let _timeout_guard = timeout_secs
        .map(|secs| TimeoutGuard::with_label(std::time::Duration::from_secs(secs), "generate"));

    // Parse generation mode
    let generation_mode = match mode.as_str() {
        "single-pass" | "single" => GenerationMode::SinglePass,
        "iterative" => GenerationMode::Iterative { max_iterations },
        "agent" | "agent-based" => GenerationMode::AgentBased {
            max_steps: max_iterations,
            autonomous: false,
        },
        _ => anyhow::bail!("Unknown generation mode: '{mode}'.\n\nAvailable modes:\n  - single-pass: Generate code in one LLM call\n  - iterative: Refine code through multiple iterations\n  - agent: Use autonomous agent-based generation"),
    };

    // Parse trust level
    let trust_level = match trust.to_lowercase().as_str() {
        "low" => TrustLevel::Low,
        "medium" => TrustLevel::Medium,
        "high" => TrustLevel::High,
        _ => anyhow::bail!("Unknown trust level: {trust}. Use: low, medium, high"),
    };

    // Parse test strategy
    let test_exec_strategy = match test_strategy.as_deref() {
        Some("sandboxed") => TestExecutionStrategy::sandboxed(),
        Some("direct") => TestExecutionStrategy::direct_with_rollback(),
        Some("skip") | None => TestExecutionStrategy::Skip,
        Some(s) => anyhow::bail!("Unknown test strategy: {s}. Use: sandboxed, direct, skip"),
    };

    // Parse target files
    let target_files: Vec<String> = files
        .as_ref()
        .map(|f| f.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Show starting info
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "starting",
                    "prompt": prompt,
                    "mode": mode,
                    "trust": trust,
                    "dry_run": dry_run,
                    "target_files": target_files
                })
            );
        },
        OutputFormat::Text => {
            println!("🤖 Clawdius Generate");
            println!("Prompt: {prompt}");
            println!("Mode: {generation_mode:?}");
            println!("Trust: {trust_level:?}");
            println!("Dry run: {dry_run}");
            if !target_files.is_empty() {
                println!("Target files: {target_files:?}");
            }
            println!();
        },
        OutputFormat::StreamJson => {
            println!(
                "{}",
                serde_json::json!({
                    "type": "start",
                    "prompt": prompt,
                    "mode": mode
                })
            );
        },
    }

    // Create task request
    let request = TaskRequest {
        id: uuid::Uuid::new_v4().to_string(),
        description: prompt.clone(),
        target_files,
        mode: generation_mode.clone(),
        test_strategy: test_exec_strategy,
        apply_workflow: ApplyWorkflow::trust_based_with_level(
            trust_level,
            trust_level < TrustLevel::High,
        ),
        context: TaskContext::default(),
        trust_level,
    };

    // Handle dry-run mode early (no LLM client needed)
    if dry_run {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "dry_run",
                        "message": "Would execute task",
                        "task": request.description,
                        "config": {
                            "mode": mode,
                            "trust": trust,
                            "test_strategy": test_strategy,
                            "max_iterations": max_iterations
                        }
                    })
                );
            },
            OutputFormat::Text => {
                println!("[DRY RUN] Would execute task: {}", request.description);
                println!();
                println!("Configuration:");
                println!("  Mode: {generation_mode:?}");
                println!("  Trust: {trust_level:?}");
                println!("  Test Strategy: {test_exec_strategy:?}");
                println!("  Apply Workflow: {:?}", request.apply_workflow);
                if !request.target_files.is_empty() {
                    println!("  Target Files: {:?}", request.target_files);
                }
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "dry_run",
                        "task": request.description
                    })
                );
            },
        }
        return Ok(());
    }

    // Load config and create LLM client (only when not in dry-run mode)
    let show_progress = output_format == OutputFormat::Text;

    if show_progress {
        crate::cli_progress::status("Loading configuration...");
    }
    let config = load_config(config_path.as_deref())?;

    if show_progress {
        crate::cli_progress::status("Creating LLM client...");
    }
    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model = m.clone();
    }

    let llm_client = std::sync::Arc::new(create_provider(&llm_config)?);

    let workspace_root = std::env::current_dir().context("Failed to determine workspace root")?;
    let tool_executor = std::sync::Arc::new(CliToolExecutor::new(workspace_root));

    // Create agentic system
    let apply_workflow =
        ApplyWorkflow::trust_based_with_level(trust_level, trust_level < TrustLevel::High);

    let mut system = AgenticSystem::new(generation_mode, test_exec_strategy, apply_workflow)
        .with_llm_client(llm_client)
        .with_tool_executor(tool_executor);

    // Execute the task
    if show_progress {
        crate::cli_progress::status("Executing task...");
    }
    let task_result = system.execute(request).await?;

    // Format output based on format
    match output_format {
        OutputFormat::Json => {
            let changes: Vec<serde_json::Value> = task_result
                .changes
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "path": c.path,
                        "change_type": format!("{:?}", c.change_type),
                        "lines_added": c.new.lines().count(),
                        "lines_removed": c.original.as_ref().map_or(0, |o| o.lines().count()),
                        "diff": c.diff
                    })
                })
                .collect();

            let issues: Vec<serde_json::Value> = task_result
                .verification
                .issues
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "severity": format!("{:?}", i.severity),
                        "message": i.message,
                        "file": i.file,
                        "is_blocking": i.is_blocking()
                    })
                })
                .collect();

            println!(
                "{}",
                serde_json::json!({
                    "status": if task_result.success { "success" } else { "failed" },
                    "task_id": task_result.id,
                    "duration_ms": task_result.duration_ms,
                    "changes": changes,
                    "issues": issues,
                    "test_result": task_result.test_result.as_ref().map(|t| serde_json::json!({
                        "passed": t.passed,
                        "output": t.output
                    })),
                    "rollback_checkpoint": task_result.rollback_checkpoint,
                    "log_entries": task_result.log.len()
                })
            );
        },
        OutputFormat::Text => {
            if task_result.success {
                println!("✅ Task completed successfully!");
            } else {
                println!("❌ Task failed");
            }

            println!("Task ID: {}", task_result.id);
            println!("Duration: {}ms", task_result.duration_ms);

            if !task_result.changes.is_empty() {
                println!("\n📝 Changes ({} files):", task_result.changes.len());
                for change in &task_result.changes {
                    let change_icon = match change.change_type {
                        clawdius_core::agentic::ChangeType::Created => "➕",
                        clawdius_core::agentic::ChangeType::Modified => "✏️",
                        clawdius_core::agentic::ChangeType::Deleted => "🗑️",
                    };
                    println!(
                        "  {} {} ({})",
                        change_icon,
                        change.path,
                        format!("{:?}", change.change_type).to_lowercase()
                    );
                    println!(
                        "    +{} -{}",
                        change.new.lines().count(),
                        change
                            .original
                            .as_ref()
                            .map_or(0, |o| o.lines().count())
                    );
                }
            }

            if !task_result.verification.issues.is_empty() {
                println!("\n⚠️  Issues ({}):", task_result.verification.issues.len());
                for issue in &task_result.verification.issues {
                    let severity_icon = match issue.severity {
                        clawdius_core::agentic::IssueSeverity::Critical => "🔴",
                        clawdius_core::agentic::IssueSeverity::Blocking => "❌",
                        clawdius_core::agentic::IssueSeverity::Warning => "⚠️",
                        clawdius_core::agentic::IssueSeverity::Info => "ℹ️",
                    };
                    println!(
                        "  {} [{:?}] {}",
                        severity_icon, issue.severity, issue.message
                    );
                    println!("     File: {}", issue.file);
                }
            }

            if let Some(ref test_result) = task_result.test_result {
                println!("\n🧪 Test Results:");
                println!("  Passed: {}", test_result.passed);
                if !test_result.output.is_empty() {
                    println!("  Output: {}", test_result.output);
                }
            }

            if let Some(ref checkpoint) = task_result.rollback_checkpoint {
                println!("\n💾 Rollback checkpoint: {checkpoint}");
            }
        },
        OutputFormat::StreamJson => {
            // Stream each change as an event
            for change in &task_result.changes {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "change",
                        "path": change.path,
                        "change_type": format!("{:?}", change.change_type)
                    })
                );
            }

            // Stream final result
            println!(
                "{}",
                serde_json::json!({
                    "type": "complete",
                    "success": task_result.success,
                    "duration_ms": task_result.duration_ms,
                    "changes_count": task_result.changes.len(),
                    "issues_count": task_result.verification.issues.len()
                })
            );
        },
    }

    Ok(())
}
