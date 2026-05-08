use super::engine::SprintEngine;
use super::{PhaseResult, PhaseStatus, SprintPhase, SprintState};
use crate::llm::{ChatMessage, ChatRole};
use crate::Result;

pub fn phase_prompt(phase: &SprintPhase) -> String {
    match phase {
        SprintPhase::Think => {
            "You are a product-thinking AI. Analyze the task and produce:\n\
             1. Problem statement (one sentence)\n\
             2. Key questions that need answers before building\n\
             3. Assumptions being made\n\
             4. Success criteria (measurable)\n\
             5. Risks and mitigations\n\
             6. Recommended approach (high-level)"
                .to_string()
        },
        SprintPhase::Plan => {
            "You are a technical planner. Based on the thinking output, create a detailed execution plan:\n\
             1. List of files to create/modify (with paths)\n\
             2. For each file: what changes are needed\n\
             3. Order of operations (dependencies)\n\
             4. Test strategy\n\
             5. Risk assessment for each step\n\
             Format as a numbered checklist with file paths."
                .to_string()
        },
        SprintPhase::Build => {
            "You are a senior engineer. Execute the plan by writing/modifying code.\n\
             Follow the plan step by step. For each step:\n\
             - State what you're doing\n\
             - Write the code (full file contents)\n\
             - Note any deviations from the plan\n\
             IMPORTANT: Actually write the code changes, don't just describe them."
                .to_string()
        },
        SprintPhase::Review => {
            "You are a staff engineer doing code review. Review all changes made:\n\
             - Correctness: Does the code do what the plan says?\n\
             - Edge cases: Are error paths handled?\n\
             - Security: Any vulnerabilities?\n\
             - Performance: Any anti-patterns?\n\
             - Style: Consistent with project conventions?\n\
             Rate each area 1-5 and provide specific feedback."
                .to_string()
        },
        SprintPhase::Test => {
            "You are a QA engineer. Based on the code changes:\n\
             1. List test cases that should exist\n\
             2. Run the project's test suite\n\
             3. Report pass/fail results\n\
             4. If failures: diagnose root cause and suggest fixes"
                .to_string()
        },
        SprintPhase::Ship => {
            "You are a release engineer. Based on all previous phases:\n\
             1. Summarize what was built\n\
             2. Verify all tests pass\n\
             3. Generate a commit message (conventional commits format)\n\
             4. Check if the branch is safe to push\n\
             5. Report what needs to happen to ship"
                .to_string()
        },
        SprintPhase::Reflect => {
            "You are a retrospective facilitator. Based on the entire sprint:\n\
             1. What went well?\n\
             2. What could be improved?\n\
             3. What did we learn?\n\
             4. Action items for next sprint\n\
             5. Metrics summary"
                .to_string()
        },
    }
}

/// Build the base user message with task description, context, and optional project structure.
fn build_base_user_message(state: &SprintState) -> String {
    let mut user_message = format!(
        "Task: {}\n\nPrevious context:\n{}",
        state.config.task_description, state.context_accumulator
    );
    if let Some(ref ctx) = state.config.extra_context {
        if !ctx.is_empty() {
            user_message = format!("{}\n\n## Project Structure\n{}", ctx, user_message);
        }
    }
    user_message
}

/// Attempt to attach browser QA snapshot context for the Test phase.
async fn attach_browser_qa_context(
    engine: &SprintEngine,
    state: &SprintState,
    user_message: &mut String,
) {
    let Some(ref url) = state.config.browser_qa_url else {
        return;
    };

    if let Some(ref daemon) = engine.browser_daemon {
        let session_id = "sprint-qa";
        let _ = daemon.register_session(session_id).await;
        let _ = daemon.initialize().await;

        if daemon.navigate(url, Some(session_id)).await.is_ok() {
            if let Ok(snapshot) = daemon.build_snapshot(session_id).await {
                let tree_lines: Vec<String> = snapshot
                    .elements
                    .iter()
                    .map(|e| format!("  {} {} \"{}\"", e.ref_id, e.role, e.name))
                    .collect();
                let tree_text = tree_lines.join("\n");
                user_message.push_str(&format!(
                    "\n\n## Browser QA — Live Snapshot (URL: {})\n\
                     ### Accessibility Tree\n{}\n\
                     ### Element References\n{}\n\
                     Use the references above (e.g. @e1, @e2) to identify specific elements.\n\
                     Report any visual or functional issues found.",
                    snapshot.url,
                    tree_text,
                    snapshot.to_ref_list(),
                ));
            } else {
                user_message.push_str(&format!(
                    "\n\n## Browser QA\n\
                     A browser-based QA check is available at: {url}\n\
                     (Browser daemon connected but snapshot failed.)\n\
                     Report any issues you can identify."
                ));
            }
        } else {
            user_message.push_str(&format!(
                "\n\n## Browser QA\n\
                 A browser-based QA check is available at: {url}\n\
                 (Browser daemon connected but navigation failed.)\n\
                 Report any issues you can identify."
            ));
        }
        let _ = daemon.unregister_session(session_id).await;
    } else {
        user_message.push_str(&format!(
            "\n\n## Browser QA\n\
             A browser-based QA check is available at: {url}\n\
             If the task involves a web application or UI, consider:\n\
             1. Navigate to the URL and verify the UI renders correctly\n\
             2. Check for console errors\n\
             3. Test interactive elements (buttons, forms, navigation)\n\
             4. Verify responsive behavior\n\
             5. Check accessibility basics (focus states, ARIA labels)\n\
             Report any visual or functional issues found."
        ));
    }
}

