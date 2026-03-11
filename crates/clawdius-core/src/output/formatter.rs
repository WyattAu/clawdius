//! High-level output formatter for CLI commands

use chrono::Utc;
use std::io::Write;

use super::{
    ActionResult, BrokerResult, CheckpointResult, ComplianceResult, ConfigResult, ContextResult,
    IndexResult, InitResult, JsonOutput, MetricsResult, ModesResult, OutputFormat, OutputOptions,
    RefactorResult, ResearchResult, StreamEvent, StreamWriter, TelemetryResult, TestResult,
    VerifyResult,
};

pub struct OutputFormatter {
    options: OutputOptions,
}

impl OutputFormatter {
    #[must_use]
    pub fn new(options: OutputOptions) -> Self {
        Self { options }
    }

    #[must_use]
    pub fn format(&self) -> OutputFormat {
        self.options.format
    }

    pub fn format_chat_response<W: Write>(
        &self,
        writer: &mut W,
        content: &str,
        session_id: &str,
        provider: &str,
        model: Option<&str>,
        tokens_input: usize,
        tokens_output: usize,
        duration_ms: u64,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let output = JsonOutput::success(content, session_id)
                    .with_usage(super::TokenUsageInfo::new(tokens_input, tokens_output))
                    .with_duration(duration_ms);

                let json = if self.options.quiet {
                    output.to_json_compact()?
                } else {
                    output.to_json()?
                };

                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer = StreamWriter::new(writer, OutputFormat::StreamJson);

                stream_writer.write_event(&StreamEvent::start(
                    session_id,
                    model.map(std::string::ToString::to_string),
                ))?;

                for token in content.split_whitespace() {
                    stream_writer.write_event(&StreamEvent::token(format!("{token} ")))?;
                }

                stream_writer.write_event(&StreamEvent::complete(
                    tokens_input,
                    tokens_output,
                    duration_ms,
                ))?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if self.options.include_metadata {
                    if let Some(m) = model {
                        writeln!(writer, "Provider: {provider}")?;
                        writeln!(writer, "Model: {m}")?;
                    } else {
                        writeln!(writer, "Provider: {provider}")?;
                    }
                    writeln!(writer, "Session: {session_id}")?;
                    writeln!(writer)?;
                }

                writeln!(writer, "{content}")?;

                if self.options.include_metadata && (tokens_input > 0 || tokens_output > 0) {
                    writeln!(writer)?;
                    writeln!(
                        writer,
                        "Tokens: {} input, {} output ({} total)",
                        tokens_input,
                        tokens_output,
                        tokens_input + tokens_output
                    )?;
                    writeln!(writer, "Duration: {duration_ms}ms")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_error<W: Write>(
        &self,
        writer: &mut W,
        error: &str,
        session_id: Option<&str>,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let output = JsonOutput::error(error, session_id.unwrap_or("none"));

                let json = if self.options.quiet {
                    output.to_json_compact()?
                } else {
                    output.to_json()?
                };

                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer = StreamWriter::new(writer, OutputFormat::StreamJson);
                stream_writer.write_event(&StreamEvent::error(error, "error"))?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                writeln!(writer, "Error: {error}")?;
            }
        }

        Ok(())
    }

    pub fn format_session_list<W: Write>(
        &self,
        writer: &mut W,
        sessions: &[SessionInfo],
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&sessions)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                for (idx, session) in sessions.iter().enumerate() {
                    let event = StreamEvent::Progress {
                        message: format!(
                            "{} - {}",
                            session.id,
                            session.title.as_deref().unwrap_or("Untitled")
                        ),
                        current: idx + 1,
                        total: sessions.len(),
                    };
                    stream_writer.write_event(&event)?;
                }
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if sessions.is_empty() {
                    writeln!(writer, "No sessions found.")?;
                    return Ok(());
                }

