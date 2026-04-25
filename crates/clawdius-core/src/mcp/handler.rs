use super::protocol::*;
use crate::graph_rag::ast::FileInfo;
use crate::graph_rag::languages::{detect_language, supported_extensions};
use crate::graph_rag::parser::CodeParser;
use crate::graph_rag::store::GraphStore;
use crate::llm::{ChatMessage, ChatRole, LlmConfig};
use crate::llm::providers::LlmClient;
use crate::tools::web_search::{SearchProvider, WebSearchTool};
use std::sync::LazyLock;
use walkdir::WalkDir;

static TOKIO_RT: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});

static CODE_STORE: LazyLock<std::sync::Mutex<Option<GraphStore>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));

/// Handle a single MCP JSON-RPC request and return the response.
/// This is transport-agnostic — both HTTP and stdio transports call this.
pub fn handle_mcp_request(request: &McpRequest) -> McpResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "notifications/initialized" => McpResponse::notification(),
        "ping" => McpResponse::success(request.id, serde_json::json!({})),
        "tools/list" => handle_tools_list(request),
        "tools/call" => handle_tools_call(request),
        "resources/list" => handle_resources_list(request),
        "prompts/list" => handle_prompts_list(request),
        _ => McpResponse::error(request.id, McpError::method_not_found(&request.method)),
    }
}

fn handle_initialize(request: &McpRequest) -> McpResponse {
    let result = serde_json::json!({
        "protocolVersion": MCP_VERSION,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "listChanged": false, "subscribe": false },
            "prompts": { "listChanged": false }
        },
        "serverInfo": {
            "name": "clawdius-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    });
    McpResponse::success(request.id, result)
}

fn handle_tools_list(request: &McpRequest) -> McpResponse {
    let tools = vec![
        McpTool::new("read_file", "Read the contents of a file").with_string_param(
            "path",
            "Absolute path to the file",
            true,
        ),
        McpTool::new(
            "list_directory",
            "List files and directories at a given path",
        )
        .with_string_param("path", "Directory path to list", true),
        McpTool::new("git_status", "Show short git status output"),
        McpTool::new("git_log", "Show recent git commits").with_string_param(
            "count",
            "Number of commits to show (default 10)",
            false,
        ),
        McpTool::new("git_diff", "Show git diff output"),
        McpTool::new("check_build", "Run cargo check and return the result"),
        McpTool::new("write_file", "Write content to a file")
            .with_string_param("path", "File path relative to workspace root", true)
            .with_string_param("content", "Content to write", true),
        McpTool::new(
            "edit_file",
            "Replace first occurrence of old_string with new_string in a file",
        )
        .with_string_param("path", "File path relative to workspace root", true)
        .with_string_param("old_string", "Text to find and replace", true)
        .with_string_param("new_string", "Replacement text", true),
        McpTool::new("web_search", "Search the web using DuckDuckGo")
            .with_string_param("query", "Search query", true)
            .with_string_param(
                "max_results",
                "Maximum number of results (default 5)",
                false,
            ),
        McpTool::new("generate_code", "Generate or edit code using an LLM")
            .with_string_param("prompt", "Description of the code to generate", true)
            .with_string_param("language", "Target programming language", false)
            .with_string_param("context", "Existing code to edit or build upon", false),
        McpTool::new(
            "codebase_search",
            "Search codebase symbols indexed via tree-sitter (lazy-indexed on first call)",
        )
        .with_string_param(
            "query",
            "Search query matched against symbol names, signatures, and doc comments",
            true,
        )
        .with_string_param(
            "language",
            "Filter results by file extension (e.g. 'rs', 'py', 'ts')",
            false,
        ),
        McpTool::new(
            "run_tests",
            "Run a test command with optional filter and 120s timeout",
        )
        .with_string_param(
            "command",
            "Test command to run (default: 'cargo test --lib')",
            false,
        )
        .with_string_param(
            "filter",
            "Filter argument appended to the test command",
            false,
        ),
    ];
    McpResponse::success(request.id, serde_json::json!({ "tools": tools }))
}

fn handle_tools_call(request: &McpRequest) -> McpResponse {
    let params = request.params.as_ref().unwrap_or(&serde_json::Value::Null);

    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return McpResponse::error(request.id, McpError::invalid_params("missing tool name"));
        },
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match name {
        "read_file" => tool_read_file(&arguments),
        "list_directory" => tool_list_directory(&arguments),
        "git_status" => tool_git_status(),
        "git_log" => tool_git_log(&arguments),
        "git_diff" => tool_git_diff(),
        "check_build" => tool_check_build(),
        "write_file" => tool_write_file(&arguments),
        "edit_file" => tool_edit_file(&arguments),
        "web_search" => tool_web_search(&arguments),
        "generate_code" => tool_generate_code(&arguments),
        "codebase_search" => tool_codebase_search(&arguments),
        "run_tests" => tool_run_tests(&arguments),
        _ => {
            return McpResponse::error(
                request.id,
                McpError::invalid_params(format!("unknown tool: {name}")),
            );
        },
    };

    McpResponse::success(request.id, serde_json::to_value(result).unwrap_or_default())
}