/// Attach LSP document symbols for the Plan phase.
async fn attach_lsp_symbols(engine: &SprintEngine, state: &SprintState, user_message: &mut String) {
    let Some(ref lsp) = engine.lsp_client else {
        return;
    };

    let all_mod: Vec<String> = state
        .phase_results
        .iter()
        .flat_map(|r| r.files_modified.clone())
        .collect();

    if all_mod.is_empty() {
        return;
    }

    let syms = {
        let mut lsp = lsp.lock().await;
        let mut text = String::new();
        for fp in &all_mod {
            let uri = format!("file://{}", fp);
            if let Ok(syms) = lsp.document_symbols(&uri).await {
                if !syms.is_empty() {
                    text.push_str(&format!("\n### {}\n", fp));
                    for s in &syms {
                        text.push_str(&format!("  {} ({:?})\n", s.name, s.kind));
                    }
                }
            }
        }
        text
    };

    if !syms.is_empty() {
        user_message.push_str("\n\n## Current Code Structure\n");
        user_message.push_str(&syms);
    }
}

/// Attach LSP diagnostics and code actions for Build/Test/Review phases.
async fn attach_lsp_diagnostics(
    engine: &SprintEngine,
    state: &SprintState,
    user_message: &mut String,
) {
    let Some(ref lsp) = engine.lsp_client else {
        return;
    };

    let all_modified: Vec<String> = state
        .phase_results
        .iter()
        .flat_map(|r| r.files_modified.clone())
        .collect();

    sync_lsp_documents(engine, &all_modified).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let (all_diags, code_actions) = {
        let mut lsp = lsp.lock().await;
        let diags = lsp.get_all_diagnostics().await;
        let mut actions = Vec::new();
        for (uri, file_diags) in &diags {
            let err_diags: Vec<crate::lsp::protocol::Diagnostic> = file_diags
                .iter()
                .filter(|d| d.severity == Some(crate::lsp::protocol::DiagnosticSeverity::Error))
                .cloned()
                .collect();
            if !err_diags.is_empty() {
                if let Ok(ca) = lsp
                    .code_actions(uri, err_diags[0].range.clone(), err_diags)
                    .await
                {
                    actions.extend(ca);
                }
            }
        }
        (diags, actions)
    };

    if !all_diags.is_empty() {
        let mut diag_text = String::from("\n\n## LSP Diagnostics\nThe language server reported:\n");
        let (mut error_count, mut warning_count) = (0usize, 0usize);
        for (uri, diags) in &all_diags {
            for d in diags {
                use crate::lsp::protocol::DiagnosticSeverity;
                let sev = match d.severity {
                    Some(DiagnosticSeverity::Error) => {
                        error_count += 1;
                        "ERROR"
                    },
                    Some(DiagnosticSeverity::Warning) => {
                        warning_count += 1;
                        "WARNING"
                    },
                    Some(DiagnosticSeverity::Information) => "INFO",
                    Some(DiagnosticSeverity::Hint) => "HINT",
                    _ => "UNKNOWN",
                };
                diag_text.push_str(&format!(
                    "  [{}] {} L{}:C{}: {}\n",
                    sev,
                    uri,
                    d.range.start.line + 1,
                    d.range.start.character + 1,
                    d.message
                ));
            }
        }
        diag_text.push_str(&format!(
            "\nTotal: {} errors, {} warnings\n",
            error_count, warning_count
        ));
        user_message.push_str(&diag_text);
    }

    if !code_actions.is_empty() {
        let mut action_text = String::from("\n\n## LSP Suggested Fixes\n");
        for action in &code_actions {
            if action.is_preferred {
                action_text.push_str(&format!("[PREFERRED] {}\n", action.title));
            } else {
                action_text.push_str(&format!("- {}\n", action.title));
            }
        }
        user_message.push_str(&action_text);
    }
}

