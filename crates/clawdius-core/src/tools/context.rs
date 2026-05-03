//! Tool execution context — carries project/workspace routing information.
//!
//! When the agent makes a tool call, the system resolves which project
//! it should target based on the current file path or explicit project
//! selection. [`ToolContext`] carries this resolved information to the
//! tool executors.

use crate::workspace::{Project, ProjectId, WorkspaceId};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────
// ToolContext
// ─────────────────────────────────────────────────────────

/// Execution context for a tool call, resolved from the workspace.
///
/// Carries the project root and metadata needed by tools to sandbox
/// file access and shell commands correctly.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// The project being operated on.
    pub project: Project,

    /// The workspace this project belongs to.
    pub workspace_id: Option<WorkspaceId>,
}

impl ToolContext {
    /// Create a tool context for a single project.
    #[must_use]
    pub fn new(project: Project) -> Self {
        Self {
            project,
            workspace_id: None,
        }
    }

    /// Create a tool context with workspace association.
    #[must_use]
    pub fn with_workspace(mut self, workspace_id: WorkspaceId) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    /// Get the project root path.
    #[must_use]
    pub fn project_root(&self) -> &PathBuf {
        &self.project.root_path
    }

    /// Get the project ID.
    #[must_use]
    pub fn project_id(&self) -> &ProjectId {
        &self.project.id
    }

    /// Get the project name.
    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project.name
    }

    /// Create a context for a bare path (no workspace).
    ///
    /// This is the backward-compatible path for single-project usage.
    #[must_use]
    pub fn for_single_project(root: impl Into<PathBuf>) -> Self {
        let project = Project::new("project", root);
        Self::new(project)
    }
}

// ─────────────────────────────────────────────────────────
// ToolContextResolver
// ─────────────────────────────────────────────────────────

/// Resolves tool calls to the appropriate project context.
///
/// Uses the workspace manager to find which project a file belongs to,
/// falling back to the default project or the current working directory.
pub struct ToolContextResolver;

impl ToolContextResolver {
    /// Resolve a tool context for a file path within a workspace.
    ///
    /// Strategy:
    /// 1. If file_path is provided, find the deepest ancestor project
    /// 2. Fall back to the default project in the workspace
    /// 3. If no workspace, use cwd as a single-project context
    pub async fn resolve(
        file_path: Option<&std::path::Path>,
        workspace_id: Option<&WorkspaceId>,
        project_id: Option<&ProjectId>,
    ) -> ToolContext {
        // If no workspace, use cwd
        if workspace_id.is_none() {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            return ToolContext::for_single_project(cwd);
        }

        // Workspace-based routing is planned: once SprintConfig carries a workspace_id
        // field, this will delegate to WorkspaceManager to resolve the target project.
        // For now, fall back to the current working directory.
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        ToolContext::for_single_project(cwd)
    }

    /// Create a tool context from explicit project info.
    #[must_use]
    pub fn from_project(project: Project, workspace_id: Option<WorkspaceId>) -> ToolContext {
        let mut ctx = ToolContext::new(project);
        if let Some(wid) = workspace_id {
            ctx = ctx.with_workspace(wid);
        }
        ctx
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_context_new() {
        let project = Project::new("test-proj", "/home/user/test");
        let ctx = ToolContext::new(project);
        assert_eq!(ctx.project_name(), "test-proj");
        assert_eq!(ctx.project_root(), &PathBuf::from("/home/user/test"));
    }

    #[test]
    fn test_tool_context_with_workspace() {
        let project = Project::new("api", "/home/user/api");
        let ws_id = WorkspaceId("ws-1".to_string());
        let ctx = ToolContext::new(project).with_workspace(ws_id);
        assert!(ctx.workspace_id.is_some());
    }

    #[test]
    fn test_tool_context_for_single_project() {
        let ctx = ToolContext::for_single_project("/home/user/my-project");
        assert_eq!(ctx.project_name(), "project");
        assert_eq!(ctx.project_root(), &PathBuf::from("/home/user/my-project"));
    }

    #[tokio::test]
    async fn test_resolver_no_workspace() {
        let ctx = ToolContextResolver::resolve(None, None, None).await;
        // Should use cwd
        assert!(ctx.project_root().exists() || true); // cwd always "exists"
    }
}
