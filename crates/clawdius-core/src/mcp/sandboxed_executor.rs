use super::protocol::McpToolResult;
use crate::sandbox::executor::SandboxExecutor;
use crate::sandbox::tiers::SandboxConfig;
use crate::sandbox::SandboxTier;
use crate::session::storage::{LocalFsBackend, Vfs, VfsError};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSandboxTier {
    Unrestricted,
    Filtered,
    Bubblewrap,
    Wasm,
}

#[derive(Debug, Clone)]
pub struct SandboxedToolExecutor {
    working_dir: PathBuf,
    default_tier: ToolSandboxTier,
    tool_tiers: std::collections::HashMap<String, ToolSandboxTier>,
    path_allowlist: Vec<PathBuf>,
    vfs: Arc<dyn Vfs>,
}

impl SandboxedToolExecutor {
    #[must_use] 
    pub fn new(working_dir: PathBuf) -> Self {
        let vfs = Arc::new(LocalFsBackend::new(&working_dir));
        Self {
            working_dir,
            default_tier: ToolSandboxTier::Filtered,
            tool_tiers: std::collections::HashMap::new(),
            path_allowlist: Vec::new(),
            vfs,
        }
    }

    /// Create with a custom VFS backend for testing.
    pub fn with_vfs(working_dir: PathBuf, vfs: Arc<dyn Vfs>) -> Self {
        Self {
            working_dir,
            default_tier: ToolSandboxTier::Filtered,
            tool_tiers: std::collections::HashMap::new(),
            path_allowlist: Vec::new(),
            vfs,
        }
    }

    /// Get a clone of the VFS backend.
    #[must_use] 
    pub fn vfs(&self) -> Arc<dyn Vfs> {
        Arc::clone(&self.vfs)
    }

    #[must_use] 
    pub const fn with_default_tier(mut self, tier: ToolSandboxTier) -> Self {
        self.default_tier = tier;
        self
    }

    pub fn with_tool_tier(mut self, tool: impl Into<String>, tier: ToolSandboxTier) -> Self {
        self.tool_tiers.insert(tool.into(), tier);
        self
    }

    #[must_use] 
    pub fn with_path_allowlist(mut self, paths: Vec<PathBuf>) -> Self {
        self.path_allowlist = paths;
        self
    }

