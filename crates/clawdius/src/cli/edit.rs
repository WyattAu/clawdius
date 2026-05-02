use super::OutputFormat;

pub(super) async fn handle_edit(
    initial: Option<String>,
    editor: Option<String>,
    extension: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::tools::editor::{EditorConfig, ExternalEditor};
    use std::io::{self, Write};

    let config = editor.map_or_else(EditorConfig::default, EditorConfig::with_editor);

    let external_editor = ExternalEditor::new(config);

    if output_format == OutputFormat::Text {
        println!(
            "Opening editor ({}). Save and close to continue...",
            external_editor.editor()
        );
        io::stdout().flush()?;
    }

    let content = match extension {
        Some(ext) => {
            let initial_content = initial.unwrap_or_default();
            external_editor
                .edit_with_extension(&initial_content, &ext)
                .await?
        },
        None => external_editor.edit_prompt(initial.as_deref()).await?,
    };

    if content.trim().is_empty() {
        if output_format == OutputFormat::Text {
            println!("No content provided (empty or only comments).");
        }
        return Ok(());
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "content": content,
                    "length": content.len(),
                    "lines": content.lines().count()
                })
            );
        },
        OutputFormat::Text | OutputFormat::StreamJson => {
            println!("Edited content:\n");
            println!("{content}");
            println!("\n---");
            println!(
                "{} characters, {} lines",
                content.len(),
                content.lines().count()
            );
        },
    }

    Ok(())
}
