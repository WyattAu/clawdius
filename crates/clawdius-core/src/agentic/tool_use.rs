//! Tool-Use Protocol for Sprint Build Phase
//!
//! Parses structured tool calls from LLM output and executes them via
//! ShellToolExecutor. Implements the agent loop: LLM thinks → calls tools →
//! sees results → thinks again → calls more tools → signals done.

use crate::agentic::tool_executor::{ShellToolExecutor, ToolExecutor, ToolRequest, ToolResult};
use crate::llm::providers::{ChatWithToolsResult, LlmClient};
use crate::llm::{ChatMessage, ChatRole};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Maximum number of tool-use iterations before forcing completion.
const MAX_TOOL_ITERATIONS: usize = 20;

/// Maximum output size per tool call (128KB).
const MAX_TOOL_OUTPUT_BYTES: usize = 128 * 1024;

/// A parsed tool call from LLM output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    /// Tool name: "write_file", "shell", "edit_file"
    pub tool: String,
    /// Arguments as key-value pairs
    pub args: serde_json::Value,
    /// Raw text of the tool call block (for error messages)
    pub raw: String,
}

/// Result of executing a batch of tool calls.
#[derive(Debug, Clone)]
pub struct ToolUseRound {
    /// Tool calls that were executed
    pub calls: Vec<ToolCall>,
    /// Results of each tool call (same order)
    pub results: Vec<ToolExecutionResult>,
    /// Total tokens used in the LLM request that produced these calls
    pub tokens_used: usize,
}

/// Result of executing a single tool call.
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

/// Parse LLM output for tool calls.
///
/// Supports two formats:
/// 1. JSON code blocks: ```tool\n{"tool":"shell","args":{"command":"ls"}}\n```
/// 2. Markdown-style: [TOOL:shell] command="ls"
pub fn parse_tool_calls(llm_output: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();

    // Format 1: ```tool JSON blocks
    for line_group in extract_code_blocks(llm_output, "tool") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_group) {
            if let (Some(tool), Some(args)) = (
                json.get("tool").and_then(|t| t.as_str()),
                json.get("args").cloned(),
            ) {
                calls.push(ToolCall {
                    tool: tool.to_string(),
                    args,
                    raw: line_group.clone(),
                });
            }
        }
    }

    // Format 2: [TOOL:name] key="value" key2="value2"
    for line in llm_output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[TOOL:") {
            if let Some(tool_end) = rest.find(']') {
                let tool = &rest[..tool_end];
                let args_str = &rest[tool_end + 1..];

                // Parse key="value" pairs
                let mut args = serde_json::Map::new();
                for pair in parse_kv_pairs(args_str) {
                    args.insert(pair.0, serde_json::Value::String(pair.1));
                }

                if !tool.is_empty() && !args.is_empty() {
                    calls.push(ToolCall {
                        tool: tool.to_string(),
                        args: serde_json::Value::Object(args),
                        raw: line.to_string(),
                    });
                }
            }
        }
    }

    calls
}

