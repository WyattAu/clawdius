use super::OutputFormat;
use std::path::PathBuf;

#[allow(clippy::cast_precision_loss)]
pub(super) async fn handle_sprint(
    task: String,
    max_iterations: usize,
    real_execution: bool,
    auto_approve: bool,
    provider: String,
    model: Option<String>,
    browser_qa_url: Option<String>,
    resume: bool,
    lsp_command: Option<String>,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::agentic::sprint::{PhaseStatus, SprintConfig, SprintEngine};
    use clawdius_core::agentic::tool_executor::{ShellToolExecutor, ToolExecutor};
    use clawdius_core::llm::providers::LlmClient;
    use std::sync::Arc;

    let config = clawdius_core::config::Config::load_or_default();

    let llm_config = match clawdius_core::llm::LlmConfig::from_config(&config.llm, &provider) {
        Ok(mut cfg) => {
            // Override model if --model flag was provided
            if let Some(m) = &model {
                cfg.model.clone_from(m);
            }
            cfg
        },
        Err(e) => {
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "error": e.to_string(),
                            "provider": provider,
                        })
                    );
                },
                _ => {
                    eprintln!("Failed to create LLM config for provider '{provider}': {e}");
                },
            }
            return Ok(());
        },
    };

    let provider_instance = match clawdius_core::llm::create_provider(&llm_config) {
        Ok(p) => p,
        Err(e) => {
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": e.to_string(),
                        "provider": provider,
                    })
                );
            } else {
                eprintln!("Failed to create LLM provider '{provider}': {e}");
                eprintln!("Ensure your API key is set (e.g., export ANTHROPIC_API_KEY=...)");
            }
            return Ok(());
        },
    };

    let mut sprint_config = SprintConfig::new(&task);
    sprint_config.max_iterations = max_iterations;
    sprint_config.real_execution = real_execution;
    sprint_config.auto_approve = auto_approve;
    sprint_config.model.clone_from(&model);
    sprint_config.browser_qa_url = browser_qa_url;

    // Build workspace context if available
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Ok(ctx) =
        clawdius_core::workspace::WorkspaceContextBuilder::build_single(&workspace_root, None)
    {
        if !ctx.trim().is_empty() {
            sprint_config.extra_context = Some(ctx);
            if output_format == OutputFormat::Text {
                println!("   Workspace context: loaded");
            }
        }
    }

    let llm: Arc<dyn LlmClient> = Arc::new(provider_instance);
    let tool_executor: Arc<dyn ToolExecutor> =
        Arc::new(ShellToolExecutor::new(workspace_root.clone()));
    let mut engine = SprintEngine::new(llm).with_tool_executor(tool_executor);

    // Attach LSP client if --lsp was specified
    if let Some(lsp_cmd) = &lsp_command {
        use clawdius_core::lsp::{LspClient, LspClientConfig};

        let lsp_config = LspClientConfig::new(lsp_cmd.as_str())
            .with_cwd(workspace_root.clone())
            .with_timeout_ms(30_000);
        let mut lsp_client = LspClient::new(lsp_config);

        match lsp_client
            .start(Some(&workspace_root.to_string_lossy()))
            .await
        {
            Ok(()) => {
                if output_format == OutputFormat::Text {
                    println!("   LSP: {lsp_cmd} (connected)");
                }
                engine = engine.with_lsp_client(lsp_client);
            },
            Err(e) => {
                if output_format == OutputFormat::Json {
                    eprintln!(
                        "{}",
                        serde_json::json!({
                            "warning": format!("LSP client failed to start: {e}"),
                            "lsp_command": lsp_cmd,
                        })
                    );
                } else {
                    eprintln!("⚠️  LSP client '{lsp_cmd}' failed to start: {e}");
                    eprintln!("   Continuing sprint without LSP code intelligence.");
                }
            },
        }
    }

    if output_format == OutputFormat::Text {
        println!("🚀 Starting sprint");
        println!("   Task: {task}");
        println!("   Max iterations: {max_iterations}");
        println!("   Real execution: {real_execution}");
        println!("   Auto-approve: {auto_approve}");
        println!("   Provider: {provider}");
        if let Some(m) = &model {
            println!("   Model: {m}");
        }
        if let Some(url) = &sprint_config.browser_qa_url {
            println!("   Browser QA: {url}");
        }
        println!();
    }

    let result = engine
        .run_with_persistence(sprint_config, resume)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if output_format == OutputFormat::Json {
        let phase_results_json: Vec<serde_json::Value> = result
            .phase_results
            .iter()
            .map(|pr| {
                serde_json::json!({
                    "phase": pr.phase.to_string(),
                    "status": format!("{:?}", pr.status),
                    "duration_ms": pr.duration_ms,
                    "tokens_used": pr.tokens_used,
                    "output": pr.output,
                    "files_modified": pr.files_modified,
                    "errors": pr.errors,
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::json!({
                "success": result.success,
                "phase_results": phase_results_json,
                "total_duration_ms": result.total_duration_ms,
                "summary": result.summary,
                "checkpoint_ref": result.checkpoint_ref,
                "rollback_available": result.rollback_available,
                "metrics": {
                    "total_tokens": result.metrics.total_tokens,
                    "retry_cycles": result.metrics.retry_cycles,
                    "phases_succeeded": result.metrics.phases_succeeded,
                    "phases_failed": result.metrics.phases_failed,
                    "phases_skipped": result.metrics.phases_skipped,
                },
            })
        );
    } else {
        println!("{}", result.summary);
        println!();

        for pr in &result.phase_results {
            let status_icon = match pr.status {
                PhaseStatus::Success => "✅",
                PhaseStatus::Failed => "❌",
                PhaseStatus::Skipped => "⏭️",
            };
            println!(
                "  {status_icon} {} ({:.1}s, {} tokens)",
                pr.phase,
                pr.duration_ms as f64 / 1000.0,
                pr.tokens_used
            );
            if !pr.files_modified.is_empty() {
                for f in &pr.files_modified {
                    println!("      {f}");
                }
            }
            if !pr.errors.is_empty() {
                for e in &pr.errors {
                    println!("      error: {e}");
                }
            }
        }

        println!();
        println!(
            "Total duration: {:.1}s",
            result.total_duration_ms as f64 / 1000.0
        );
        if let Some(ref checkpoint) = result.checkpoint_ref {
            println!("Checkpoint: {checkpoint}");
        }
        if result.rollback_available {
            println!("Rollback available: yes");
        }

        println!();
        println!("{}", result.metrics.report());
    }

    Ok(())
}
