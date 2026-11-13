//! Pipeline execution: `clawdius pipeline` — runs the formalized dev
//! loop (research → implement → gates → repeat) with real stages.
//!
//! LLM stages delegate to the `auto` engine (15 tools, loop guard,
//! journal); gate stages are deterministic shell ground truth. Every
//! run journals to `.clawdius/runs/pipeline-<name>.jsonl` so the
//! consolidation loop sees it.

use anyhow::{anyhow, Result};
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, ResolvedLlmConfig};
use clawdius_core::orchestrator::fallback::{EscalationRequest, OpencodeEscalator};
use clawdius_core::orchestrator::pipeline::{
    run_gate, GateResult, Pipeline, PipelineError, StageKind,
};
use std::path::PathBuf;

/// How many prior failure notes a repeated stage carries.
const FAILURE_CONTEXT_CAP: usize = 6;

pub(super) async fn handle_pipeline(
    name: String,
    goal: String,
    dry_run: bool,
    max_loops: Option<usize>,
    provider: String,
    model: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let mut pipeline = match name.as_str() {
        "product-loop" => Pipeline::product_loop(&goal),
        "dev-loop" => Pipeline::dev_loop(&goal),
        other => {
            return Err(anyhow!(
                "unknown pipeline '{other}' (available: product-loop, dev-loop)"
            ))
        },
    };
    if let Some(loops) = max_loops {
        pipeline.max_loops = loops;
    }
    pipeline.validate().map_err(|e| pipeline_err(&e))?;

    if dry_run {
        println!("{}", pipeline.plan());
        return Ok(());
    }

    run_pipeline(&pipeline, provider, model, config_path).await
}

fn pipeline_err(e: &PipelineError) -> anyhow::Error {
    anyhow!("invalid pipeline: {e}")
}

