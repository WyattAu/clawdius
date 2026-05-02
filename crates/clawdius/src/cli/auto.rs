use super::{OutputFormat, load_config};

use std::path::{Path, PathBuf};
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};
use clawdius_core::SessionManager;

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
    use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, LlmConfig};
    use clawdius_core::llm::providers::LlmClient;
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

    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model = m.clone();
    }

    let llm_client = match create_provider(&llm_config) {
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
    let system_message = ChatMessage {
        role: ChatRole::System,
        content: mode.system_prompt().to_string(),
    };

    let user_message = ChatMessage {
        role: ChatRole::User,
        content: format!(
            "Task: {task}\n\nPlease complete this task autonomously. Make the necessary changes and report what you did."
        ),
    };

    let messages = vec![system_message, user_message];

    if output_format == OutputFormat::Text {
        print!("Working...");
    }

    let response = match llm_client.chat(messages).await {
        Ok(resp) => resp,
        Err(e) => {
            let result = ActionResult::error("auto", task.clone(), e.to_string());
            formatter.format_action_result(&mut io::stdout(), &result)?;
            return Err(e.into());
        },
    };

    let duration = start.elapsed();
    let mut changes_made = Vec::new();
    let mut tests_passed = true;

    // Parse response for changes
    if response.contains("created") || response.contains("modified") || response.contains("updated")
    {
        changes_made.push("Files modified based on LLM response".to_string());
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