/// Execute a parsed tool call via ShellToolExecutor.
pub async fn execute_tool_call(
    executor: &dyn ToolExecutor,
    call: &ToolCall,
    project_root: &std::path::Path,
) -> ToolExecutionResult {
    let start = std::time::Instant::now();

    match call.tool.as_str() {
        "write_file" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = call
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let full_path = resolve_path(project_root, path);

            match std::fs::create_dir_all(full_path.parent().unwrap_or_else(|| full_path.as_path()))
                .and_then(|_| std::fs::write(&full_path, content))
            {
                Ok(()) => ToolExecutionResult {
                    success: true,
                    output: format!("Wrote {} ({} bytes)", full_path.display(), content.len()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
                Err(e) => ToolExecutionResult {
                    success: false,
                    output: format!("Failed to write {}: {e}", full_path.display()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        },
        "edit_file" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let old_text = call
                .args
                .get("old_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_text = call
                .args
                .get("new_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let full_path = resolve_path(project_root, path);

            match std::fs::read_to_string(&full_path) {
                Ok(original) => {
                    // Use edit cascade for fuzzy matching fallback
                    let cascade_params = crate::tools::edit_cascade::EditParams {
                        content: &original,
                        old_text,
                        new_text,
                        replace_all: false,
                    };

                    match crate::tools::edit_cascade::apply_edit_cascade(&cascade_params) {
                        Ok(result) => {
                            match std::fs::write(&full_path, &result.new_content) {
                                Ok(()) => {
                                    let strategy_note = if result.strategy
                                        != crate::tools::edit_cascade::Strategy::Exact
                                    {
                                        format!(
                                            " (via {} strategy, confidence {:.0}%)",
                                            result.strategy,
                                            result.confidence * 100.0
                                        )
                                    } else {
                                        String::new()
                                    };
                                    ToolExecutionResult {
                                        success: true,
                                        output: format!(
                                            "Edited {} (replaced {} bytes with {} bytes){}",
                                            full_path.display(),
                                            old_text.len(),
                                            new_text.len(),
                                            strategy_note
                                        ),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                    }
                                }
                                Err(e) => ToolExecutionResult {
                                    success: false,
                                    output: format!(
                                        "Failed to write {}: {e}",
                                        full_path.display()
                                    ),
                                    duration_ms: start.elapsed().as_millis() as u64,
                                },
                            }
                        }
                        Err(e) => ToolExecutionResult {
                            success: false,
                            output: format!(
                                "old_text not found in {}. The file content may have changed.\n\
                                 Edit cascade diagnostics:\n{}",
                                full_path.display(),
                                e
                            ),
                            duration_ms: start.elapsed().as_millis() as u64,
                        },
                    }
                }
                Err(e) => ToolExecutionResult {
                    success: false,
                    output: format!("Failed to read {}: {e}", full_path.display()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        },
        "shell" => {
            let command = call
                .args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let request = ToolRequest::new("shell")
                .with_arg("command", serde_json::Value::String(command.to_string()));

            match executor.execute(request).await {
                Ok(result) => {
                    let output = truncate_output(&result.content, MAX_TOOL_OUTPUT_BYTES);
                    ToolExecutionResult {
                        success: result.success,
                        output: format!(
                            "[shell: {}]\n{}{}",
                            command,
                            output,
                            if result.success {
                                ""
                            } else {
                                "\n(exit code: non-zero)"
                            }
                        ),
                        duration_ms: start.elapsed().as_millis() as u64,
                    }
                },
                Err(e) => ToolExecutionResult {
                    success: false,
                    output: format!("Tool execution error: {e}"),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        },
        "read_file" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let full_path = resolve_path(project_root, path);

            match std::fs::read_to_string(&full_path) {
                Ok(content) => {
                    let truncated = truncate_output(&content, MAX_TOOL_OUTPUT_BYTES);
                    ToolExecutionResult {
                        success: true,
                        output: format!("[{}]\n{}", full_path.display(), truncated),
                        duration_ms: start.elapsed().as_millis() as u64,
                    }
                },
                Err(e) => ToolExecutionResult {
                    success: false,
                    output: format!("Failed to read {}: {e}", full_path.display()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        },
        "list_files" => {
            let dir = call
                .args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let full_dir = resolve_path(project_root, dir);

            match std::fs::read_dir(&full_dir) {
                Ok(entries) => {
                    let mut listing = String::new();
                    for entry in entries.flatten() {
                        let file_type = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            "DIR "
                        } else {
                            "    "
                        };
                        listing.push_str(&format!(
                            "{}{}\n",
                            file_type,
                            entry.file_name().to_string_lossy()
                        ));
                    }
                    ToolExecutionResult {
                        success: true,
                        output: if listing.is_empty() {
                            "(empty directory)".to_string()
                        } else {
                            listing
                        },
                        duration_ms: start.elapsed().as_millis() as u64,
                    }
                },
                Err(e) => ToolExecutionResult {
                    success: false,
                    output: format!("Failed to list {}: {e}", full_dir.display()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        },
        _ => ToolExecutionResult {
            success: false,
            output: format!("Unknown tool: {}", call.tool),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Check if the LLM output signals completion (no more tool calls needed).
pub fn is_completion(llm_output: &str) -> bool {
    let calls = parse_tool_calls(llm_output);
    let lower = llm_output.to_lowercase();
    // Explicit completion signals
    let explicit_done = lower.contains("[done]")
        || lower.contains("[complete]")
        || lower.contains("[finished]")
        || lower.contains("no more changes needed")
        || lower.contains("all changes have been made");

    explicit_done || calls.is_empty()
}

/// Format tool execution results for feeding back to the LLM.
pub fn format_tool_results(round: &ToolUseRound) -> String {
    let mut output = String::from("\n\n## Tool Results\n\n");
    for (call, result) in round.calls.iter().zip(round.results.iter()) {
        let status = if result.success { "OK" } else { "FAILED" };
        output.push_str(&format!(
            "### [{}] {} — {}\n",
            call.tool, status, result.output
        ));
        output.push_str("---\n\n");
    }
    output
}

/// System prompt addition for tool-use enabled phases.
pub fn tool_use_instructions() -> String {
    r#"
## Tool Use

You have access to the following tools. Use them to actually make changes, not just describe them.

### Available Tools

1. **write_file** — Create or overwrite a file
   ```tool
   {"tool": "write_file", "args": {"path": "src/main.rs", "content": "fn main() {}\n"}}
   ```

2. **edit_file** — Find and replace text in an existing file
   ```tool
   {"tool": "edit_file", "args": {"path": "src/main.rs", "old_text": "old code", "new_text": "new code"}}
   ```

3. **shell** — Run a shell command
   ```tool
   {"tool": "shell", "args": {"command": "cargo build 2>&1"}}
   ```

4. **read_file** — Read a file's contents
   ```tool
   {"tool": "read_file", "args": {"path": "Cargo.toml"}}
   ```

5. **list_files** — List files in a directory
   ```tool
   {"tool": "list_files", "args": {"path": "src"}}
   ```

### Rules
- Use `write_file` for new files. Use `edit_file` for existing files.
- Paths are relative to the project root.
- Run `shell` commands to build and test your changes.
- After making changes, run tests to verify correctness.
- When all changes are complete, write `[DONE]` on its own line.
- You can issue multiple tool calls in a single response.
"#.to_string()
}

/// Run the full tool-use loop for a phase.
///
/// 1. Send initial message to LLM (with tool instructions)
/// 2. Parse tool calls from response
/// 3. Execute them
/// 4. Feed results back to LLM
/// 5. Repeat until LLM signals [DONE] or max iterations reached
pub async fn run_tool_use_loop(
    llm: &Arc<dyn LlmClient>,
    executor: &Arc<dyn ToolExecutor>,
    system_prompt: &str,
    initial_user_message: &str,
    project_root: &std::path::Path,
    max_iterations: Option<usize>,
) -> Result<(String, usize, Vec<String>)> {
    let max_iters = max_iterations.unwrap_or(MAX_TOOL_ITERATIONS);
    let full_system = format!("{}\n{}", system_prompt, tool_use_instructions());

    let mut messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: full_system,
        },
        ChatMessage {
            role: ChatRole::User,
            content: initial_user_message.to_string(),
        },
    ];

    let mut total_tokens = 0usize;
    let mut all_files_modified = Vec::new();
    let mut final_output = String::new();

    for iteration in 0..max_iters {
        // Ask the LLM (with 60s timeout per call)
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            llm.chat(messages.clone()),
        )
        .await
        .map_err(|_| crate::Error::Llm("Tool-use loop LLM call timed out (60s)".to_string()))?
        .map_err(|e| crate::Error::Llm(format!("Tool-use loop LLM error: {e}")))?;

        let tokens = llm.count_tokens(&response);
        total_tokens += tokens;

        eprintln!(
            "  [tool loop iter {}/{}] {} tokens",
            iteration + 1,
            max_iters,
            tokens
        );

        // Check for completion
        if is_completion(&response) {
            final_output = response;
            break;
        }

        // Parse tool calls
        let calls = parse_tool_calls(&response);

        if calls.is_empty() {
            // No tool calls and no completion signal — treat as done
            final_output = response;
            break;
        }

        // Execute tool calls
        let mut results = Vec::new();
        for call in &calls {
            let result = execute_tool_call(executor.as_ref(), call, project_root).await;
            eprintln!(
                "    [{}] {} ({})",
                call.tool,
                if result.success { "ok" } else { "FAIL" },
                result.duration_ms
            );

            // Track modified files
            if result.success {
                if call.tool == "write_file" || call.tool == "edit_file" {
                    if let Some(path) = call.args.get("path").and_then(|v| v.as_str()) {
                        all_files_modified.push(path.to_string());
                    }
                }
            }

            results.push(result);
        }

        let round = ToolUseRound {
            calls,
            results,
            tokens_used: tokens,
        };

        // Build the feedback message
        let tool_results_text = format_tool_results(&round);
        final_output = format!(
            "{}\n\n{}",
            response
                .lines()
                .filter(|l| !l.starts_with("```tool"))
                .collect::<Vec<_>>()
                .join("\n"),
            tool_results_text
        );

        // Add LLM response and tool results to conversation
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: response,
        });
        messages.push(ChatMessage {
            role: ChatRole::User,
            content: tool_results_text,
        });
    }

    // Deduplicate files
    all_files_modified.sort();
    all_files_modified.dedup();

    Ok((final_output, total_tokens, all_files_modified))
}

// ── Native Tool-Use Loop (for Anthropic/OpenAI/OpenRouter) ──────────────────

/// Build the tool definitions for native function calling.
///
/// These `genai::chat::Tool` definitions are sent to Anthropic/OpenAI/OpenRouter
/// as part of the API request. The LLM sees structured tool schemas and returns
/// `ToolCall` responses instead of text-based tool invocations.
pub fn native_tool_definitions() -> Vec<genai::chat::Tool> {
    use serde_json::json;

    vec![
        genai::chat::Tool::new("write_file")
            .with_description("Create or overwrite a file with the given content. Use for new files or complete rewrites.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to the project root"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write"
                    }
                },
                "required": ["path", "content"]
            })),
        genai::chat::Tool::new("edit_file")
            .with_description("Find and replace text in an existing file. Use for targeted changes to existing code.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to the project root"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find in the file"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            })),
        genai::chat::Tool::new("shell")
            .with_description("Run a shell command in the project directory. Use for building, testing, and git operations.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["command"]
            })),
        genai::chat::Tool::new("read_file")
            .with_description("Read the contents of a file. Use to inspect existing code before making changes.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to the project root"
                    }
                },
                "required": ["path"]
            })),
        genai::chat::Tool::new("list_files")
            .with_description("List files and directories in a given path. Use to explore the project structure.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path relative to the project root (default: \".\")"
                    }
                }
            })),
    ]
}

