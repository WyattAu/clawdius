//! Multi-repo context builder for workspace-aware LLM context injection.
//!
//! [`WorkspaceContextBuilder`] combines repo-maps from multiple projects
//! into a single context string that the LLM can use to understand the
//! full workspace structure.
//!
//! # Output format
//!
//! ```text
//! ## Workspace: my-workspace
//!
//! ### Project: api-server (default) — /home/user/api-server
//! <repo-map for api-server>
//!
//! ### Project: shared-lib — /home/user/shared-lib
//! <repo-map for shared-lib>
//! ```

use crate::error::Result;
use crate::graph_rag::repo_map::{RepoMap, RepoMapBuilder};
use crate::workspace::{Project, WorkspaceManager};
use std::path::Path;

// ─────────────────────────────────────────────────────────
// WorkspaceContextBuilder
// ─────────────────────────────────────────────────────────

/// Builds combined LLM context from multiple projects in a workspace.
pub struct WorkspaceContextBuilder {
    /// Token budget per project (total = per_project × num_projects).
    pub per_project_tokens: usize,
    /// Maximum total tokens across all projects.
    pub max_total_tokens: usize,
    /// Whether to include the default project marker.
    pub mark_default: bool,
}

impl WorkspaceContextBuilder {
    /// Create a new builder with default settings.
    ///
    /// - `per_project_tokens`: 2000 (fits ~3-5 projects in 8k context)
    /// - `max_total_tokens`: 8000
    #[must_use]
    pub fn new() -> Self {
        Self {
            per_project_tokens: 2000,
            max_total_tokens: 8000,
            mark_default: true,
        }
    }

    /// Set per-project token budget.
    #[must_use]
    pub fn per_project_tokens(mut self, tokens: usize) -> Self {
        self.per_project_tokens = tokens;
        self
    }

    /// Set maximum total token budget.
    #[must_use]
    pub fn max_total_tokens(mut self, tokens: usize) -> Self {
        self.max_total_tokens = tokens;
        self
    }

    /// Build context for all projects in a workspace.
    ///
    /// Returns a formatted string suitable for prepending to an LLM prompt.
    pub async fn build<R>(
        &self,
        manager: &WorkspaceManager<R>,
        workspace_id: &crate::workspace::WorkspaceId,
    ) -> Result<String>
    where
        R: crate::storage::WorkspaceRepository,
    {
        let workspace = manager.load_workspace(workspace_id).await?;
        let projects = manager.list_projects(workspace_id).await?;
        let default_project_id = workspace.default_project_id.as_ref();

        self.build_from_projects(&workspace.name, &projects, default_project_id)
    }

    /// Build context from a single project (non-workspace mode).
    ///
    /// This is the backward-compatible path for single-project usage.
    pub fn build_single(project_root: &Path, project_name: Option<&str>) -> Result<String> {
        Self::build_single_with_budget(project_root, project_name, 4096)
    }

    /// Build context from a single project with a custom token budget.
    pub fn build_single_with_budget(
        project_root: &Path,
        project_name: Option<&str>,
        token_budget: usize,
    ) -> Result<String> {
        let name = project_name
            .map(|n| n.to_string())
            .unwrap_or_else(|| {
                project_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project")
                    .to_string()
            });

        let repo_map = RepoMapBuilder::new(project_root.to_path_buf())
            .token_budget(token_budget)
            .build()?;

        if repo_map.tag_count() == 0 {
            Ok(String::new())
        } else {
            Ok(format!(
                "## Project: {name} — {}\n\n{}",
                project_root.display(),
                repo_map.to_string()
            ))
        }
    }

    /// Build context from an explicit list of projects.
    fn build_from_projects(
        &self,
        workspace_name: &str,
        projects: &[Project],
        default_project_id: Option<&crate::workspace::ProjectId>,
    ) -> Result<String> {
        if projects.is_empty() {
            return Ok(String::new());
        }

        let mut sections = Vec::new();

        // Header
        sections.push(format!("## Workspace: {workspace_name}"));

        // Distribute token budget across projects
        let num_projects = projects.len();
        let per_project = if num_projects == 1 {
            self.max_total_tokens
        } else {
            // Reserve some tokens for the header and separators
            let header_reserve = 200;
            let available = self.max_total_tokens.saturating_sub(header_reserve);
            (available / num_projects).min(self.per_project_tokens)
        };

        for project in projects {
            let is_default = default_project_id
                .map(|id| id == &project.id)
                .unwrap_or(false);

            let marker = if self.mark_default && is_default {
                " (default)"
            } else {
                ""
            };

            let repo_map = match RepoMapBuilder::new(project.root_path.clone())
                .token_budget(per_project)
                .build()
            {
                Ok(map) => map,
                Err(e) => {
                    // Log but don't fail — one bad project shouldn't break context
                    eprintln!(
                        "  [workspace-ctx] warning: failed to build repo-map for '{}': {}",
                        project.name, e
                    );
                    continue;
                }
            };

            let header = format!(
                "### Project: {}{marker} — {}",
                project.name,
                project.root_path.display()
            );

            if repo_map.tag_count() == 0 {
                sections.push(format!("{header}\n*(no symbols found)*"));
            } else {
                sections.push(format!("{header}\n\n{}", repo_map.to_string()));
            }
        }

        Ok(sections.join("\n\n"))
    }
}