fn handle_resources_list(request: &McpRequest) -> McpResponse {
    McpResponse::success(request.id, serde_json::json!({ "resources": [] }))
}

fn handle_prompts_list(request: &McpRequest) -> McpResponse {
    McpResponse::success(request.id, serde_json::json!({ "prompts": [] }))
}

fn tool_read_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    match std::fs::read_to_string(path) {
        Ok(content) => McpToolResult::text(content),
        Err(e) => McpToolResult::error(format!("failed to read file: {e}")),
    }
}

fn tool_list_directory(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut lines = Vec::new();
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let kind = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "dir"
                } else {
                    "file"
                };
                lines.push(format!("{kind} {file_name}"));
            }
            McpToolResult::text(lines.join("\n"))
        },
        Err(e) => McpToolResult::error(format!("failed to list directory: {e}")),
    }
}

fn tool_git_status() -> McpToolResult {
    match std::process::Command::new("git")
        .args(["status", "--short"])
        .output()
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.is_empty() {
                McpToolResult::text("(clean working tree)")
            } else {
                McpToolResult::text(text)
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run git status: {e}")),
    }
}

fn tool_git_log(args: &serde_json::Value) -> McpToolResult {
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10);

    match std::process::Command::new("git")
        .args(["log", &format!("-{count}"), "--oneline"])
        .output()
    {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            McpToolResult::text(text)
        },
        Err(e) => McpToolResult::error(format!("failed to run git log: {e}")),
    }
}

fn tool_git_diff() -> McpToolResult {
    match std::process::Command::new("git").arg("diff").output() {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.is_empty() {
                McpToolResult::text("(no diff)")
            } else {
                McpToolResult::text(text)
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run git diff: {e}")),
    }
}

fn tool_check_build() -> McpToolResult {
    match std::process::Command::new("cargo").arg("check").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stdout.is_empty() { stderr } else { stdout };
            if output.status.success() {
                McpToolResult::text(format!("cargo check passed:\n{combined}"))
            } else {
                McpToolResult::error(format!("cargo check failed:\n{combined}"))
            }
        },
        Err(e) => McpToolResult::error(format!("failed to run cargo check: {e}")),
    }
}

fn validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    if path.contains("..") {
        return Err("path traversal ('..') is not allowed".to_string());
    }
    let workspace_root =
        std::env::current_dir().map_err(|e| format!("failed to get workspace root: {e}"))?;
    let resolved = workspace_root.join(path);
    let resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !resolved.starts_with(&workspace_root) {
        return Err("path is outside the workspace root".to_string());
    }
    Ok(resolved)
}

fn tool_write_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };
    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return McpToolResult::error("missing 'content' parameter"),
    };

    let resolved = match validate_path(path) {
        Ok(p) => p,
        Err(e) => return McpToolResult::error(e),
    };

    if let Some(parent) = resolved.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return McpToolResult::error(format!("failed to create directories: {e}"));
        }
    }

    match std::fs::write(&resolved, content) {
        Ok(()) => {
            let bytes = content.len();
            McpToolResult::text(format!("wrote {bytes} bytes to {path}"))
        },
        Err(e) => McpToolResult::error(format!("failed to write file: {e}")),
    }
}

fn tool_edit_file(args: &serde_json::Value) -> McpToolResult {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return McpToolResult::error("missing 'path' parameter"),
    };
    let old_string = match args.get("old_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return McpToolResult::error("missing 'old_string' parameter"),
    };
    let new_string = match args.get("new_string").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return McpToolResult::error("missing 'new_string' parameter"),
    };

    let resolved = match validate_path(path) {
        Ok(p) => p,
        Err(e) => return McpToolResult::error(e),
    };

    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(e) => return McpToolResult::error(format!("failed to read file: {e}")),
    };

    // Use edit cascade for fuzzy matching fallback
    let cascade_params = crate::tools::edit_cascade::EditParams {
        content: &content,
        old_text: old_string,
        new_text: new_string,
        replace_all: false,
    };

    match crate::tools::edit_cascade::apply_edit_cascade(&cascade_params) {
        Ok(result) => {
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
            match std::fs::write(&resolved, &result.new_content) {
                Ok(()) => McpToolResult::text(format!(
                    "edited {path} successfully{strategy_note}"
                )),
                Err(e) => McpToolResult::error(format!("failed to write file: {e}")),
            }
        }
        Err(e) => McpToolResult::error(format!(
            "old_string not found in file.\nEdit cascade diagnostics:\n{e}"
        )),
    }
}