/// Detect whether an LLM response is a provider error rather than real content.
fn is_provider_error(output: &str, tokens: usize) -> bool {
    let trimmed = output.trim();
    trimmed.is_empty()
        || trimmed.contains("no healthy upstream")
        || trimmed.contains("503 Service Unavailable")
        || trimmed.starts_with("[Error:")
        || trimmed.contains("overloaded")
        || (trimmed.contains("error") && tokens < 5)
}

/// Build a fallback combined message for retry when system-prompt chat fails.
fn build_fallback_message(phase: &SprintPhase, state: &SprintState) -> String {
    let mut combined = format!(
        "[Instructions]\n{}\n\n[Task & Context]\nTask: {}\n\nPrevious context:\n{}",
        phase_prompt(phase),
        &state.config.task_description,
        &state.context_accumulator
    );
    if let Some(ref ctx) = state.config.extra_context {
        if !ctx.is_empty() {
            combined = format!("[Project Structure]\n{}\n\n{}", ctx, combined);
        }
    }
    combined
}

/// Execute the primary LLM call and handle fallback on failure.
async fn call_llm_with_fallback(
    engine: &SprintEngine,
    phase: &SprintPhase,
    state: &SprintState,
    start: std::time::Instant,
) -> PhaseResult {
    let system_prompt = phase_prompt(phase);
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: system_prompt,
        },
        ChatMessage {
            role: ChatRole::User,
            content: build_base_user_message(state),
        },
    ];

    match engine.chat_collecting_stream(messages).await {
        Ok(output) => {
            let tokens = engine.llm.count_tokens(&output);
            if is_provider_error(&output, tokens) {
                PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Failed,
                    output: format!(
                        "LLM returned error response: {}",
                        &output[..output.len().min(200)]
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    files_modified: Vec::new(),
                    errors: vec!["LLM error response".to_string()],
                    tokens_used: tokens,
                }
            } else {
                PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Success,
                    output,
                    duration_ms: start.elapsed().as_millis() as u64,
                    files_modified: Vec::new(),
                    errors: Vec::new(),
                    tokens_used: tokens,
                }
            }
        },
        Err(_) => {
            // Retry without system prompt (some providers reject it)
            let fallback = vec![ChatMessage {
                role: ChatRole::User,
                content: build_fallback_message(phase, state),
            }];
            match engine.chat_collecting_stream(fallback).await {
                Ok(output) => {
                    let tokens = engine.llm.count_tokens(&output);
                    PhaseResult {
                        phase: phase.clone(),
                        status: PhaseStatus::Success,
                        output,
                        duration_ms: start.elapsed().as_millis() as u64,
                        files_modified: Vec::new(),
                        errors: Vec::new(),
                        tokens_used: tokens,
                    }
                },
                Err(e) => PhaseResult {
                    phase: phase.clone(),
                    status: PhaseStatus::Failed,
                    output: format!("LLM error: {e}"),
                    duration_ms: start.elapsed().as_millis() as u64,
                    files_modified: Vec::new(),
                    errors: vec![e.to_string()],
                    tokens_used: 0,
                },
            }
        },
    }
}

pub async fn run_phase(
    engine: &SprintEngine,
    state: &mut SprintState,
    phase: &SprintPhase,
) -> Result<PhaseResult> {
    if state.config.skip_phases.contains(phase) {
        let result = PhaseResult {
            phase: phase.clone(),
            status: PhaseStatus::Skipped,
            output: String::new(),
            duration_ms: 0,
            files_modified: Vec::new(),
            errors: Vec::new(),
            tokens_used: 0,
        };
        state.phase_results.push(result.clone());
        return Ok(result);
    }

    state.current_phase = Some(phase.clone());
    let start = std::time::Instant::now();

    // Build user message with phase-specific context
    let mut user_message = build_base_user_message(state);

    if *phase == SprintPhase::Test {
        attach_browser_qa_context(engine, state, &mut user_message).await;
    }
    if *phase == SprintPhase::Plan {
        attach_lsp_symbols(engine, state, &mut user_message).await;
    }
    if matches!(
        phase,
        SprintPhase::Build | SprintPhase::Test | SprintPhase::Review
    ) {
        attach_lsp_diagnostics(engine, state, &mut user_message).await;
    }

    // Run the LLM call
    let result = call_llm_with_fallback(engine, phase, state, start).await;

    state.context_accumulator.push_str(&format!(
        "\n\n=== {} Phase ===\n{}\n",
        phase.display_name(),
        result.output
    ));

    if let Err(e) = maybe_compact_context(engine, state).await {
        tracing::warn!("context compaction failed: {e}");
    }

    state.phase_results.push(result.clone());
    state.current_phase = None;

    Ok(result)
}