    fn tier_for_tool(&self, tool_name: &str) -> ToolSandboxTier {
        self.tool_tiers
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_tier)
    }

    fn vfs_err_to_string(e: VfsError) -> String {
        match e {
            VfsError::NotFound(p) => format!("path not found: {}", p.display()),
            VfsError::PermissionDenied(p) => format!("permission denied: {}", p.display()),
            VfsError::PathTraversal(p) => format!("path traversal attempt: {}", p.display()),
            VfsError::Io(io_err) => format!("I/O error: {io_err}"),
            other => format!("{other}"),
        }
    }

    pub fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        if path.contains("..") {
            return Err("path traversal ('..') is not allowed".to_string());
        }

        let canonical_root = self
            .vfs
            .canonicalize(Path::new(&self.working_dir))
            .map_err(Self::vfs_err_to_string)?;

        let requested = PathBuf::from(path);
        let resolved = if requested.is_absolute() {
            requested
        } else {
            canonical_root.join(&requested)
        };

        // Try VFS canonicalize for paths that exist
        let resolved_canonical = if self.vfs.exists(&resolved) {
            if let Ok(p) = self.vfs.canonicalize(&resolved) { p } else {
                // VFS rejected it (outside root). Check allowlist using std::fs
                // since allowlisted paths are intentionally outside the VFS root.
                if !self.check_allowlist(&resolved) {
                    return Err(format!(
                        "path '{path}' escapes the working directory {}",
                        canonical_root.display()
                    ));
                }
                resolved.clone()
            }
        } else {
            resolved
        };

        if !resolved_canonical.starts_with(&canonical_root)
            && !self.check_allowlist(&resolved_canonical) {
                return Err(format!(
                    "path '{path}' escapes the working directory {}",
                    canonical_root.display()
                ));
            }

        Ok(resolved_canonical)
    }

    /// Check if a path falls within any allowlisted directory using `std::fs` canonicalize
    /// (since allowlisted paths may be outside the VFS root).
    fn check_allowlist(&self, resolved: &Path) -> bool {
        self.path_allowlist.iter().any(|allowed| {
            let allowed_canonical = std::fs::canonicalize(allowed).ok();
            let resolved_canonical = std::fs::canonicalize(resolved).ok();
            match (allowed_canonical, resolved_canonical) {
                (Some(a), Some(r)) => r.starts_with(&a),
                _ => false,
            }
        })
    }

    #[must_use] 
    pub fn execute_tool(&self, tool_name: &str, args: &serde_json::Value) -> McpToolResult {
        let tier = self.tier_for_tool(tool_name);

        match tool_name {
            "read_file" => self.execute_read_file(args, tier),
            "write_file" => self.execute_write_file(args, tier),
            "edit_file" => self.execute_edit_file(args, tier),
            "list_directory" => self.execute_list_directory(args, tier),
            "run_tests" | "check_build" | "execute_code" => {
                self.execute_shell_command(tool_name, args, tier)
            },
            _ => McpToolResult::error(format!("unknown tool for sandboxed execution: {tool_name}")),
        }
    }

    fn execute_read_file(&self, args: &serde_json::Value, _tier: ToolSandboxTier) -> McpToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return McpToolResult::error("missing 'path' parameter"),
        };

        let resolved = match self.validate_path(path) {
            Ok(p) => p,
            Err(e) => return McpToolResult::error(e),
        };

        match self.vfs.read_text(&resolved) {
            Ok(content) => McpToolResult::text(content),
            Err(e) => McpToolResult::error(Self::vfs_err_to_string(e)),
        }
    }

    fn execute_write_file(
        &self,
        args: &serde_json::Value,
        _tier: ToolSandboxTier,
    ) -> McpToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return McpToolResult::error("missing 'path' parameter"),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return McpToolResult::error("missing 'content' parameter"),
        };

        let resolved = match self.validate_path(path) {
            Ok(p) => p,
            Err(e) => return McpToolResult::error(e),
        };

        // ── Protected files guard ──────────────────────────────────────────
        // Shared guard across all write paths to prevent overwriting critical
        // crate root files with hallucinated content.
        if let Err(e) = crate::agentic::file_ops::FileOperations::validate_protected_file_write(
            &resolved, content,
        ) {
            return McpToolResult::error(e);
        }

        if let Some(parent) = resolved.parent() {
            if let Err(e) = self.vfs.create_dir_all(parent) {
                return McpToolResult::error(Self::vfs_err_to_string(e));
            }
        }

        match self.vfs.write_text(&resolved, content) {
            Ok(()) => McpToolResult::text(format!("wrote {} bytes to {path}", content.len())),
            Err(e) => McpToolResult::error(Self::vfs_err_to_string(e)),
        }
    }

    fn execute_edit_file(&self, args: &serde_json::Value, _tier: ToolSandboxTier) -> McpToolResult {
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

        let resolved = match self.validate_path(path) {
            Ok(p) => p,
            Err(e) => return McpToolResult::error(e),
        };

        let content = match self.vfs.read_text(&resolved) {
            Ok(c) => c,
            Err(e) => return McpToolResult::error(Self::vfs_err_to_string(e)),
        };

        if !content.contains(old_string) {
            return McpToolResult::error("old_string not found in file");
        }

        let new_content = content.replacen(old_string, new_string, 1);

        match self.vfs.write_text(&resolved, &new_content) {
            Ok(()) => McpToolResult::text(format!("edited {path} successfully")),
            Err(e) => McpToolResult::error(Self::vfs_err_to_string(e)),
        }
    }

    fn execute_list_directory(
        &self,
        args: &serde_json::Value,
        _tier: ToolSandboxTier,
    ) -> McpToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return McpToolResult::error("missing 'path' parameter"),
        };

        let resolved = match self.validate_path(path) {
            Ok(p) => p,
            Err(e) => return McpToolResult::error(e),
        };

        match self.vfs.read_dir(&resolved) {
            Ok(entries) => {
                let mut lines = Vec::new();
                for entry in entries {
                    let file_name = entry
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let kind = if entry.metadata.is_dir { "dir" } else { "file" };
                    lines.push(format!("{kind} {file_name}"));
                }
                McpToolResult::text(lines.join("\n"))
            },
            Err(e) => McpToolResult::error(Self::vfs_err_to_string(e)),
        }
    }

    fn execute_shell_command(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        tier: ToolSandboxTier,
    ) -> McpToolResult {
        let command = match tier {
            ToolSandboxTier::Unrestricted => self.build_command(tool_name, args),
            ToolSandboxTier::Filtered | ToolSandboxTier::Bubblewrap | ToolSandboxTier::Wasm => {
                match self.try_sandboxed_execution(tool_name, args, tier) {
                    Ok(result) => return result,
                    Err(warning) => {
                        tracing::warn!("{warning}");
                        self.build_command(tool_name, args)
                    },
                }
            },
        };

        let parts: Vec<String> = command.split_whitespace().map(String::from).collect();
        if parts.is_empty() {
            return McpToolResult::error("empty command");
        }

        let (cmd, cmd_args) = (parts[0].clone(), parts[1..].to_vec());
        let (tx, rx) = std::sync::mpsc::channel();

        let cwd = self.working_dir.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new(&cmd)
                .args(&cmd_args)
                .current_dir(&cwd)
                .output();
            let _ = tx.send(output);
        });

        match rx.recv_timeout(std::time::Duration::from_secs(120)) {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stdout.is_empty() { stderr } else { stdout };
                if output.status.success() {
                    McpToolResult::text(combined)
                } else {
                    McpToolResult::error(format!("{tool_name} failed:\n{combined}"))
                }
            },
            Ok(Err(e)) => McpToolResult::error(format!("failed to run command: {e}")),
            Err(_) => McpToolResult::error("command timed out after 120 seconds"),
        }
    }

    fn build_command(&self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "run_tests" => {
                let cmd = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cargo test --lib");
                let filter = args.get("filter").and_then(|v| v.as_str());
                match filter {
                    Some(f) => format!("{cmd} {f}"),
                    None => cmd.to_string(),
                }
            },
            "check_build" => "cargo check".to_string(),
            "execute_code" => {
                let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                cmd.to_string()
            },
            _ => String::new(),
        }
    }

    fn try_sandboxed_execution(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        tier: ToolSandboxTier,
    ) -> std::result::Result<McpToolResult, String> {
        match tier {
            ToolSandboxTier::Bubblewrap | ToolSandboxTier::Wasm => {
                let sandbox_tier = match tier {
                    ToolSandboxTier::Bubblewrap => SandboxTier::Untrusted,
                    ToolSandboxTier::Wasm => SandboxTier::Hardened,
                    _ => return Err("unsupported tier".to_string()),
                };

                let config = SandboxConfig {
                    tier: sandbox_tier,
                    network: false,
                    mounts: vec![],
                };

                let executor = SandboxExecutor::new_with_fallback(sandbox_tier, config);
                let command = self.build_command(tool_name, args);
                let parts: Vec<&str> = command.split_whitespace().collect();
                if parts.is_empty() {
                    return Ok(McpToolResult::error("empty command"));
                }

                let (cmd, cmd_args) = (parts[0], &parts[1..]);
                match executor.execute(cmd, cmd_args, &self.working_dir) {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let combined = if stdout.is_empty() { stderr } else { stdout };
                        if output.status.success() {
                            Ok(McpToolResult::text(combined))
                        } else {
                            Ok(McpToolResult::error(format!(
                                "{tool_name} failed in sandbox:\n{combined}"
                            )))
                        }
                    },
                    Err(e) => Err(format!(
                        "sandbox execution failed, falling back to direct: {e}"
                    )),
                }
            },
            ToolSandboxTier::Filtered => Err("filtered tier uses direct execution".to_string()),
            ToolSandboxTier::Unrestricted => {
                Err("unrestricted tier uses direct execution".to_string())
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_executor(dir: &std::path::Path) -> SandboxedToolExecutor {
        SandboxedToolExecutor::new(dir.to_path_buf())
            .with_default_tier(ToolSandboxTier::Filtered)
            .with_tool_tier("read_file", ToolSandboxTier::Filtered)
            .with_tool_tier("write_file", ToolSandboxTier::Filtered)
            .with_tool_tier("edit_file", ToolSandboxTier::Filtered)
    }

    #[test]
    fn test_path_traversal_blocked_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let result = executor.validate_path("../../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
    }

    #[test]
    fn test_path_traversal_blocked_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let result = executor.validate_path("/etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // VFS canonicalize rejects absolute paths outside root with PathTraversal
        assert!(
            err.contains("traversal") || err.contains("working directory"),
            "expected traversal or working directory error, got: {err}"
        );
    }

    #[test]
    fn test_working_directory_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        // Write via VFS (which writes to the working dir)
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let result = executor.validate_path("test.txt");
        assert!(result.is_ok());

        let result = executor.validate_path("subdir/../test.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_within_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        // Write directly to temp dir (VFS base path points here)
        std::fs::write(dir.path().join("hello.txt"), "hello world").unwrap();

        let args = serde_json::json!({"path": "hello.txt"});
        let result = executor.execute_tool("read_file", &args);

        assert!(!result.is_error);
        let text = &result.content[0];
        match text {
            super::super::protocol::McpContent::Text { text } => {
                assert_eq!(text, "hello world");
            },
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn test_read_file_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let args = serde_json::json!({"path": "../../../etc/passwd"});
        let result = executor.execute_tool("read_file", &args);

        assert!(result.is_error);
    }

    #[test]
    fn test_write_file_within_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let args = serde_json::json!({"path": "new_file.txt", "content": "new content"});
        let result = executor.execute_tool("write_file", &args);

        assert!(!result.is_error);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new_file.txt")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn test_write_file_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let args = serde_json::json!({"path": "../../../tmp/evil.txt", "content": "hack"});
        let result = executor.execute_tool("write_file", &args);

        assert!(result.is_error);
    }

    #[test]
    fn test_edit_file_within_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        std::fs::write(dir.path().join("edit.txt"), "foo bar baz").unwrap();

        let args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "bar",
            "new_string": "qux"
        });
        let result = executor.execute_tool("edit_file", &args);

        assert!(!result.is_error);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("edit.txt")).unwrap(),
            "foo qux baz"
        );
    }

    #[test]
    fn test_list_directory_within_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();

        let args = serde_json::json!({"path": "."});
        let result = executor.execute_tool("list_directory", &args);

        assert!(!result.is_error);
        let text = &result.content[0];
        match text {
            super::super::protocol::McpContent::Text { text } => {
                assert!(text.contains("a.txt"));
                assert!(text.contains("subdir"));
            },
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn test_list_directory_blocks_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let args = serde_json::json!({"path": "../../"});
        let result = executor.execute_tool("list_directory", &args);

        assert!(result.is_error);
    }

    #[test]
    fn test_run_tests_echo_command() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let args = serde_json::json!({"command": "echo sandbox_test_ok"});
        let result = executor.execute_tool("run_tests", &args);

        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_tier_override() {
        let dir = tempfile::tempdir().unwrap();
        let executor = SandboxedToolExecutor::new(dir.path().to_path_buf())
            .with_default_tier(ToolSandboxTier::Unrestricted)
            .with_tool_tier("read_file", ToolSandboxTier::Wasm);

        assert_eq!(executor.tier_for_tool("read_file"), ToolSandboxTier::Wasm);
        assert_eq!(
            executor.tier_for_tool("write_file"),
            ToolSandboxTier::Unrestricted
        );
    }

    #[test]
    fn test_path_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let allowed = tempfile::tempdir().unwrap();

        let executor = SandboxedToolExecutor::new(dir.path().to_path_buf())
            .with_path_allowlist(vec![allowed.path().to_path_buf()]);

        let result = executor.validate_path("normal_file.txt");
        assert!(result.is_ok());

        let allowed_file = allowed.path().join("allowed.txt");
        std::fs::write(&allowed_file, "allowed").unwrap();

        let relative = allowed
            .path()
            .join("allowed.txt")
            .to_str()
            .unwrap()
            .to_string();
        let result = executor.validate_path(&relative);
        assert!(result.is_ok());

        let result = executor.validate_path("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_tool_returns_correct_results() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        let result =
            executor.execute_tool("read_file", &serde_json::json!({"path": "missing.txt"}));
        assert!(result.is_error);

        let result = executor.execute_tool("unknown_tool", &serde_json::json!({}));
        assert!(result.is_error);
        let text = &result.content[0];
        match text {
            super::super::protocol::McpContent::Text { text } => {
                assert!(text.contains("unknown tool"));
            },
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn test_vfs_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let executor = make_executor(dir.path());

        // VFS should be accessible
        let vfs = executor.vfs();
        assert!(vfs.exists(dir.path()));
    }
}