fn tool_web_search(args: &serde_json::Value) -> McpToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.trim().is_empty() => q,
        _ => return McpToolResult::error("missing or empty 'query' parameter"),
    };

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    let tool = WebSearchTool::new(SearchProvider::DuckDuckGo);
    match TOKIO_RT.block_on(tool.search(query, max_results)) {
        Ok(results) => {
            if results.is_empty() {
                McpToolResult::text("No results found.")
            } else {
                let mut output = String::new();
                for (i, r) in results.iter().enumerate() {
                    output.push_str(&format!(
                        "[{}] {}\n   URL: {}\n   {}\n\n",
                        i + 1,
                        r.title,
                        r.url,
                        r.snippet
                    ));
                }
                McpToolResult::text(output)
            }
        },
        Err(e) => McpToolResult::error(format!("search failed: {e}")),
    }
}

fn tool_generate_code(args: &serde_json::Value) -> McpToolResult {
    let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.trim().is_empty() => p,
        _ => return McpToolResult::error("missing or empty 'prompt' parameter"),
    };

    let language = args.get("language").and_then(|v| v.as_str());
    let context = args.get("context").and_then(|v| v.as_str());

    let provider_name = std::env::var("CLAWDIUS_PROVIDER").unwrap_or_else(|_| "anthropic".into());

    let config = match LlmConfig::from_env(&provider_name) {
        Ok(c) => c,
        Err(e) => {
            return McpToolResult::error(format!(
                "LLM not configured: {e}. Set CLAWDIUS_PROVIDER (anthropic/openai/ollama) and the corresponding API key env var (e.g. ANTHROPIC_API_KEY, OPENAI_API_KEY)."
            ));
        },
    };

    let provider = match crate::llm::create_provider(&config) {
        Ok(p) => p,
        Err(e) => return McpToolResult::error(format!("failed to create LLM provider: {e}")),
    };

    let mut user_msg = String::new();
    if let Some(lang) = language {
        user_msg.push_str(&format!("Language: {lang}\n"));
    }
    if let Some(ctx) = context {
        user_msg.push_str(&format!("Existing code:\n```\n{ctx}\n```\n\n"));
    }
    user_msg.push_str(prompt);

    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: "You are an expert programmer. Generate clean code.".to_string(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: user_msg,
        },
    ];

    match TOKIO_RT.block_on(provider.chat(messages)) {
        Ok(response) => McpToolResult::text(response),
        Err(e) => McpToolResult::error(format!("LLM generation failed: {e}")),
    }
}

fn ensure_indexed() -> Result<(), String> {
    let mut guard = CODE_STORE.lock().map_err(|e| format!("lock error: {e}"))?;
    if guard.is_some() {
        return Ok(());
    }

    let store = GraphStore::open_in_memory().map_err(|e| format!("failed to open store: {e}"))?;
    let parser = CodeParser::new().map_err(|e| format!("failed to create parser: {e}"))?;
    let extensions: Vec<&str> = supported_extensions();

    let root = std::env::current_dir().map_err(|e| format!("failed to get cwd: {e}"))?;

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != "target"
                && name != "node_modules"
                && name != ".git"
                && name != "dist"
                && name != "build"
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = match entry.path().extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => continue,
        };
        if !extensions.contains(&ext.as_str()) {
            continue;
        }

        let lang = match detect_language(entry.path()) {
            Some(l) => l,
            None => continue,
        };

        let source = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let relative_path = entry.path().strip_prefix(&root).unwrap_or(entry.path());
        let path_str = relative_path.to_string_lossy().to_string();

        let file_info = FileInfo {
            path: path_str,
            hash: blake3::hash(source.as_bytes()).to_hex().to_string(),
            language: Some(lang.as_str().to_string()),
            last_modified: None,
        };

        let file_id = match store.insert_file(&file_info) {
            Ok(id) => id,
            Err(_) => continue,
        };

        let tree = match parser.parse(&source, lang) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let symbols = parser.extract_symbols(&tree, &source, file_id, lang);
        for symbol in &symbols {
            let _ = store.insert_symbol(symbol);
        }
    }

    *guard = Some(store);
    Ok(())
}

