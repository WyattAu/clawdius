use super::{load_config, OutputFormat};

use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter};
use clawdius_core::SessionManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(super) async fn handle_auto(
    task: String,
    model: Option<String>,
    provider: String,
    max_iterations: Option<usize>,
    run_tests: bool,
    auto_commit: bool,
    fail_on_test_failure: bool,
    _auto_output_format: Option<String>,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::config::RetryConfig;
    use clawdius_core::llm::providers::LlmClient;
    use clawdius_core::llm::{create_provider_with_retry, ResolvedLlmConfig};
    use clawdius_core::modes::AgentMode;
    use clawdius_core::output::{ActionEdit, ActionResult, OutputOptions};
    use std::io;
    use std::process::Command;
    use std::time::Instant;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let config = load_config(config_path.as_deref())?;
    let session_manager = SessionManager::new(&config)?;
    let mut session = session_manager.get_or_create_active()?;

    // Load Auto mode
    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");
    let mode = AgentMode::load_by_name("auto", &modes_dir).unwrap_or(AgentMode::Auto);

    let mut llm_config = ResolvedLlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model.clone_from(m);
    }

    // Crash-safe run journal (opencode-parity: durable runs).
    let journal_path = std::env::current_dir()?.join(".clawdius/runs/auto-latest.jsonl");
    let mut journal = clawdius_core::session::journal::RunJournal::open(&journal_path)?;
    journal.record(&clawdius_core::session::journal::JournalEvent::Started {
        task: task.clone(),
        provider: provider.clone(),
    })?;
    let journal = Arc::new(Mutex::new(journal));

    // Retry wrapper with an autonomous-run window: provider capacity
    // failures (Google 503 high-demand windows last minutes) must be
    // outlasted, not surrendered after ~3 seconds of backoff.
    let llm_client = match create_provider_with_retry(
        &llm_config,
        Some(RetryConfig {
            max_retries: 6,
            initial_delay_ms: 2_000,
            max_delay_ms: 30_000,
            ..RetryConfig::default()
        }),
    ) {
        Ok(client) => client,
        Err(e) => {
            let result = ActionResult::error("auto", task.clone(), e.to_string());
            formatter.format_action_result(&mut io::stdout(), &result)?;
            return Err(e.into());
        },
    };

    let max_iters = max_iterations.unwrap_or(50);
    let start = Instant::now();

    if output_format == OutputFormat::Text {
        println!("🤖 Clawdius Auto Mode");
        println!("Task: {task}");
        println!("Provider: {provider}");
        println!("Max iterations: {max_iters}");
        if run_tests {
            println!("Tests: enabled");
        }
        if auto_commit {
            println!("Auto-commit: enabled");
        }
        println!();
    }

    // Build initial prompt with task

    if output_format == OutputFormat::Text {
        println!("Working...");
    }

    // Host the bounded subagent engine: a real tool loop (read/grep/glob/
    // edit/shell/git/todo/web) against the working tree — not a single
    // text-only call.
    let client: Arc<dyn LlmClient> = Arc::new(llm_client);
    let tool_executor = Arc::new(crate::cli::auto_tools::AutoToolHost::new(
        std::env::current_dir()?,
    ));
    let tools = crate::cli::auto_tools::AutoToolHost::tool_definitions();
    let exec_for_sub = Arc::clone(&tool_executor);
    let journal_for_sub = Arc::clone(&journal);
    let sub_exec: clawdius_core::tools::subagent::SubToolExecutor = {
        let exec = Arc::clone(&exec_for_sub);
        let j = Arc::clone(&journal_for_sub);
        Arc::new(move |name: &str, args: &str| {
            if let Ok(mut run) = j.lock() {
                let _ = run.record(&clawdius_core::session::journal::JournalEvent::ToolCall {
                    n: 0,
                    name: name.to_string(),
                    args: args.to_string(),
                });
            }
            let (out, is_err) = exec.execute(name, args);
            if let Ok(mut run) = j.lock() {
                let _ = run.record(&clawdius_core::session::journal::JournalEvent::ToolResult {
                    n: 0,
                    name: name.to_string(),
                    is_error: is_err,
                });
            }
            (out, is_err)
        })
    };
    let engine = clawdius_core::tools::subagent::SubagentTool::new(
        client,
        tools,
        sub_exec,
        clawdius_core::tools::subagent::SubagentConfig {
            max_iterations: max_iterations.unwrap_or(15),
            ..clawdius_core::tools::subagent::SubagentConfig::default()
        },
    )
    .system_prompt(mode.system_prompt().to_string());

    if output_format == OutputFormat::Text {
        println!("Working...");
    }

    let response = match engine.run(&task).await {
        Ok(resp) => resp,
        Err(e) => {
            let result = ActionResult::error("auto", task.clone(), e.to_string());
            formatter.format_action_result(&mut io::stdout(), &result)?;
            return Err(e.into());
        },
    };

    let duration = start.elapsed();
    let mut tests_passed = true;
    let mut changes_made: Vec<String> = Vec::new();

    // Real changes: the working tree after the agent ran.
    if let Ok(out) = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(std::env::current_dir()?)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        for line in text.lines() {
            if let Some(file) = line.split_whitespace().nth(1) {
                changes_made.push(file.to_string());
            }
        }
    }

    // Run tests if requested
    if run_tests {
        if output_format == OutputFormat::Text {
            println!("\n🧪 Running tests...");
        }

        let test_output = Command::new("cargo")
            .args(["test", "--no-fail-fast"])
            .current_dir(std::env::current_dir()?)
            .output();

        match test_output {
            Ok(output) => {
                if output.status.success() {
                    if output_format == OutputFormat::Text {
                        println!("✅ Tests passed");
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output_format == OutputFormat::Text {
                        println!("❌ Tests failed:\n{stderr}");
                    }
                    tests_passed = false;
                    if fail_on_test_failure {
                        let result = ActionResult::error(
                            "auto",
                            task.clone(),
                            format!("Tests failed: {stderr}"),
                        );
                        formatter.format_action_result(&mut io::stdout(), &result)?;
                        anyhow::bail!("Tests failed and fail_on_test_failure is set");
                    }
                }
            },
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not run tests: {e}");
                }
            },
        }
    }

    // Auto-commit if requested and changes were made
    if auto_commit && !changes_made.is_empty() {
        if output_format == OutputFormat::Text {
            println!("\n📝 Committing changes...");
        }

        let commit_message = format!("auto: {task}");
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(std::env::current_dir()?)
            .output();

        let commit_output = Command::new("git")
            .args(["commit", "-m", &commit_message])
            .current_dir(std::env::current_dir()?)
            .output();

        match commit_output {
            Ok(output) => {
                if output.status.success() {
                    if output_format == OutputFormat::Text {
                        println!("✅ Changes committed");
                    }
                    changes_made.push("Changes committed to git".to_string());
                } else if output_format == OutputFormat::Text {
                    println!("⚠️ Git commit failed (maybe no changes?)");
                }
            },
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not commit: {e}");
                }
            },
        }
    }

    // Save session
    let user_msg = clawdius_core::session::Message::user(&task);
    session_manager
        .add_message(&mut session, user_msg.clone())
        .await?;

    let assistant_msg = clawdius_core::session::Message::assistant(&response);
    session_manager
        .add_message(&mut session, assistant_msg.clone())
        .await?;

    // Journal the run outcome (crash trail: .clawdius/runs/auto-latest.jsonl)
    if let Ok(mut run) = journal.lock() {
        let _ = run.record(&clawdius_core::session::journal::JournalEvent::Finished {
            ok: tests_passed,
            note: format!("auto run in {}ms", duration.as_millis()),
        });
    }

    // Build result
    let result = ActionResult::success(
        "auto",
        task.clone(),
        format!("Auto task completed in {}ms", duration.as_millis()),
        format!("{changes_made:?}"),
        changes_made
            .iter()
            .map(|c| ActionEdit {
                start_line: 0,
                start_column: 0,
                end_line: 0,
                end_column: 0,
                new_text: c.clone(),
            })
            .collect(),
    );

    formatter.format_action_result(&mut io::stdout(), &result)?;

    // Return error code if tests failed and fail_on_test_failure is set
    if !tests_passed && run_tests && fail_on_test_failure {
        std::process::exit(1);
    }

    Ok(())
}
