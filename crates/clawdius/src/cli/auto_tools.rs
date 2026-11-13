//! CLI tool host for `auto` mode: core tools behind one dispatch closure.
//!
//! Mirrors the TUI executor's tool surface for the headless path (web
//! search/fetch and the subagent host are TUI-side for now; the subagent
//! tool is excluded because `auto` already *is* the nested loop).

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use clawdius_core::llm::providers::Tool;
use clawdius_core::tools::file::{
    FileEditParams, FileListParams, FileReadParams, FileTool, FileWriteParams,
};
use clawdius_core::tools::git::{GitDiffParams, GitLogParams, GitTool};
use clawdius_core::tools::search::{resolve_root, SearchTool};
use clawdius_core::tools::shell::{ShellParams, ShellTool};
use clawdius_core::tools::todo;
use serde_json::Value;

/// Tool host for `auto` runs: the repair-essential core toolset.
pub struct AutoToolHost {
    file: FileTool,
    shell: ShellTool,
    git: GitTool,
    search: SearchTool,
    todos: Mutex<Vec<todo::TodoItem>>,
}

impl AutoToolHost {
    /// Build the host rooted at `workspace_root`.
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        let sandbox = clawdius_core::config::ShellSandboxConfig::default();
        Self {
            file: FileTool::with_workspace_root(&workspace_root),
            shell: ShellTool::new(sandbox.clone(), workspace_root.clone()),
            git: GitTool::new(sandbox, workspace_root),
            search: SearchTool::new(),
            todos: Mutex::new(Vec::new()),
        }
    }

    /// Tool definitions sent to the model.
    ///
    /// Every tool carries a real JSON Schema: models that only see prose
    /// argument hints routinely emit `{}` (live-caught with flash-lite
    /// calling `read_file {}` until the loop guard fired).
    #[must_use]
    pub fn tool_definitions() -> Vec<Tool> {
        let s = |t: &str| serde_json::json!({ "type": t });
        let schema = |props: serde_json::Value, required: &[&str]| {
            serde_json::json!({
                "type": "object",
                "properties": props,
                "required": required,
            })
        };

        vec![
            Tool::new("read_file")
                .with_description("Read a file's contents. Prefer offset/limit for large files.")
                .with_schema(schema(
                    serde_json::json!({
                        "path": s("string"),
                        "offset": s("integer"),
                        "limit": s("integer"),
                    }),
                    &["path"],
                )),
            Tool::new("write_file")
                .with_description("Write content to a file, creating parent directories.")
                .with_schema(schema(
                    serde_json::json!({
                        "path": s("string"),
                        "content": s("string"),
                    }),
                    &["path", "content"],
                )),
            Tool::new("edit_file")
                .with_description(
                    "Replace an exact old_string with new_string in a file. old_string must \
                     match exactly once.",
                )
                .with_schema(schema(
                    serde_json::json!({
                        "path": s("string"),
                        "old_string": s("string"),
                        "new_string": s("string"),
                    }),
                    &["path", "old_string", "new_string"],
                )),
            Tool::new("list_directory")
                .with_description("List files in a directory (defaults to the repo root).")
                .with_schema(schema(serde_json::json!({ "path": s("string") }), &[])),
            Tool::new("grep")
                .with_description("Regex search file contents, respecting .gitignore.")
                .with_schema(schema(
                    serde_json::json!({
                        "pattern": s("string"),
                        "path": s("string"),
                        "include": s("string"),
                        "max_results": s("integer"),
                    }),
                    &["pattern"],
                )),
            Tool::new("glob")
                .with_description("List files matching a glob pattern, e.g. **/*.rs.")
                .with_schema(schema(
                    serde_json::json!({
                        "pattern": s("string"),
                        "path": s("string"),
                    }),
                    &["pattern"],
                )),
            Tool::new("todo_write")
                .with_description("Replace the session todo list.")
                .with_schema(schema(
                    serde_json::json!({
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": s("string"),
                                    "status": {
                                        "type": "string",
                                        "enum": ["pending", "in_progress", "completed"],
                                    },
                                },
                                "required": ["content", "status"],
                            },
                        },
                    }),
                    &["todos"],
                )),
            Tool::new("todo_read")
                .with_description("Show the current session todo list.")
                .with_schema(schema(serde_json::json!({}), &[])),
            Tool::new("shell")
                .with_description("Run a shell command in the workspace and return its output.")
                .with_schema(schema(
                    serde_json::json!({
                        "command": s("string"),
                        "timeout": s("integer"),
                    }),
                    &["command"],
                )),
            Tool::new("git_status")
                .with_description("Show the git working tree status.")
                .with_schema(schema(serde_json::json!({}), &[])),
            Tool::new("git_diff")
                .with_description("Show the git diff (unstaged, or staged when requested).")
                .with_schema(schema(
                    serde_json::json!({
                        "staged": s("boolean"),
                        "file": s("string"),
                    }),
                    &[],
                )),
            Tool::new("git_log")
                .with_description("Show recent commits.")
                .with_schema(schema(
                    serde_json::json!({
                        "count": s("integer"),
                        "file": s("string"),
                    }),
                    &[],
                )),
        ]
    }

    /// Dispatch a tool call: `(output, is_error)`.
    #[must_use]
    pub fn execute(&self, name: &str, arguments: &str) -> (String, bool) {
        let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
        let s = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let (output, is_error) = match name {
            "read_file" => {
                let params = FileReadParams {
                    path: s("path"),
                    offset: args
                        .get("offset")
                        .and_then(Value::as_u64)
                        .map(|v| usize::try_from(v).unwrap_or(0)),
                    limit: args
                        .get("limit")
                        .and_then(Value::as_u64)
                        .map(|v| usize::try_from(v).unwrap_or(0)),
                };
                self.file
                    .read(params)
                    .map_or_else(|e| (format!("Error: {e}"), true), |body| (body, false))
            },
            "write_file" => {
                let params = FileWriteParams {
                    path: s("path"),
                    content: s("content"),
                };
                let written_path = params.path.clone();
                self.file.write(params).map_or_else(
                    |e| (format!("Error: {e}"), true),
                    |()| (format!("Written to {written_path}"), false),
                )
            },
            "edit_file" => {
                let params = FileEditParams {
                    path: s("path"),
                    old_string: s("old_string"),
                    new_string: s("new_string"),
                    replace_all: false,
                };
                let edited_path = params.path.clone();
                match self.file.edit(params) {
                    Ok(true) => (format!("Edited {edited_path}"), false),
                    Ok(false) => (("old_string not found".to_string()), true),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "list_directory" => {
                // `path` is optional in the schema; an omitted arg arrives
                // as "" and fails path validation. Default to the root.
                let raw = s("path");
                let path = if raw.is_empty() { ".".to_string() } else { raw };
                let params = FileListParams { path };
                self.file.list(params).map_or_else(
                    |e| (format!("Error: {e}"), true),
                    |entries| (entries.join("\n"), false),
                )
            },
            "grep" => {
                let pattern = s("pattern");
                let path = if s("path").is_empty() {
                    ".".to_string()
                } else {
                    s("path")
                };
                let include = args
                    .get("include")
                    .and_then(Value::as_str)
                    .map(String::from);
                let max = usize::try_from(
                    args.get("max_results")
                        .and_then(Value::as_u64)
                        .unwrap_or(100),
                )
                .unwrap_or(100);
                match self
                    .search
                    .grep(&pattern, &resolve_root(&path), include.as_deref(), max)
                {
                    Ok(hits) if hits.is_empty() => (format!("No matches for '{pattern}'"), false),
                    Ok(hits) => {
                        let mut body = String::new();
                        for m in &hits {
                            let _ = writeln!(body, "{}:{}: {}", m.path, m.line, m.text);
                        }
                        (format!("{} matches:\n{body}", hits.len()), false)
                    },
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "glob" => {
                let pattern = s("pattern");
                let path = if s("path").is_empty() {
                    ".".to_string()
                } else {
                    s("path")
                };
                match self.search.glob(&pattern, &resolve_root(&path)) {
                    Ok(paths) if paths.is_empty() => (format!("No files match '{pattern}'"), false),
                    Ok(paths) => (
                        format!("{} files:\n{}\n", paths.len(), paths.join("\n")),
                        false,
                    ),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "todo_write" => {
                let parsed = todo::parse_todos(&args.get("todos").cloned().unwrap_or(Value::Null));
                if parsed.is_empty() {
                    return (("todo_write: no valid entries".to_string()), true);
                }
                let rendered = todo::render_todos(&parsed);
                if let Ok(mut t) = self.todos.lock() {
                    *t = parsed;
                }
                (format!("Todo list updated.\n{rendered}"), false)
            },
            "todo_read" => self.todos.lock().ok().map_or_else(
                || (("todo list locked".to_string()), true),
                |todos| (todo::render_todos(&todos), false),
            ),
            "shell" => {
                let params = ShellParams {
                    command: s("command"),
                    timeout: args
                        .get("timeout")
                        .and_then(Value::as_u64)
                        .unwrap_or(120_000),
                    cwd: None,
                };
                if let Ok(result) = self.shell.execute(params) {
                    let output = if result.stdout.is_empty() {
                        result.stderr
                    } else if result.stderr.is_empty() {
                        result.stdout
                    } else {
                        format!("{}\n{}", result.stdout, result.stderr)
                    };
                    (output, false)
                } else {
                    ("shell failed".to_string(), true)
                }
            },
            "git_status" => self
                .git
                .status(None)
                .map_or_else(|e| (format!("Error: {e}"), true), |out| (out, false)),
            "git_diff" => self
                .git
                .diff(
                    GitDiffParams {
                        staged: args.get("staged").and_then(Value::as_bool).unwrap_or(false),
                        path: args.get("file").and_then(Value::as_str).map(String::from),
                    },
                    None,
                )
                .map_or_else(|e| (format!("Error: {e}"), true), |out| (out, false)),
            "git_log" => self
                .git
                .log(
                    GitLogParams {
                        count: usize::try_from(
                            args.get("count").and_then(Value::as_u64).unwrap_or(10),
                        )
                        .unwrap_or(10),
                        path: args.get("file").and_then(Value::as_str).map(String::from),
                    },
                    None,
                )
                .map_or_else(|e| (format!("Error: {e}"), true), |out| (out, false)),
            other => (format!("Unknown tool: {other}"), true),
        };

        // Blueprint 14/32/48: scrub secrets, collapse repetition, cap size
        // before the output enters model context.
        let (scrubbed, redactions) = clawdius_core::tools::output_filter::scrub_secrets(&output);
        let compacted = clawdius_core::tools::output_filter::compact_output(&scrubbed, 20_000);
        let output = if redactions > 0 {
            format!("{compacted}\n[{redactions} credential-shaped string(s) redacted]")
        } else {
            compacted
        };
        (output, is_error)
    }
}
