use super::OutputFormat;

use std::path::{Path, PathBuf};
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};

pub(super) fn handle_refactor(
    _from: String,
    _to: String,
    _path: PathBuf,
    _dry_run: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, RefactorResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let result = RefactorResult::error("Refactor command not yet implemented");

    formatter.format_refactor_result(&mut io::stdout(), &result)?;

    Ok(())
}

pub(super) fn handle_action(
    action: &str,
    file: &PathBuf,
    line: Option<usize>,
    column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::actions::{ActionContext, ActionRegistry, Position};
    use clawdius_core::output::{ActionEdit, ActionResult, OutputOptions};
    use std::fs;
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let document = fs::read_to_string(file)?;
    let language = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt")
        .to_string();

    let position = Position {
        line: line.unwrap_or(0),
        column: column.unwrap_or(0),
    };

    let selection = if let (Some(end_l), Some(end_c)) = (end_line, end_column) {
        let lines: Vec<&str> = document.lines().collect();
        if position.line < lines.len() && end_l < lines.len() {
            if position.line == end_l {
                Some(lines[position.line][position.column..end_c].to_string())
            } else {
                let mut selected_text = String::new();
                for i in position.line..=end_l {
                    if i < lines.len() {
                        if i == position.line {
                            selected_text.push_str(&lines[i][position.column..]);
                        } else if i == end_l {
                            selected_text.push_str(&lines[i][..end_c]);
                        } else {
                            selected_text.push_str(lines[i]);
                        }
                        if i < end_l {
                            selected_text.push('\n');
                        }
                    }
                }
                Some(selected_text)
            }
        } else {
            None
        }
    } else {
        None
    };

    let context = ActionContext {
        document,
        language,
        position,
        selection,
        symbol_at_position: None,
    };

    let registry = ActionRegistry::default();

    let action_impl = match action {
        "extract-function" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.extract.function")
            .ok_or_else(|| anyhow::anyhow!("Extract function action not available"))?,
        "extract-variable" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.extract.variable")
            .ok_or_else(|| anyhow::anyhow!("Extract variable action not available"))?,
        "inline-variable" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.inline.variable")
            .ok_or_else(|| anyhow::anyhow!("Inline variable action not available"))?,
        "rename" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.rename")
            .ok_or_else(|| anyhow::anyhow!("Rename action not available"))?,
        "move-module" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.move.module")
            .ok_or_else(|| anyhow::anyhow!("Move to module action not available"))?,
        "generate-tests" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "source.generate.tests")
            .ok_or_else(|| anyhow::anyhow!("Generate tests action not available"))?,
        _ => {
            anyhow::bail!("Unknown action: {action}");
        },
    };

    let result = match action_impl.execute(&context) {
        Ok(action_result) => {
            let edits: Vec<ActionEdit> = action_result
                .edits
                .iter()
                .map(|edit| ActionEdit {
                    start_line: edit.range.start.line,
                    start_column: edit.range.start.column,
                    end_line: edit.range.end.line,
                    end_column: edit.range.end.column,
                    new_text: edit.new_text.clone(),
                })
                .collect();

            ActionResult::success(
                action,
                file.display().to_string(),
                action_result.title,
                format!("{:?}", action_result.kind),
                edits,
            )
        },
        Err(e) => ActionResult::error(action, file.display().to_string(), e.to_string()),
    };

    formatter.format_action_result(&mut io::stdout(), &result)?;

    Ok(())
}