/// Execute a native `ToolCall` from `genai::chat::ToolCall` via the same logic
/// as the parser-based tool executor.
async fn execute_native_tool_call(
    executor: &Arc<dyn ToolExecutor>,
    call: &genai::chat::ToolCall,
    project_root: &std::path::Path,
) -> ToolExecutionResult {
    // Convert genai ToolCall to our internal ToolCall format for reuse
    let internal_call = ToolCall {
        tool: call.fn_name.clone(),
        args: call.fn_arguments.clone(),
        raw: format!(
            "native_tool_call({}): {}",
            call.call_id,
            serde_json::to_string(&call.fn_arguments).unwrap_or_default()
        ),
    };
    execute_tool_call(executor.as_ref(), &internal_call, project_root).await
}

/// Run the native tool-use loop using provider's structured function calling.
///
/// Unlike the parser-based `run_tool_use_loop`, this uses `chat_with_tools()`
/// which sends tool definitions via the API and receives structured `ToolCall`
/// responses. This is more reliable for Anthropic Claude and OpenAI GPT-4o.
///
/// Returns `Err` if the provider doesn't support `chat_with_tools` (the caller
/// should fall back to the parser-based loop in that case).
pub async fn run_native_tool_use_loop(
    llm: &Arc<dyn LlmClient>,
    executor: &Arc<dyn ToolExecutor>,
    system_prompt: &str,
    initial_user_message: &str,
    project_root: &std::path::Path,
    max_iterations: Option<usize>,
) -> Result<(String, usize, Vec<String>)> {
    let max_iters = max_iterations.unwrap_or(MAX_TOOL_ITERATIONS);
    let tools = native_tool_definitions();

    let native_system = format!(
        "{}\n\nYou have access to tools for file operations and shell commands. \
         Use them to make real changes. When done, respond with text only (no tool calls).",
        system_prompt
    );

    let initial_messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: native_system,
        },
        ChatMessage {
            role: ChatRole::User,
            content: initial_user_message.to_string(),
        },
    ];

    let mut total_tokens = 0usize;
    let mut all_files_modified = Vec::new();
    let mut final_output = String::new();
    let mut messages: Vec<ChatMessage> = initial_messages;

    for iteration in 0..max_iters {
        // Try native tool calling (with 60s timeout)
        let result: ChatWithToolsResult = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            llm.chat_with_tools(messages.clone(), tools.clone()),
        )
        .await
        .map_err(|_| crate::Error::Llm("Native tool-use LLM call timed out (60s)".to_string()))?
        .map_err(|e| crate::Error::Llm(format!("Native tool-use not available: {e}")))?;

        let tokens = llm.count_tokens(&result.text);
        total_tokens += tokens;

        eprintln!(
            "  [native tool loop iter {}/{}] {} tokens, {} tool calls",
            iteration + 1,
            max_iters,
            tokens,
            result.tool_calls.len()
        );

        // If no tool calls, we're done
        if result.tool_calls.is_empty() {
            final_output = result.text;
            break;
        }

        // Execute each tool call
        let mut tool_results = Vec::new();
        for tc in &result.tool_calls {
            let exec_result = execute_native_tool_call(executor, tc, project_root).await;
            eprintln!(
                "    [{}] {} ({})",
                tc.fn_name,
                if exec_result.success { "ok" } else { "FAIL" },
                exec_result.duration_ms
            );

            // Track modified files
            if exec_result.success {
                if tc.fn_name == "write_file" || tc.fn_name == "edit_file" {
                    if let Some(path) = tc.fn_arguments.get("path").and_then(|v| v.as_str()) {
                        all_files_modified.push(path.to_string());
                    }
                }
            }

            // Build a ToolResponse to send back to the LLM
            tool_results.push(genai::chat::ToolResponse {
                call_id: tc.call_id.clone(),
                content: exec_result.output,
            });
        }

        // Append assistant response (with tool calls) to conversation
        // The assistant turn contains both text and tool calls
        let assistant_text = if result.text.is_empty() {
            String::new()
        } else {
            result.text.clone()
        };

        // Clone tool_calls before moving into MessageContent
        let tool_calls_for_output = result.tool_calls.clone();

        // Add the assistant message with tool calls
        let assistant_content = if assistant_text.is_empty() {
            genai::chat::MessageContent::from_tool_calls(result.tool_calls.clone())
        } else {
            // Mix text + tool calls: start with text, append tool calls
            let mut parts: Vec<genai::chat::ContentPart> =
                vec![genai::chat::ContentPart::Text(assistant_text.clone())];
            for tc in result.tool_calls {
                parts.push(genai::chat::ContentPart::ToolCall(tc));
            }
            genai::chat::MessageContent::from_parts(parts)
        };

        // We need to add assistant and tool-response messages to our ChatMessage list
        // Since ChatMessage only has role + content string, we reconstruct from the parts
        let assistant_str = assistant_content.texts().join("\n")
            + if !tool_calls_for_output.is_empty() {
                "\n[Called tools — see tool results below]"
            } else {
                ""
            };
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: assistant_str,
        });

        // Add tool results as a user message
        let mut results_text = String::from("## Tool Results\n\n");
        for (tc, tr) in tool_calls_for_output.iter().zip(tool_results.iter()) {
            results_text.push_str(&format!(
                "### [{}] {} — {}\n---\n\n",
                tc.fn_name,
                if tr.content.to_lowercase().contains("fail")
                    || tr.content.to_lowercase().contains("error")
                {
                    "FAILED"
                } else {
                    "OK"
                },
                tr.content
            ));
        }
        messages.push(ChatMessage {
            role: ChatRole::User,
            content: results_text.clone(),
        });

        final_output = format!("{}\n{}", assistant_text, results_text);
    }

    // Deduplicate files
    all_files_modified.sort();
    all_files_modified.dedup();

    Ok((final_output, total_tokens, all_files_modified))
}

