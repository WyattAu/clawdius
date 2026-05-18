use super::{load_config, OutputFormat};
use anyhow::Context;
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};
use clawdius_core::MentionResolver;
use clawdius_core::SessionManager;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::cast_possible_truncation)]
pub(super) async fn handle_chat(
    prompt: Option<String>,
    model: Option<String>,
    provider: String,
    _session: Option<String>,
    use_editor: bool,
    mode_name: String,
    exit_after_response: bool,
    quiet_mode: bool,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::llm::providers::LlmClient;
    use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, LlmConfig};
    use clawdius_core::modes::AgentMode;
    use clawdius_core::tools::editor::ExternalEditor;
    use std::io::{self, IsTerminal, Read, Write};
    use std::time::Instant;

    // Determine if we should exit after response (non-interactive mode)
    // Auto-enable if prompt is provided via CLI args
    let non_interactive = exit_after_response || prompt.is_some();

    // Handle message input
    let message = if use_editor {
        let editor = ExternalEditor::default_editor();

        if output_format == OutputFormat::Text && !quiet_mode {
            println!(
                "Opening editor ({}). Save and close to continue...",
                editor.editor()
            );
        }

        let initial_content = prompt.unwrap_or_default();
        editor
            .open_and_edit(&initial_content)
            .map_err(|e| anyhow::anyhow!("Editor error: {e}"))?
    } else if let Some(msg) = prompt {
        // Prompt provided via CLI args
        msg
    } else if !io::stdin().is_terminal() {
        // Read from stdin if not a terminal (piped input)
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            anyhow::bail!("No input provided via stdin. Please pipe content or provide a message argument.\nExample: echo 'Hello' | clawdius chat");
        }
        input.trim().to_string()
    } else if non_interactive {
        anyhow::bail!(
            "Message is required in non-interactive mode.\n\nProvide a message via:\n  - Argument: clawdius chat \"Your message\"\n  - Stdin: echo \"Your message\" | clawdius chat"
        );
    } else {
        anyhow::bail!("Message is required.\n\nOptions:\n  - Use --editor to open your $EDITOR\n  - Provide via argument: clawdius chat \"Your message\"\n  - Pipe via stdin: echo \"Your message\" | clawdius chat");
    };

    if message.trim().is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text && !quiet_mode,
        quiet: quiet_mode,
        include_metadata: output_format == OutputFormat::Text && !quiet_mode,
    };
    let formatter = OutputFormatter::new(options);

    let config = load_config(config_path.as_deref())?;
    let session_manager = SessionManager::new(&config)?;
    let mut session = session_manager.get_or_create_active()?;

    // Load agent mode
    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");
    let mode = AgentMode::load_by_name(&mode_name, &modes_dir)
        .with_context(|| format!("Failed to load mode: {mode_name}"))?;

    let resolver = MentionResolver::new(std::env::current_dir()?);
    let context_items = resolver.resolve_all(&message).await?;

    let context_str = if context_items.is_empty() {
        message.clone()
    } else {
        let items: Vec<String> = context_items
            .iter()
            .map(clawdius_core::ContextItem::to_formatted_string)
            .collect();
        format!(
            "\n\n[Context]\n{}\n\n[User Message]\n{}",
            items.join("\n---\n"),
            message
        )
    };

    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model.clone_from(m);
    }

    let llm_client = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => {
            formatter.format_error(
                &mut io::stderr(),
                &e.to_string(),
                Some(session.id.to_string().as_str()),
            )?;
            return Err(e.into());
        },
    };

    // Build messages with mode-specific system prompt
    let system_message = ChatMessage {
        role: ChatRole::System,
        content: mode.system_prompt().to_string(),
    };

    let user_message = ChatMessage {
        role: ChatRole::User,
        content: context_str.clone(),
    };

    let messages = vec![system_message, user_message];

    if output_format == OutputFormat::Text {
        println!("Provider: {provider}");
        println!("Session: {}", session.id);
        println!("Mode: {} - {}", mode.name(), mode.description());
        println!();
    }

    if output_format == OutputFormat::Text {
        print!("Thinking...");
        io::stdout().flush()?;
    }

    let start = Instant::now();
    let response = match llm_client.chat(messages).await {
        Ok(resp) => resp,
        Err(e) => {
            if output_format == OutputFormat::Text {
                println!();
            }
            formatter.format_error(
                &mut io::stderr(),
                &e.to_string(),
                Some(session.id.to_string().as_str()),
            )?;
            return Err(e.into());
        },
    };
    let duration = start.elapsed();

    if output_format == OutputFormat::Text {
        println!("\x1b[2K\r");
    }

    let user_msg = clawdius_core::session::Message::user(&message);
    session_manager
        .add_message(&mut session, user_msg.clone())
        .await?;

    let assistant_msg = clawdius_core::session::Message::assistant(&response);
    session_manager
        .add_message(&mut session, assistant_msg.clone())
        .await?;

    formatter.format_chat_response(
        &mut io::stdout(),
        &response,
        &session.id.to_string(),
        &provider,
        model.as_deref(),
        0,
        0,
        duration.as_millis() as u64,
    )?;

    Ok(())
}