pub async fn sync_lsp_documents(engine: &SprintEngine, files: &[String]) {
    let Some(lsp) = engine.lsp_client.as_ref() else {
        return;
    };
    if files.is_empty() {
        return;
    }
    let mut lsp = lsp.lock().await;
    for file_path in files {
        if file_path.contains("://") {
            continue;
        }
        let path = std::path::Path::new(file_path);
        if !path.exists() {
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let uri = format!("file://{}", file_path);
                let lang_id = match path.extension().and_then(|e| e.to_str()) {
                    Some("rs") => "rust",
                    Some("ts") => "typescript",
                    Some("tsx") => "typescriptreact",
                    Some("js") => "javascript",
                    Some("jsx") => "javascriptreact",
                    Some("py") => "python",
                    Some("go") => "go",
                    Some("java") => "java",
                    Some("c") | Some("h") => "c",
                    Some("cpp") | Some("hpp") | Some("cc") => "cpp",
                    Some(other) => other,
                    None => "",
                };
                if let Err(e) = lsp.open_document(&uri, lang_id, &text).await {
                    tracing::debug!("LSP sync failed for {}: {}", file_path, e);
                }
            },
            Err(_) => {},
        }
    }
}

async fn maybe_compact_context(
    engine: &SprintEngine,
    state: &mut SprintState,
) -> crate::Result<()> {
    const COMPACT_THRESHOLD_CHARS: usize = 320_000;
    const KEEP_RECENT_CHARS: usize = 20_000;
    if state.context_accumulator.len() <= COMPACT_THRESHOLD_CHARS {
        return Ok(());
    }
    let total_len = state.context_accumulator.len();
    if total_len <= KEEP_RECENT_CHARS {
        return Ok(());
    }
    let split_point = total_len - KEEP_RECENT_CHARS;
    let split_point = state.context_accumulator[..split_point]
        .rfind('\n')
        .map(|p| p + 1)
        .unwrap_or(split_point);
    let old_context = &state.context_accumulator[..split_point];
    let recent_context = &state.context_accumulator[split_point..];
    tracing::info!(
        total_chars = total_len,
        old_chars = old_context.len(),
        "context compaction triggered"
    );
    let old_for_llm = if old_context.len() > 100_000 {
        &old_context[old_context.len() - 100_000..]
    } else {
        old_context
    };
    let system_prompt = ChatMessage {
        role: ChatRole::System,
        content: "You are a context compaction assistant. Summarize this sprint context preserving: task/objective, files modified (paths), key decisions, progress state, errors and fixes, code patterns. Discard verbose output. Under 1500 words.".to_string(),
    };
    let user_prompt = ChatMessage {
        role: ChatRole::User,
        content: format!("Summarize:\n\n{old_for_llm}"),
    };
    match engine.llm.chat(vec![system_prompt, user_prompt]).await {
        Ok(summary) => {
            let summary = if summary.len() > 8000 {
                format!("{}... [truncated]", &summary[..8000])
            } else {
                summary
            };
            let old_len = old_context.len();
            let new_acc = format!(
                "[Previous sprint context compacted]\n\n{summary}\n\n[End of compacted context]\n\n--- Recent context ---\n{recent_context}"
            );
            tracing::info!(
                old_chars = old_len,
                new_chars = new_acc.len(),
                "context compaction completed"
            );
            state.context_accumulator = new_acc;
        },
        Err(e) => {
            tracing::warn!("LLM compaction failed, using truncation fallback: {e}");
            let truncated = if old_context.len() > 8000 {
                format!(
                    "[Previous context truncated]\n...{}",
                    &old_context[old_context.len() - 8000..]
                )
            } else {
                old_context.to_string()
            };
            state.context_accumulator =
                format!("{truncated}\n\n--- Recent context ---\n{recent_context}");
        },
    }
    Ok(())
}