// --- Internal helpers ---

fn extract_code_blocks(text: &str, language: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let marker = format!("```{}", language);
    let end_marker = "```";

    let mut in_block = false;
    let mut current = String::new();

    for line in text.lines() {
        if !in_block {
            if line.trim().starts_with(&marker) {
                in_block = true;
                // Check if there's content after the marker on the same line
                if let Some(rest) = line.trim().strip_prefix(&marker) {
                    let rest = rest.trim();
                    if !rest.is_empty() {
                        current.push_str(rest);
                        current.push('\n');
                    }
                }
            }
        } else {
            if line.trim() == end_marker || line.trim().starts_with(end_marker) {
                in_block = false;
                if !current.is_empty() {
                    blocks.push(current.trim().to_string());
                }
                current = String::new();
            } else {
                current.push_str(line);
                current.push('\n');
            }
        }
    }

    // Handle unclosed block
    if in_block && !current.is_empty() {
        blocks.push(current.trim().to_string());
    }

    blocks
}

fn parse_kv_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_key = String::new();
    let mut in_quotes = false;
    let mut current_value = String::new();
    let mut quote_char = ' ';

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if !in_quotes {
            match c {
                '=' => {
                    current_key = current_key.trim().to_string();
                    i += 1;
                    // Skip whitespace
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }
                    // Check for opening quote
                    if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                        quote_char = chars[i];
                        in_quotes = true;
                        current_value = String::new();
                        i += 1;
                    }
                    continue;
                },
                ' ' | '\t' => {
                    // End of key if we have content
                    if !current_key.is_empty() {
                        current_key = current_key.trim().to_string();
                    }
                },
                _ => current_key.push(c),
            }
        } else {
            if c == quote_char {
                in_quotes = false;
                pairs.push((current_key.clone(), current_value.clone()));
                current_key = String::new();
                current_value = String::new();
            } else {
                current_value.push(c);
            }
        }
        i += 1;
    }

    // Handle unclosed quote
    if in_quotes {
        pairs.push((current_key, current_value));
    }

    pairs
}