impl Default for WorkspaceContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryBackend;
    use crate::workspace::WorkspaceManager;

    async fn setup_workspace() -> (
        WorkspaceManager<InMemoryBackend>,
        crate::workspace::WorkspaceId,
        tempfile::TempDir,
    ) {
        let backend = InMemoryBackend::new();
        let manager = WorkspaceManager::new(backend);
        let dir = tempfile::tempdir().unwrap();

        // Create a mini Rust project
        let proj_dir = dir.path().join("test-proj");
        std::fs::create_dir_all(proj_dir.join("src")).unwrap();
        std::fs::write(
            proj_dir.join("Cargo.toml"),
            r#"[package]
name = "test-proj"
version = "0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(
            proj_dir.join("src/main.rs"),
            r#"fn main() {
    println!("hello");
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
        )
        .unwrap();

        let ws = manager.create_workspace("test-workspace").await.unwrap();
        manager.add_project_by_path(&ws.id, &proj_dir).await.unwrap();

        (manager, ws.id, dir)
    }

    #[tokio::test]
    async fn test_build_workspace_context() {
        let (manager, ws_id, _dir) = setup_workspace().await;

        let ctx = WorkspaceContextBuilder::new()
            .build(&manager, &ws_id)
            .await
            .unwrap();

        assert!(ctx.contains("## Workspace: test-workspace"));
        assert!(ctx.contains("### Project: test-proj"));
        assert!(ctx.contains("test-proj") && ctx.contains("(default)"));
    }

    #[tokio::test]
    async fn test_build_single_project_context() {
        let dir = tempfile::tempdir().unwrap();
        let proj_dir = dir.path().join("single-proj");
        std::fs::create_dir_all(proj_dir.join("src")).unwrap();
        std::fs::write(
            proj_dir.join("Cargo.toml"),
            r#"[package]
name = "single-proj"
version = "0.1.0"
"#,
        )
        .unwrap();
        std::fs::write(
            proj_dir.join("src/lib.rs"),
            "pub fn greet() -> &'static str { \"hello\" }\n",
        )
        .unwrap();

        let ctx = WorkspaceContextBuilder::build_single(&proj_dir, Some("single-proj"))
            .unwrap();

        assert!(ctx.contains("## Project: single-proj"));
        assert!(ctx.contains("greet"));
    }

    #[tokio::test]
    async fn test_build_empty_workspace() {
        let backend = InMemoryBackend::new();
        let manager = WorkspaceManager::new(backend);
        let ws = manager.create_workspace("empty-ws").await.unwrap();

        let ctx = WorkspaceContextBuilder::new()
            .build(&manager, &ws.id)
            .await
            .unwrap();

        assert!(ctx.is_empty());
    }

    #[tokio::test]
    async fn test_build_single_project_without_name() {
        let dir = tempfile::tempdir().unwrap();
        let proj_dir = dir.path().join("my-proj");
        std::fs::create_dir_all(proj_dir.join("src")).unwrap();
        std::fs::write(
            proj_dir.join("Cargo.toml"),
            "[package]\nname = \"my-proj\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(proj_dir.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

        let ctx = WorkspaceContextBuilder::build_single(&proj_dir, None)
            .unwrap();

        // Should use directory name
        assert!(ctx.contains("my-proj"));
    }

    #[test]
    fn test_default_token_budgets() {
        let builder = WorkspaceContextBuilder::new();
        assert_eq!(builder.per_project_tokens, 2000);
        assert_eq!(builder.max_total_tokens, 8000);
    }

    #[test]
    fn test_custom_token_budgets() {
        let builder = WorkspaceContextBuilder::new()
            .per_project_tokens(1000)
            .max_total_tokens(4000);
        assert_eq!(builder.per_project_tokens, 1000);
        assert_eq!(builder.max_total_tokens, 4000);
    }
}
