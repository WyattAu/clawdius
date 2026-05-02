use super::{OutputFormat, ModeCommands};

use std::path::PathBuf;
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter};

pub(super) async fn handle_modes(
    action: ModeCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::modes::AgentMode;
    use clawdius_core::output::{ModeDetails, ModeInfo, ModesResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");

    let result: ModesResult = match action {
        ModeCommands::List => match AgentMode::list_all(&modes_dir) {
            Ok(modes) => {
                let mode_infos: Vec<ModeInfo> = modes
                    .iter()
                    .map(|(name, description)| ModeInfo {
                        name: name.clone(),
                        description: description.clone(),
                    })
                    .collect();

                ModesResult::success("list").with_modes(mode_infos)
            },
            Err(e) => ModesResult::error("list", e.to_string()),
        },

        ModeCommands::Create { name, output } => {
            let output_path = output.unwrap_or_else(|| modes_dir.join(format!("{name}.toml")));

            if output_path.exists() {
                ModesResult::error(
                    "create",
                    format!("Mode file already exists: {}", output_path.display()),
                )
            } else {
                if let Some(parent) = output_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                let template = format!(
                    r#"name = "{name}"
description = "Custom mode for {name}"
system_prompt = """
You are Clawdius, a custom assistant specialized in {name}.

Add your specific instructions here.
"""
temperature = 0.7
tools = ["file", "shell", "git"]
"#
                );

                match tokio::fs::write(&output_path, template).await {
                    Ok(()) => ModesResult::success("create")
                        .with_mode_name(&name)
                        .with_created_path(output_path.display().to_string()),
                    Err(e) => ModesResult::error("create", e.to_string()),
                }
            }
        },

        ModeCommands::Show { name } => match AgentMode::load_by_name(&name, &modes_dir) {
            Ok(mode) => ModesResult::success("show")
                .with_mode_name(&name)
                .with_mode_details(ModeDetails {
                    name: mode.name().to_string(),
                    description: mode.description().to_string(),
                    system_prompt: mode.system_prompt().to_string(),
                    temperature: mode.temperature(),
                    tools: mode.tools(),
                }),
            Err(e) => ModesResult::error("show", e.to_string()),
        },
    };

    formatter.format_modes_result(&mut io::stdout(), &result)?;

    Ok(())
}