fn resolve_path(root: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(relative);
    if path.is_absolute() {
        // Safety: don't allow absolute paths to escape the project root
        let stripped = path
            .to_str()
            .map(|s| s.strip_prefix('/').unwrap_or(s))
            .unwrap_or(relative);
        root.join(stripped)
    } else {
        root.join(path)
    }
}

fn truncate_output(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let truncated = &s[..max_bytes];
        format!(
            "{}\n\n[... output truncated: {} of {} bytes ...]",
            truncated,
            max_bytes,
            s.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_calls_json_format() {
        let output = r#"
Here are the changes I'll make:

```tool
{"tool": "write_file", "args": {"path": "src/main.rs", "content": "fn main() {\n    println!(\"Hello\");\n}\n"}}
```

And another:

```tool
{"tool": "shell", "args": {"command": "cargo build"}}
```

[DONE]
"#;
        let calls = parse_tool_calls(output);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "write_file");
        assert_eq!(
            calls[0].args.get("path").unwrap().as_str().unwrap(),
            "src/main.rs"
        );
        assert_eq!(calls[1].tool, "shell");
        assert_eq!(
            calls[1].args.get("command").unwrap().as_str().unwrap(),
            "cargo build"
        );
    }

    #[test]
    fn test_parse_tool_calls_bracket_format() {
        let output = r#"
[TOOL:shell] command="ls -la"
[TOOL:write_file] path="README.md" content="Hello World"
"#;
        let calls = parse_tool_calls(output);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool, "shell");
        assert_eq!(calls[1].tool, "write_file");
    }

    #[test]
    fn test_parse_tool_calls_no_calls() {
        let output = "I've analyzed the code and here are my recommendations...";
        let calls = parse_tool_calls(output);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_is_completion() {
        assert!(is_completion("[DONE]"));
        assert!(is_completion("All changes complete.\n[finished]"));
        assert!(is_completion("No more changes needed."));
        // Tool-like text without valid args is treated as done (no parseable calls)
        assert!(is_completion(
            "```tool\n{\"tool\":\"shell\"}\n```\nMore text after."
        ));
        assert!(is_completion("Just a summary, no tool calls."));
        // A response with a valid tool call is NOT completion
        assert!(!is_completion(
            "```tool\n{\"tool\":\"shell\",\"args\":{\"command\":\"ls\"}}\n```"
        ));
    }

    #[test]
    fn test_format_tool_results() {
        let round = ToolUseRound {
            calls: vec![ToolCall {
                tool: "write_file".to_string(),
                args: serde_json::json!({"path": "test.rs", "content": "fn test() {}"}),
                raw: String::new(),
            }],
            results: vec![ToolExecutionResult {
                success: true,
                output: "Wrote test.rs (14 bytes)".to_string(),
                duration_ms: 5,
            }],
            tokens_used: 100,
        };
        let text = format_tool_results(&round);
        assert!(text.contains("### [write_file] OK"));
        assert!(text.contains("Wrote test.rs"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            resolve_path(root, "src/main.rs"),
            std::path::PathBuf::from("/project/src/main.rs")
        );
    }

    #[test]
    fn test_resolve_path_absolute_safety() {
        let root = std::path::Path::new("/project");
        assert_eq!(
            resolve_path(root, "/etc/passwd"),
            std::path::PathBuf::from("/project/etc/passwd")
        );
    }

    #[test]
    fn test_truncate_output() {
        let long = "x".repeat(200);
        let truncated = truncate_output(&long, 100);
        assert!(truncated.len() < 200);
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_extract_code_blocks() {
        let text = r#"
Before
```tool
{"tool": "shell", "args": {"command": "echo hello"}}
```
After
"#;
        let blocks = extract_code_blocks(text, "tool");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("echo hello"));
    }
}