/// Execute the pipeline with loop control.
async fn run_pipeline(
    pipeline: &Pipeline,
    provider: String,
    model: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let workdir = std::env::current_dir()?;
    let journal_path = workdir.join(format!(".clawdius/runs/pipeline-{}.jsonl", pipeline.name));
    let mut journal = clawdius_core::session::journal::RunJournal::open(&journal_path)?;
    journal.record(&clawdius_core::session::journal::JournalEvent::Started {
        task: pipeline.goal.chars().take(200).collect(),
        provider: provider.clone(),
    })?;

    println!("{}", pipeline.plan());

    let mut gate_failures: Vec<String> = Vec::new();
    let mut loop_no = 1usize;
    let mut stage_index = 0usize;
    let mut prev_stage_output: Option<String> = None;

    loop {
        let stage = &pipeline.stages[stage_index];
        let slug = stage.kind.slug();

        if stage.kind.is_llm_stage() {
            let mut task = format!(
                "[pipeline {} loop {}/{}] [stage {}] {}\n\nOverall goal: {}",
                pipeline.name, loop_no, pipeline.max_loops, slug, stage.instruction, pipeline.goal
            );
            if let Some(prev) = &prev_stage_output {
                task.push_str("\n\nPrevious stage output (context):\n");
                task.push_str(&prev.chars().take(4_000).collect::<String>());
            }
            if !gate_failures.is_empty() {
                task.push_str("\n\nPrevious gate failures to fix:\n");
                for note in gate_failures.iter().rev().take(FAILURE_CONTEXT_CAP) {
                    task.push_str("- ");
                    task.push_str(note);
                    task.push('\n');
                }
            }

            println!("--- [loop {loop_no}] LLM stage: {slug} ---");
            let _ = journal.record(&clawdius_core::session::journal::JournalEvent::ToolCall {
                n: loop_no,
                name: format!("pipeline:{slug}"),
                args: String::new(),
            });
            // Implementation keeps the full auto tool loop; every other
            // LLM stage is a text deliverable — one chat call, no tools
            // (the 15-tool loop invites thrash on pure writing work).
            let outcome = if stage.kind == StageKind::Implementation {
                super::auto::handle_auto(
                    task,
                    model.clone(),
                    provider.clone(),
                    None,  // default iteration budget
                    false, // gates run here, not in auto
                    false, // no auto-commit mid-pipeline
                    false, // pipeline gates decide pass/fail
                    None,
                    config_path.clone(),
                    super::OutputFormat::Text,
                )
                .await
                .map(|()| String::from("implementation stage complete"))
            } else {
                run_chat_stage(&task, &provider, model.as_deref(), config_path.as_deref()).await
            };
            let is_error = outcome.is_err();
            let _ = journal.record(&clawdius_core::session::journal::JournalEvent::ToolResult {
                n: loop_no,
                name: format!("pipeline:{slug}"),
                is_error,
            });
            let stage_output = outcome?;
            // Persist the deliverable and feed it to the next stage.
            let out_dir = workdir.join(".clawdius/pipeline").join(&pipeline.name);
            std::fs::create_dir_all(&out_dir)?;
            let out_path = out_dir.join(format!("{loop_no}-{slug}.md"));
            std::fs::write(&out_path, &stage_output)?;
            prev_stage_output = Some(stage_output);
            gate_failures.clear();
        } else {
            println!("--- [loop {loop_no}] GATE stage: {slug} ---");
            let result: GateResult = run_gate(&stage.kind, &workdir);
            println!(
                "    {} ({} ms){}",
                if result.ok { "PASS" } else { "FAIL" },
                result.duration_ms,
                if result.ok {
                    String::new()
                } else {
                    format!("\n    tail: {}", first_lines(&result.output_tail, 5))
                },
            );
            let _ = journal.record(&clawdius_core::session::journal::JournalEvent::ToolResult {
                n: loop_no,
                name: format!("gate:{slug}"),
                is_error: !result.ok,
            });
            if !result.ok {
                gate_failures.push(format!(
                    "{} failed: {}",
                    slug,
                    first_lines(&result.output_tail, 3)
                ));
            }
        }

        // Advance / loop / finish.
        stage_index += 1;
        if stage_index < pipeline.stages.len() {
            continue;
        }
        // Tail reached: decide.
        if gate_failures.is_empty() {
            println!(
                "\nPipeline `{}` GREEN after {loop_no} loop(s).",
                pipeline.name
            );
            journal.record(&clawdius_core::session::journal::JournalEvent::Finished {
                ok: true,
                note: format!("green after {loop_no} loop(s)"),
            })?;
            return Ok(());
        }
        if loop_no >= pipeline.max_loops {
            // Last resort: hand the whole thing to opencode with the
            // failure context, then re-run the gates once on its work.
            println!("\n=== loops exhausted — escalating to opencode (last resort) ===");
            let escalator = OpencodeEscalator::new(workdir.clone());
            let request = EscalationRequest {
                task: pipeline.goal.clone(),
                failed_provider: provider.clone(),
                failure_notes: gate_failures.clone(),
            };
            let report = escalator.escalate(&request);
            let (esc_ok, esc_note) = match report {
                Ok(report) => {
                    println!(
                        "    opencode: {} ({} ms){}",
                        if report.ok { "ok" } else { "failed" },
                        report.duration_ms,
                        if report.ok {
                            format!("\n    {}", first_lines(&report.output, 5))
                        } else {
                            String::new()
                        },
                    );
                    (report.ok, format!("opencode exit-ok={}", report.ok))
                },
                Err(e) => {
                    println!("    opencode spawn failed: {e}");
                    (false, format!("opencode spawn failed: {e}"))
                },
            };
            if esc_ok {
                let recheck: Vec<GateResult> =
                    [StageKind::Verify, StageKind::Test, StageKind::Lint]
                        .iter()
                        .map(|k| run_gate(k, &workdir))
                        .collect();
                let all_green = recheck.iter().all(|g| g.ok);
                for g in &recheck {
                    println!(
                        "    recheck {}: {}",
                        g.slug,
                        if g.ok { "PASS" } else { "FAIL" }
                    );
                }
                if all_green {
                    journal.record(&clawdius_core::session::journal::JournalEvent::Finished {
                        ok: true,
                        note: format!("green after opencode escalation ({esc_note})"),
                    })?;
                    println!(
                        "\nPipeline `{}` GREEN via opencode escalation.",
                        pipeline.name
                    );
                    return Ok(());
                }
            }
            journal.record(&clawdius_core::session::journal::JournalEvent::Finished {
                ok: false,
                note: format!(
                    "gates still red after {loop_no} loop(s) + escalation: {} ({esc_note})",
                    gate_failures.join("; ")
                ),
            })?;
            return Err(anyhow!(
                "pipeline `{}` still failing after {loop_no} loop(s): {} (escalation: {esc_note})",
                pipeline.name,
                gate_failures.join("; ")
            ));
        }
        loop_no += 1;
        stage_index = pipeline.repeat_from;
        println!(
            "\n=== gates red — repeating tail from `{}` (loop {loop_no}/{}) ===\n",
            pipeline.stages[stage_index].kind.slug(),
            pipeline.max_loops,
        );
    }
}

/// One chat round for a text-deliverable stage (no tool loop).
async fn run_chat_stage(
    prompt: &str,
    provider: &str,
    model: Option<&str>,
    config_path: Option<&std::path::Path>,
) -> Result<String> {
    let config = super::load_config(config_path)?;
    let mut llm_config = ResolvedLlmConfig::from_config(&config.llm, provider)?;
    if let Some(m) = model {
        llm_config.model = m.to_string();
    }
    let client = create_provider(&llm_config)?;
    client
        .chat(vec![
            ChatMessage {
                role: ChatRole::System,
                content: "You are a precise software planning assistant. Respond with the \
                          deliverable only — no tool use, no questions, no preamble."
                    .to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: prompt.to_string(),
            },
        ])
        .await
        .map_err(|e| anyhow!("chat stage failed: {e}"))
}

fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_lines_caps_and_joins() {
        assert_eq!(first_lines("a\nb\nc", 2), "a | b");
        assert_eq!(first_lines("", 3), "");
    }
}