fn tool_codebase_search(args: &serde_json::Value) -> McpToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.trim().is_empty() => q,
        _ => return McpToolResult::error("missing or empty 'query' parameter"),
    };

    let language_filter = args.get("language").and_then(|v| v.as_str());

    if let Err(e) = ensure_indexed() {
        return McpToolResult::error(format!("failed to index codebase: {e}"));
    }

    let guard = match CODE_STORE.lock() {
        Ok(g) => g,
        Err(e) => return McpToolResult::error(format!("lock error: {e}")),
    };

    let store = match guard.as_ref() {
        Some(s) => s,
        None => return McpToolResult::error("store not initialized"),
    };

    let symbols = match store.search_symbols(query) {
        Ok(s) => s,
        Err(e) => return McpToolResult::error(format!("search failed: {e}")),
    };

    let mut results = Vec::new();
    for sym in &symbols {
        if let Some(lang) = language_filter {
            if let Ok(Some(file)) = store.get_file_by_id(sym.file_id) {
                let matches_ext = file
                    .path
                    .rsplit('.')
                    .next()
                    .map(|ext| ext.eq_ignore_ascii_case(lang))
                    .unwrap_or(false);
                if !matches_ext {
                    continue;
                }
            }
        }

        let sig = sym.signature.as_deref().unwrap_or("");
        let doc = sym.doc_comment.as_deref().unwrap_or("");
        let file_path = store
            .get_file_by_id(sym.file_id)
            .ok()
            .flatten()
            .map(|f| f.path)
            .unwrap_or_else(|| "?".to_string());
        results.push(format!(
            "{} [{}] ({} L{}-L{}) sig: {}",
            sym.name,
            sym.kind.as_str(),
            file_path,
            sym.start_line,
            sym.end_line,
            sig,
        ));
        if !doc.is_empty() {
            let last = results.last_mut().unwrap();
            let doc_preview: String = doc.lines().take(3).collect::<Vec<_>>().join("\n       ");
            last.push_str(&format!("\n  doc: {}", doc_preview));
        }
    }

    if results.is_empty() {
        McpToolResult::text(format!("No symbols found matching '{query}'"))
    } else {
        McpToolResult::text(format!(
            "Found {} symbols matching '{}':\n\n{}",
            results.len(),
            query,
            results.join("\n\n")
        ))
    }
}