                writeln!(writer, "Sessions ({}):", sessions.len())?;
                for session in sessions {
                    let title = session.title.as_deref().unwrap_or("Untitled");
                    writeln!(
                        writer,
                        "  {} - {} ({} messages, {} tokens)",
                        session.id, title, session.message_count, session.tokens
                    )?;
                }
            }
        }

        Ok(())
    }

    pub fn format_tool_result<W: Write>(
        &self,
        writer: &mut W,
        tool_name: &str,
        result: &str,
        success: bool,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "type": "tool_result",
                    "tool": tool_name,
                    "result": result,
                    "success": success,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                writeln!(writer, "{}", serde_json::to_string_pretty(&output)?)?;
            }
            OutputFormat::StreamJson => {
                let event = if success {
                    StreamEvent::tool_result(
                        tool_name,
                        serde_json::Value::String(result.to_string()),
                        true,
                    )
                } else {
                    StreamEvent::error(result, tool_name)
                };
                let mut stream_writer = StreamWriter::new(writer, OutputFormat::StreamJson);
                stream_writer.write_event(&event)?;
            }
            OutputFormat::Text => {
                let status = if success { "✓" } else { "✗" };
                writeln!(writer, "{status} {tool_name}: {result}")?;
            }
        }

        Ok(())
    }

    pub fn format_init_result<W: Write>(
        &self,
        writer: &mut W,
        result: &InitResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(format!("Initialized in {}", result.path), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "✓ Initialized Clawdius in {}", result.path)?;
                    writeln!(writer, "  Created: {}", result.config_path)?;
                    writeln!(writer)?;

                    if result.onboarding_complete {
                        writeln!(writer, "✓ Clawdius is configured and ready!")?;
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Initialization failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_config_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ConfigResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress("Configuration loaded".to_string(), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Configuration from: {}", result.config_path)?;
                    writeln!(writer, "{}", serde_json::to_string_pretty(&result.config)?)?;
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Failed to load configuration: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_metrics_result<W: Write>(
        &self,
        writer: &mut W,
        result: &MetricsResult,
        reset: bool,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!("Metrics: {} requests", result.requests_total),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                writeln!(writer, "Clawdius Metrics:")?;
                writeln!(writer, "  Requests: {}", result.requests_total)?;
                writeln!(writer, "  Errors: {}", result.requests_errors)?;
                writeln!(writer, "  Avg Latency: {:.2}ms", result.avg_latency_ms)?;
                writeln!(writer, "  Tokens Used: {}", result.tokens_used)?;

                if reset {
                    writeln!(writer, "Metrics reset.")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_verify_result<W: Write>(
        &self,
        writer: &mut W,
        result: &VerifyResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!(
                        "Verification: {}",
                        if result.success { "passed" } else { "failed" }
                    ),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                writeln!(writer, "Lean 4 Proof Verification")?;
                writeln!(writer, "=========================")?;
                writeln!(writer)?;

                if result.success {
                    writeln!(
                        writer,
                        "✓ Verification succeeded in {}ms",
                        result.duration_ms
                    )?;
                } else {
                    writeln!(writer, "✗ Verification failed in {}ms", result.duration_ms)?;
                    writeln!(writer)?;

                    if !result.errors.is_empty() {
                        writeln!(writer, "Errors ({}):", result.errors.len())?;
                        for error in &result.errors {
                            writeln!(
                                writer,
                                "  {}:{} - {}",
                                error.line, error.column, error.message
                            )?;
                        }
                    }

                    if !result.warnings.is_empty() {
                        writeln!(writer)?;
                        writeln!(writer, "Warnings ({}):", result.warnings.len())?;
                        for warning in &result.warnings {
                            writeln!(writer, "  {warning}")?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn format_refactor_result<W: Write>(
        &self,
        writer: &mut W,
        result: &RefactorResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!("Refactor: {} files changed", result.files_changed.len()),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    if result.dry_run {
                        writeln!(writer, "Refactor Preview (dry run):")?;
                    } else {
                        writeln!(writer, "Refactor Complete:")?;
                    }
                    writeln!(
                        writer,
                        "  {} -> {}",
                        result.from_language, result.to_language
                    )?;
                    writeln!(writer, "  Input: {}", result.input_path)?;
                    writeln!(writer)?;

                    if !result.files_changed.is_empty() {
                        writeln!(writer, "Files changed:")?;
                        for file in &result.files_changed {
                            writeln!(writer, "  {} ({})", file.original_path, file.change_type)?;
                            if file.lines_added > 0 || file.lines_removed > 0 {
                                writeln!(
                                    writer,
                                    "    +{} -{}",
                                    file.lines_added, file.lines_removed
                                )?;
                            }
                        }
                    }

                    if !result.message.is_empty() {
                        writeln!(writer)?;
                        writeln!(writer, "{}", result.message)?;
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Refactor failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_broker_result<W: Write>(
        &self,
        writer: &mut W,
        result: &BrokerResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(format!("Broker: {}", result.status), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Broker Mode Activated")?;
                    writeln!(writer, "  Status: {}", result.status)?;
                    writeln!(
                        writer,
                        "  Paper Trade: {}",
                        if result.paper_trade {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )?;

                    if let Some(config_path) = &result.config_path {
                        writeln!(writer, "  Config: {config_path}")?;
                    }

                    if !result.message.is_empty() {
                        writeln!(writer)?;
                        writeln!(writer, "{}", result.message)?;
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Broker failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_compliance_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ComplianceResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!("Compliance: {} standards", result.standards.len()),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Compliance Matrix Generated")?;
                    writeln!(writer, "  Standards: {}", result.standards.join(", "))?;
                    writeln!(writer, "  Project: {}", result.project_path)?;
                    writeln!(writer, "  Format: {}", result.output_format)?;

                    if let Some(output_path) = &result.output_path {
                        writeln!(writer, "  Output: {output_path}")?;
                    }

                    writeln!(writer)?;
                    writeln!(writer, "{}", serde_json::to_string_pretty(&result.matrix)?)?;
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Compliance generation failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_research_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ResearchResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!("Research: {} concepts found", result.concepts.len()),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Research Results for: {}", result.query)?;
                    writeln!(writer, "Confidence: {:.2}%", result.confidence * 100.0)?;
                    writeln!(writer, "Languages covered: {:?}", result.languages_covered)?;
                    writeln!(writer)?;

                    if result.concepts.is_empty() {
                        writeln!(writer, "No concepts found.")?;
                        writeln!(
                            writer,
                            "Note: Knowledge graph is empty. Add research sources first."
                        )?;
                    } else {
                        writeln!(writer, "Found {} concepts:", result.concepts.len())?;
                        for concept in &result.concepts {
                            writeln!(
                                writer,
                                "  [{}] {} - {}",
                                concept.language, concept.name, concept.definition
                            )?;
                        }
                    }

                    if !result.relationships.is_empty() {
                        writeln!(writer)?;
                        writeln!(
                            writer,
                            "Found {} relationships:",
                            result.relationships.len()
                        )?;
                        for edge in &result.relationships {
                            writeln!(
                                writer,
                                "  {} -> {} -> {}",
                                edge.from, edge.relationship, edge.to
                            )?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Research failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_action_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ActionResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(format!("Action: {}", result.action), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Action: {}", result.title)?;
                    writeln!(writer, "Kind: {}", result.kind)?;
                    writeln!(writer)?;

                    if !result.edits.is_empty() {
                        writeln!(writer, "Edits:")?;
                        for (i, edit) in result.edits.iter().enumerate() {
                            writeln!(
                                writer,
                                "\n{}. Range: {}:{} - {}:{}",
                                i + 1,
                                edit.start_line,
                                edit.start_column,
                                edit.end_line,
                                edit.end_column
                            )?;
                            writeln!(writer, "   New text:")?;
                            for line in edit.new_text.lines() {
                                writeln!(writer, "     {line}")?;
                            }
                        }
                        writeln!(writer)?;
                        writeln!(
                            writer,
                            "Note: This is a preview. Apply edits manually or use --apply flag."
                        )?;
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Action failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_test_result<W: Write>(
        &self,
        writer: &mut W,
        result: &TestResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(
                    format!("Generated {} test cases", result.test_cases.len()),
                    1,
                    1,
                );
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    if let Some(func) = &result.function {
                        writeln!(writer, "Generated tests for function: {func}")?;
                    } else {
                        writeln!(writer, "Generated tests for file: {}", result.file)?;
                    }

                    writeln!(
                        writer,
                        "\nGenerated {} test cases:",
                        result.test_cases.len()
                    )?;
                    for test in &result.test_cases {
                        writeln!(writer, "\n  - {}:", test.name)?;
                        writeln!(writer, "    {}", test.description)?;
                    }

                    if let Some(output_path) = &result.output_path {
                        writeln!(writer, "\n✓ Tests written to: {output_path}")?;
                    } else {
                        writeln!(writer, "\n--- Generated Tests ---\n")?;
                        for test in &result.test_cases {
                            writeln!(writer, "// {}", test.description)?;
                            writeln!(writer, "{}\n", test.code)?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Test generation failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_index_result<W: Write>(
        &self,
        writer: &mut W,
        result: &IndexResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event =
                    StreamEvent::progress(format!("Indexed {} files", result.files_indexed), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Indexing Complete:")?;
                    writeln!(writer, "  Files indexed: {}", result.files_indexed)?;
                    writeln!(writer, "  Symbols found: {}", result.symbols_found)?;
                    writeln!(writer, "  References found: {}", result.references_found)?;
                    writeln!(
                        writer,
                        "  Embeddings created: {}",
                        result.embeddings_created
                    )?;
                    writeln!(writer, "  Duration: {}ms", result.duration_ms)?;

                    if !result.errors.is_empty() {
                        writeln!(writer, "\nErrors ({}):", result.errors.len())?;
                        for error in &result.errors {
                            writeln!(writer, "  - {error}")?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Indexing failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_context_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ContextResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event =
                    StreamEvent::progress(format!("Gathered {} tokens", result.total_tokens), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Context gathered for: \"{}\"", result.query)?;
                    writeln!(writer, "Max tokens: {}", result.max_tokens)?;
                    writeln!(writer)?;
                    writeln!(writer, "Context gathered:")?;
                    writeln!(writer, "  Files: {}", result.files.len())?;
                    writeln!(writer, "  Symbols: {}", result.symbols.len())?;
                    writeln!(writer, "  Total tokens: {}", result.total_tokens)?;
                    writeln!(writer)?;

                    if !result.files.is_empty() {
                        writeln!(writer, "Files:")?;
                        for file in &result.files {
                            writeln!(writer, "  - {} ({} tokens)", file.path, file.token_count)?;
                        }
                        writeln!(writer)?;
                    }

                    if !result.symbols.is_empty() {
                        writeln!(writer, "Symbols:")?;
                        for symbol in &result.symbols {
                            writeln!(
                                writer,
                                "  - {} [{}] at {} ({} tokens)",
                                symbol.name, symbol.kind, symbol.location, symbol.token_count
                            )?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Context gathering failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_checkpoint_result<W: Write>(
        &self,
        writer: &mut W,
        result: &CheckpointResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event =
                    StreamEvent::progress(format!("Checkpoint: {}", result.operation), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    match result.operation.as_str() {
                        "create" => {
                            writeln!(writer, "✓ Checkpoint created")?;
                            if let Some(id) = &result.checkpoint_id {
                                writeln!(writer, "  ID: {id}")?;
                            }
                            if let Some(desc) = &result.description {
                                writeln!(writer, "  Description: {desc}")?;
                            }
                            if let Some(count) = result.file_count {
                                writeln!(writer, "  Files: {count}")?;
                            }
                        }
                        "list" => {
                            if result.checkpoints.is_empty() {
                                writeln!(writer, "No checkpoints found")?;
                            } else {
                                writeln!(writer, "Checkpoints:\n")?;
                                for (i, cp) in result.checkpoints.iter().enumerate() {
                                    writeln!(writer, "{}. {}", i + 1, cp.id)?;
                                    writeln!(writer, "   Description: {}", cp.description)?;
                                    writeln!(writer, "   Files: {}", cp.file_count)?;
                                }
                            }
                        }
                        "restore" => {
                            writeln!(writer, "✓ Checkpoint restored")?;
                            if let Some(id) = &result.checkpoint_id {
                                writeln!(writer, "  ID: {id}")?;
                            }
                        }
                        "delete" => {
                            writeln!(writer, "✓ Checkpoint deleted")?;
                            if let Some(id) = &result.checkpoint_id {
                                writeln!(writer, "  ID: {id}")?;
                            }
                        }
                        "cleanup" => {
                            if let Some(count) = result.file_count {
                                writeln!(writer, "✓ Cleaned up {count} old checkpoint(s)")?;
                            }
                        }
                        _ => {
                            writeln!(
                                writer,
                                "✓ Checkpoint operation completed: {}",
                                result.operation
                            )?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Checkpoint operation failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_modes_result<W: Write>(
        &self,
        writer: &mut W,
        result: &ModesResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress(format!("Modes: {}", result.operation), 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    match result.operation.as_str() {
                        "list" => {
                            writeln!(writer, "Available Agent Modes:\n")?;
                            for mode in &result.modes {
                                writeln!(writer, "  {} - {}", mode.name, mode.description)?;
                            }
                            writeln!(writer)?;
                            writeln!(
                                writer,
                                "Use 'clawdius chat --mode <name>' to use a specific mode."
                            )?;
                            writeln!(writer, "Custom modes can be created in .clawdius/modes/")?;
                        }
                        "create" => {
                            writeln!(writer, "✓ Created mode configuration")?;
                            if let Some(path) = &result.created_path {
                                writeln!(writer, "  Path: {path}")?;
                            }
                            if let Some(name) = &result.mode_name {
                                writeln!(writer, "  Name: {name}")?;
                            }
                            writeln!(writer)?;
                            writeln!(writer, "Edit the file to customize the mode's behavior.")?;
                        }
                        "show" => {
                            if let Some(details) = &result.mode_details {
                                writeln!(writer, "Mode: {}", details.name)?;
                                writeln!(writer, "Description: {}", details.description)?;
                                writeln!(writer)?;
                                writeln!(writer, "System Prompt:")?;
                                writeln!(writer, "{}", details.system_prompt)?;
                                writeln!(writer)?;
                                writeln!(writer, "Temperature: {}", details.temperature)?;
                                writeln!(writer, "Tools: {:?}", details.tools)?;
                            }
                        }
                        _ => {
                            writeln!(writer, "✓ Modes operation completed: {}", result.operation)?;
                        }
                    }
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Modes operation failed: {error}")?;
                }
            }
        }

        Ok(())
    }

    pub fn format_telemetry_result<W: Write>(
        &self,
        writer: &mut W,
        result: &TelemetryResult,
    ) -> crate::Result<()> {
        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                writeln!(writer, "{json}")?;
            }
            OutputFormat::StreamJson => {
                let mut stream_writer =
                    StreamWriter::new(writer.by_ref(), OutputFormat::StreamJson);
                let event = StreamEvent::progress("Telemetry configuration updated", 1, 1);
                stream_writer.write_event(&event)?;
                stream_writer.flush()?;
            }
            OutputFormat::Text => {
                if result.success {
                    writeln!(writer, "Telemetry configuration updated:")?;
                    writeln!(
                        writer,
                        "  Metrics: {}",
                        if result.metrics_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  Crash Reporting: {}",
                        if result.crash_reporting_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )?;
                    writeln!(
                        writer,
                        "  Performance Monitoring: {}",
                        if result.performance_monitoring_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )?;
                } else if let Some(error) = &result.error {
                    writeln!(writer, "✗ Telemetry configuration failed: {error}")?;
                }
            }
        }

        Ok(())
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new(OutputOptions::default())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: Option<String>,
    pub message_count: usize,
    pub tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_chat_response_text() {
        let formatter = OutputFormatter::default();
        let mut output = Vec::new();

        formatter
            .format_chat_response(
                &mut output,
                "Hello, world!",
                "session-123",
                "anthropic",
                Some("claude-3-5-sonnet"),
                10,
                5,
                1000,
            )
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Hello, world!"));
        assert!(output_str.contains("session-123"));
    }

    #[test]
    fn test_format_chat_response_json() {
        let options = OutputOptions {
            format: OutputFormat::Json,
            ..Default::default()
        };
        let formatter = OutputFormatter::new(options);
        let mut output = Vec::new();

        formatter
            .format_chat_response(
                &mut output,
                "Hello, world!",
                "session-123",
                "anthropic",
                Some("claude-3-5-sonnet"),
                10,
                5,
                1000,
            )
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\"content\": \"Hello, world!\""));
        assert!(output_str.contains("\"session_id\": \"session-123\""));
    }

    #[test]
    fn test_format_error_json() {
        let options = OutputOptions {
            format: OutputFormat::Json,
            ..Default::default()
        };
        let formatter = OutputFormatter::new(options);
        let mut output = Vec::new();

        formatter
            .format_error(&mut output, "Something went wrong", Some("session-123"))
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\"success\": false"));
        assert!(output_str.contains("Something went wrong"));
    }

    #[test]
    fn test_format_session_list() {
        let formatter = OutputFormatter::default();
        let mut output = Vec::new();

        let sessions = vec![SessionInfo {
            id: "session-1".to_string(),
            title: Some("Test Session".to_string()),
            message_count: 5,
            tokens: 100,
        }];

        formatter
            .format_session_list(&mut output, &sessions)
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("session-1"));
        assert!(output_str.contains("Test Session"));
    }

    #[test]
    fn test_format_chat_response_stream_json() {
        let options = OutputOptions {
            format: OutputFormat::StreamJson,
            ..Default::default()
        };
        let formatter = OutputFormatter::new(options);
        let mut output = Vec::new();

        formatter
            .format_chat_response(
                &mut output,
                "Hello world",
                "session-123",
                "anthropic",
                Some("claude-3-5-sonnet"),
                10,
                5,
                1000,
            )
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains(r#""type":"start"#));
        assert!(output_str.contains(r#""type":"token"#));
        assert!(output_str.contains(r#""type":"complete"#));
    }

    #[test]
    fn test_format_error_stream_json() {
        let options = OutputOptions {
            format: OutputFormat::StreamJson,
            ..Default::default()
        };
        let formatter = OutputFormatter::new(options);
        let mut output = Vec::new();

        formatter
            .format_error(&mut output, "Test error", Some("session-123"))
            .unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains(r#""type":"error"#));
        assert!(output_str.contains("Test error"));
    }
}