fn tool_run_tests(args: &serde_json::Value) -> McpToolResult {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("cargo test --lib");
    let filter = args.get("filter").and_then(|v| v.as_str());

    let full_command = match filter {
        Some(f) => format!("{command} {f}"),
        None => command.to_string(),
    };

    let parts: Vec<String> = full_command.split_whitespace().map(String::from).collect();
    if parts.is_empty() {
        return McpToolResult::error("empty command");
    }

    // Security: reject commands with shell metacharacters to prevent injection
    let joined = parts.join(" ");
    if joined.contains('|')
        || joined.contains('>')
        || joined.contains(';')
        || joined.contains("&&")
        || joined.contains("$(")
        || joined.contains('`')
    {
        return McpToolResult::error(
            "Shell metacharacters not allowed in test command (|, >, ;, &&, $(, `). Use a single command.".to_string(),
        );
    }

    let (cmd, cmd_args) = (parts[0].clone(), parts[1..].to_vec());
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let output = std::process::Command::new(&cmd).args(&cmd_args).output();
        let _ = tx.send(output);
    });

    match rx.recv_timeout(std::time::Duration::from_secs(120)) {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined = if stdout.is_empty() { stderr } else { stdout };
            if output.status.success() {
                McpToolResult::text(format!("Tests passed:\n{combined}"))
            } else {
                McpToolResult::text(format!("Tests failed:\n{combined}"))
            }
        },
        Ok(Err(e)) => McpToolResult::error(format!("failed to run command: {e}")),
        Err(_) => McpToolResult::error("command timed out after 120 seconds"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        let req = McpRequest::new(1, "initialize");
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "clawdius-server");
        assert_eq!(result["protocolVersion"], MCP_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_notification() {
        let req = McpRequest::new(0, "notifications/initialized");
        let resp = handle_mcp_request(&req);
        assert!(resp.is_notification());
        assert!(resp.result.is_none());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_ping() {
        let req = McpRequest::new(1, "ping");
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn test_tools_list() {
        let req = McpRequest::new(1, "tools/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        let tools = result["tools"]
            .as_array()
            .expect("tools should be an array");
        assert_eq!(tools.len(), 12);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"git_status"));
        assert!(names.contains(&"git_diff"));
        assert!(names.contains(&"check_build"));
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"generate_code"));
        assert!(names.contains(&"codebase_search"));
        assert!(names.contains(&"run_tests"));
    }

    #[test]
    fn test_tools_call_fixed_params() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "git_status"
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_tools_call_missing_name() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn test_tools_call_unknown_tool() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "nonexistent_tool"
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn test_method_not_found() {
        let req = McpRequest::new(1, "foo/bar");
        let resp = handle_mcp_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn test_resources_list() {
        let req = McpRequest::new(1, "resources/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["resources"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_prompts_list() {
        let req = McpRequest::new(1, "prompts/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["prompts"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_read_file() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = format!("{cargo_path}/Cargo.toml");
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "read_file",
            "arguments": { "path": path }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("clawdius-core"));
    }

    #[test]
    fn test_read_file_missing_param() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "read_file",
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
    }

    #[test]
    fn test_list_directory() {
        let src_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "list_directory",
            "arguments": { "path": format!("{src_path}/src") }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("lib.rs") || text.contains("mcp"));
    }

    #[test]
    fn test_notification_request_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: McpRequest =
            serde_json::from_str(json).expect("should parse notification without id");
        assert_eq!(req.id, 0);
        let resp = handle_mcp_request(&req);
        assert!(resp.is_notification());
    }

    #[test]
    fn test_input_schema_camelcase() {
        let tool = McpTool::new("test", "Test");
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("inputSchema"));
        assert!(!json.contains("input_schema"));
    }

    #[test]
    fn test_write_file_creates_file() {
        let dir = std::env::temp_dir().join("clawdius_test_write");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("sub").join("test.txt");
        let relative = format!("sub/test.txt");

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "write_file",
            "arguments": { "path": relative, "content": "hello world" }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("11 bytes"));
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_file_rejects_traversal() {
        let dir = std::env::temp_dir().join("clawdius_test_traversal");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "write_file",
            "arguments": { "path": "../etc/passwd", "content": "hack" }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("traversal"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_edit_file_replaces_content() {
        let dir = std::env::temp_dir().join("clawdius_test_edit");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("edit_me.txt"), "foo bar baz").unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "edit_file",
            "arguments": {
                "path": "edit_me.txt",
                "old_string": "bar",
                "new_string": "qux"
            }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.join("edit_me.txt")).unwrap(),
            "foo qux baz"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_edit_file_old_string_not_found() {
        let dir = std::env::temp_dir().join("clawdius_test_edit_nf");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("missing.txt"), "hello").unwrap();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "edit_file",
            "arguments": {
                "path": "missing.txt",
                "old_string": "goodbye",
                "new_string": "world"
            }
        }));
        let resp = handle_mcp_request(&req);
        std::env::set_current_dir(&saved).unwrap();

        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_web_search_empty_query_returns_error() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "web_search",
            "arguments": { "query": "" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing or empty"));
    }

    #[test]
    fn test_web_search_missing_query_returns_error() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "web_search",
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
    }

    #[test]
    fn test_generate_code_no_api_key_returns_error() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::set_var("CLAWDIUS_PROVIDER", "anthropic");

        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "generate_code",
            "arguments": { "prompt": "write hello world" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("ANTHROPIC_API_KEY") || text.contains("LLM not configured"));

        std::env::remove_var("CLAWDIUS_PROVIDER");
    }

    #[test]
    fn test_generate_code_missing_prompt_returns_error() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "generate_code",
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing or empty"));
    }

    #[test]
    fn test_codebase_search_empty_query_returns_error() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "codebase_search",
            "arguments": { "query": "" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing or empty"));
    }

    #[test]
    fn test_codebase_search_missing_query_returns_error() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "codebase_search",
            "arguments": {}
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result["is_error"].as_bool().unwrap());
    }

    #[test]
    fn test_run_tests_custom_command_success() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "run_tests",
            "arguments": { "command": "echo test_ok" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("test_ok"));
    }

    #[test]
    fn test_run_tests_custom_command_failure() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "run_tests",
            "arguments": { "command": "sh -c 'exit 1'" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Tests failed"));
    }

    #[test]
    fn test_run_tests_with_filter() {
        let req = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
            "name": "run_tests",
            "arguments": { "command": "echo filter_test", "filter": "my_test" }
        }));
        let resp = handle_mcp_request(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(!result["is_error"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("filter_test"));
    }

    #[test]
    fn test_tools_list_includes_new_tools() {
        let req = McpRequest::new(1, "tools/list");
        let resp = handle_mcp_request(&req);
        let result = resp.result.unwrap();
        let tools = result["tools"]
            .as_array()
            .expect("tools should be an array");
        assert_eq!(tools.len(), 12);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"codebase_search"));
        assert!(names.contains(&"run_tests"));
    }
}
