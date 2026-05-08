//! Workspace manager — service layer for workspace/project operations.
//!
//! [`WorkspaceManager`] wraps [`WorkspaceRepository`] with business logic:
//! - Create/delete workspaces with automatic project setup
//! - Add/remove projects with duplicate detection (by path)
//! - Resolve project by path (cwd → project mapping)
//! - Auto-detect project metadata from filesystem
//! - Default project management
//!
//! # Usage
//!
//! ```rust,ignore
//! use clawdius_core::workspace::WorkspaceManager;
//! use clawdius_core::storage::SqliteBackend;
//!
//! let backend = SqliteBackend::new(":memory:")?;
//! let manager = WorkspaceManager::new(backend);
//!
//! // Create workspace and add projects
//! let ws = manager.create_workspace("my-workspace").await?;
//! let proj = manager.add_project_by_path(&ws.id, "/home/user/api-server").await?;
//!
//! // Resolve cwd to project
//! let proj = manager.resolve_project_by_path(&ws.id, std::env::current_dir()?).await?;
//! ```

use super::{Project, ProjectId, Workspace, WorkspaceId};
use crate::error::Error;
use crate::error::Result;
use crate::storage::WorkspaceRepository;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────
// WorkspaceManager
// ─────────────────────────────────────────────────────────

/// Service layer for workspace and project management.
///
/// Provides business logic on top of the raw [`WorkspaceRepository`]:
/// duplicate detection, path canonicalization, auto-naming, etc.
pub struct WorkspaceManager<R: WorkspaceRepository> {
    repo: R,
}

impl<R: WorkspaceRepository> WorkspaceManager<R> {
    /// Create a new workspace manager backed by the given repository.
    #[must_use]
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Get a reference to the underlying repository.
    #[must_use]
    pub fn repo(&self) -> &R {
        &self.repo
    }

    // ── Workspace operations ──

    /// Create a new empty workspace with the given name.
    pub async fn create_workspace(&self, name: impl Into<String>) -> Result<Workspace> {
        let ws = Workspace::new(name);
        self.repo.create_workspace(&ws).await?;
        Ok(ws)
    }

    /// Load a workspace by ID.
    pub async fn load_workspace(&self, id: &WorkspaceId) -> Result<Workspace> {
        self.repo
            .load_workspace(id)
            .await?
            .ok_or_else(|| Error::NotFound(format!("workspace {id}")))
    }

    /// List all workspaces.
    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.repo.list_workspaces().await
    }

    /// Delete a workspace and all its project associations.
    ///
    /// NOTE: This does NOT delete the underlying projects — they may
    /// belong to other workspaces. Use `remove_project` explicitly.
    pub async fn delete_workspace(&self, id: &WorkspaceId) -> Result<()> {
        self.repo.delete_workspace(id).await
    }

    /// Get or create the default workspace.
    ///
    /// If no workspace exists, creates one named "default".
    /// If exactly one workspace exists, returns it.
    /// If multiple workspaces exist, returns the first one.
    pub async fn get_or_create_default_workspace(&self) -> Result<Workspace> {
        let workspaces = self.repo.list_workspaces().await?;
        if workspaces.is_empty() {
            self.create_workspace("default").await
        } else {
            Ok(workspaces
                .into_iter()
                .next()
                .expect("workspaces is non-empty"))
        }
    }

    // ── Project operations ──

    /// Add a project to a workspace by path.
    ///
    /// Auto-detects the project name from the directory name (or Cargo.toml/package.json).
    /// Returns an error if a project with the same path already exists in ANY workspace.
    pub async fn add_project_by_path(
        &self,
        workspace_id: &WorkspaceId,
        root_path: impl AsRef<Path>,
    ) -> Result<Project> {
        let root_path = root_path.as_ref();

        // Canonicalize the path for dedup
        let canonical = root_path.canonicalize().map_err(|e| {
            Error::InvalidInput(format!(
                "cannot resolve path '{}': {e}",
                root_path.display()
            ))
        })?;

        // Check for existing project with same path
        if let Some(existing) = self.repo.load_project_by_path(&canonical).await? {
            // Project already exists somewhere — just add to this workspace if not already
            self.repo
                .add_project_to_workspace(workspace_id, &existing.id)
                .await?;
            return Ok(existing);
        }

        // Detect project name
        let name = detect_project_name(&canonical);

        let project = Project::new(name, canonical);
        self.repo.add_project(&project).await?;
        self.repo
            .add_project_to_workspace(workspace_id, &project.id)
            .await?;

        // If this is the first project in the workspace, make it default
        let projects = self.repo.list_workspace_projects(workspace_id).await?;
        if projects.len() == 1 {
            self.repo
                .set_default_project(workspace_id, &project.id)
                .await?;
        }

        Ok(project)
    }

    /// Remove a project from a workspace.
    ///
    /// Removes the workspace-project association. Does NOT delete the
    /// project from other workspaces or from the database.
    pub async fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> Result<()> {
        self.repo
            .remove_project_from_workspace(workspace_id, project_id)
            .await
    }

    /// Permanently delete a project from all workspaces.
    pub async fn delete_project(&self, project_id: &ProjectId) -> Result<()> {
        self.repo.remove_project(project_id).await
    }

    /// List all projects in a workspace.
    pub async fn list_projects(&self, workspace_id: &WorkspaceId) -> Result<Vec<Project>> {
        self.repo.list_workspace_projects(workspace_id).await
    }

    /// List all projects across all workspaces.
    pub async fn list_all_projects(&self) -> Result<Vec<Project>> {
        self.repo.list_projects().await
    }

    /// Load a project by ID.
    pub async fn load_project(&self, id: &ProjectId) -> Result<Project> {
        self.repo
            .load_project(id)
            .await?
            .ok_or_else(|| Error::NotFound(format!("project {id}")))
    }

    // ── Default project ──

    /// Set the default project for a workspace.
    pub async fn set_default_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> Result<()> {
        // Verify project belongs to workspace
        let projects = self.repo.list_workspace_projects(workspace_id).await?;
        if !projects.iter().any(|p| p.id == *project_id) {
            return Err(Error::InvalidInput(format!(
                "project {project_id} is not in workspace {workspace_id}"
            )));
        }
        self.repo
            .set_default_project(workspace_id, project_id)
            .await
    }

    /// Get the default project for a workspace.
    pub async fn get_default_project(&self, workspace_id: &WorkspaceId) -> Result<Option<Project>> {
        self.repo.get_default_project(workspace_id).await
    }

    /// Get the default project, or the first project if none is set.
    pub async fn get_effective_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Project> {
        if let Some(proj) = self.repo.get_default_project(workspace_id).await? {
            return Ok(proj);
        }
        let projects = self.repo.list_workspace_projects(workspace_id).await?;
        projects
            .into_iter()
            .next()
            .ok_or_else(|| Error::NotFound(format!("workspace {workspace_id} has no projects")))
    }

    // ── Path resolution ──

    /// Resolve a filesystem path to a project within a workspace.
    ///
    /// Finds the project whose `root_path` is an ancestor of (or equal to)
    /// the given path. This enables tool calls to automatically target the
    /// correct project based on the file being operated on.
    pub async fn resolve_project_for_path(
        &self,
        workspace_id: &WorkspaceId,
        file_path: impl AsRef<Path>,
    ) -> Result<Option<Project>> {
        let file_path = file_path.as_ref();
        let canonical = match file_path.canonicalize() {
            Ok(p) => p,
            Err(_) => return Ok(None), // Path doesn't exist yet
        };

        let projects = self.repo.list_workspace_projects(workspace_id).await?;
        let mut best_match: Option<(usize, Project)> = None;

        for proj in projects {
            let proj_root = match proj.root_path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };

            if canonical.starts_with(&proj_root) {
                let match_len = proj_root.components().count();
                if best_match
                    .as_ref()
                    .map_or(true, |(len, _)| match_len > *len)
                {
                    best_match = Some((match_len, proj));
                }
            }
        }

        Ok(best_match.map(|(_, proj)| proj))
    }

    /// Resolve cwd to a project within a workspace.
    ///
    /// Convenience wrapper around [`Self::resolve_project_for_path`].
    pub async fn resolve_project_for_cwd(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<Project>> {
        let cwd = std::env::current_dir()?;
        self.resolve_project_for_path(workspace_id, cwd).await
    }

    /// Resolve a path to a project, falling back to the default project.
    pub async fn resolve_project_or_default(
        &self,
        workspace_id: &WorkspaceId,
        file_path: impl AsRef<Path>,
    ) -> Result<Project> {
        if let Some(proj) = self
            .resolve_project_for_path(workspace_id, &file_path)
            .await?
        {
            return Ok(proj);
        }
        self.get_effective_default_project(workspace_id).await
    }

    // ── Workspace summary ──

    /// Get a summary of a workspace with its projects.
    pub async fn workspace_summary(&self, workspace_id: &WorkspaceId) -> Result<WorkspaceSummary> {
        let workspace = self.load_workspace(workspace_id).await?;
        let projects = self.list_projects(workspace_id).await?;
        let default_project = self.get_default_project(workspace_id).await?;

        Ok(WorkspaceSummary {
            workspace,
            projects,
            default_project,
        })
    }
}

// ─────────────────────────────────────────────────────────
// WorkspaceSummary
// ─────────────────────────────────────────────────────────

/// A workspace together with its projects and default project.
#[derive(Debug, Clone)]
pub struct WorkspaceSummary {
    /// The workspace.
    pub workspace: Workspace,
    /// All projects in the workspace.
    pub projects: Vec<Project>,
    /// The default project (if set).
    pub default_project: Option<Project>,
}

// ─────────────────────────────────────────────────────────
// Project detection helpers
// ─────────────────────────────────────────────────────────

/// Detect a human-readable project name from the filesystem.
///
/// Priority:
/// 1. `Cargo.toml` → `package.name`
/// 2. `package.json` → `name`
/// 3. `pyproject.toml` → `project.name`
/// 4. `go.mod` → module path (last segment)
/// 5. Directory name (fallback)
fn detect_project_name(root: &Path) -> String {
    // Try Cargo.toml
    if let Some(name) = read_toml_field(root.join("Cargo.toml"), "package", "name") {
        return name;
    }

    // Try package.json
    if let Some(name) = read_json_field(root.join("package.json"), "name") {
        return name;
    }

    // Try pyproject.toml
    if let Some(name) = read_toml_field(root.join("pyproject.toml"), "project", "name") {
        return name;
    }

    // Try go.mod
    if let Some(name) = read_go_module_name(root.join("go.mod")) {
        return name;
    }

    // Fallback: directory name
    root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Read a nested TOML field value (e.g., `package.name`).
fn read_toml_field(path: PathBuf, section: &str, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(&path).ok()?;
    let table: toml::Table = content.parse().ok()?;

    table
        .get(section)?
        .get(key)?
        .as_str()
        .map(|s| s.to_string())
}

/// Read a JSON field value.
fn read_json_field(path: PathBuf, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(&path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;

    value.get(key)?.as_str().map(|s| s.to_string())
}

/// Read the module name from go.mod and return the last segment.
fn read_go_module_name(path: PathBuf) -> Option<String> {
    let content = std::fs::read_to_string(&path).ok()?;

    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            let rest = rest.trim();
            // Take the last segment: "github.com/user/project" → "project"
            if let Some(name) = rest.split('/').next_back() {
                return Some(name.to_string());
            }
            return Some(rest.to_string());
        }
    }
    None
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryBackend;

    fn setup() -> WorkspaceManager<InMemoryBackend> {
        let backend = InMemoryBackend::new();
        WorkspaceManager::new(backend)
    }

    #[tokio::test]
    async fn test_create_workspace() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-workspace").await.unwrap();
        assert_eq!(ws.name, "test-workspace");
        assert!(ws.id.0.len() > 0);
    }

    #[tokio::test]
    async fn test_load_workspace() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-workspace").await.unwrap();
        let loaded = mgr.load_workspace(&ws.id).await.unwrap();
        assert_eq!(loaded.id, ws.id);
        assert_eq!(loaded.name, ws.name);
    }

    #[tokio::test]
    async fn test_load_nonexistent_workspace() {
        let mgr = setup();
        let id = WorkspaceId("nonexistent".to_string());
        let result = mgr.load_workspace(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_workspaces() {
        let mgr = setup();
        mgr.create_workspace("ws-1").await.unwrap();
        mgr.create_workspace("ws-2").await.unwrap();
        let list = mgr.list_workspaces().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let mgr = setup();
        let ws = mgr.create_workspace("to-delete").await.unwrap();
        mgr.delete_workspace(&ws.id).await.unwrap();
        let list = mgr.list_workspaces().await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_get_or_create_default_workspace() {
        let mgr = setup();

        // No workspace → creates default
        let ws = mgr.get_or_create_default_workspace().await.unwrap();
        assert_eq!(ws.name, "default");

        // Existing workspace → returns it
        let ws2 = mgr.get_or_create_default_workspace().await.unwrap();
        assert_eq!(ws.id, ws2.id);
    }

    #[tokio::test]
    async fn test_add_project_by_temp_dir() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        // Create temp dirs to simulate projects
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-project");
        std::fs::create_dir_all(&project_path).unwrap();

        let proj = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        assert_eq!(proj.name, "my-project"); // from directory name
        assert!(proj.root_path.ends_with("my-project"));

        // Should be auto-set as default (first project)
        let default = mgr.get_default_project(&ws.id).await.unwrap();
        assert_eq!(default.unwrap().id, proj.id);
    }

    #[tokio::test]
    async fn test_add_project_auto_detects_cargo_name() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-crate");
        std::fs::create_dir_all(&project_path).unwrap();

        // Write Cargo.toml with package name
        std::fs::write(
            project_path.join("Cargo.toml"),
            r#"[package]
name = "my-awesome-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        let proj = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        assert_eq!(proj.name, "my-awesome-crate");
    }

    #[tokio::test]
    async fn test_add_project_auto_detects_package_json() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-app");
        std::fs::create_dir_all(&project_path).unwrap();

        std::fs::write(
            project_path.join("package.json"),
            r#"{"name": "@scope/my-app", "version": "1.0.0"}"#,
        )
        .unwrap();

        let proj = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        assert_eq!(proj.name, "@scope/my-app");
    }

    #[tokio::test]
    async fn test_add_project_dedup_by_path() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-project");
        std::fs::create_dir_all(&project_path).unwrap();

        let proj1 = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();
        let proj2 = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        // Same project returned
        assert_eq!(proj1.id, proj2.id);

        // Only one project in workspace
        let projects = mgr.list_projects(&ws.id).await.unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[tokio::test]
    async fn test_remove_project_from_workspace() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-project");
        std::fs::create_dir_all(&project_path).unwrap();

        let proj = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        mgr.remove_project_from_workspace(&ws.id, &proj.id)
            .await
            .unwrap();

        let projects = mgr.list_projects(&ws.id).await.unwrap();
        assert_eq!(projects.len(), 0);
    }

    #[tokio::test]
    async fn test_set_default_project() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();

        let p1 = dir.path().join("project-1");
        let p2 = dir.path().join("project-2");
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();

        let proj1 = mgr.add_project_by_path(&ws.id, &p1).await.unwrap();
        let proj2 = mgr.add_project_by_path(&ws.id, &p2).await.unwrap();

        // First project is auto-default
        let default = mgr.get_default_project(&ws.id).await.unwrap();
        assert_eq!(default.unwrap().id, proj1.id);

        // Switch default
        mgr.set_default_project(&ws.id, &proj2.id).await.unwrap();
        let default = mgr.get_default_project(&ws.id).await.unwrap();
        assert_eq!(default.unwrap().id, proj2.id);
    }

    #[tokio::test]
    async fn test_set_default_project_not_in_workspace() {
        let mgr = setup();
        let ws1 = mgr.create_workspace("ws-1").await.unwrap();
        let ws2 = mgr.create_workspace("ws-2").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("project-1");
        let p2 = dir.path().join("project-2");
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();

        let proj1 = mgr.add_project_by_path(&ws1.id, &p1).await.unwrap();
        let _proj2 = mgr.add_project_by_path(&ws2.id, &p2).await.unwrap();

        // proj1 is in ws1, not ws2
        let result = mgr.set_default_project(&ws2.id, &proj1.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_effective_default_project() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("project-1");
        std::fs::create_dir_all(&p1).unwrap();

        let proj = mgr.add_project_by_path(&ws.id, &p1).await.unwrap();

        // Default is set → returns it
        let effective = mgr.get_effective_default_project(&ws.id).await.unwrap();
        assert_eq!(effective.id, proj.id);
    }

    #[tokio::test]
    async fn test_resolve_project_for_path() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("my-project");
        let sub_dir = project_path.join("src");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let proj = mgr
            .add_project_by_path(&ws.id, &project_path)
            .await
            .unwrap();

        // Resolve a file inside the project
        let file_path = sub_dir.join("main.rs");
        std::fs::write(&file_path, "// test").unwrap();

        let resolved = mgr
            .resolve_project_for_path(&ws.id, &file_path)
            .await
            .unwrap();

        assert_eq!(resolved.unwrap().id, proj.id);
    }

    #[tokio::test]
    async fn test_resolve_project_nested_projects() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();

        // Parent project
        let parent = dir.path().join("monorepo");
        let parent_src = parent.join("src");
        std::fs::create_dir_all(&parent_src).unwrap();

        // Child project (nested)
        let child = parent.join("packages/child-lib");
        let child_src = child.join("src");
        std::fs::create_dir_all(&child_src).unwrap();

        let parent_proj = mgr.add_project_by_path(&ws.id, &parent).await.unwrap();
        let child_proj = mgr.add_project_by_path(&ws.id, &child).await.unwrap();

        // A file in child/src should resolve to child (more specific)
        let file_path = child_src.join("lib.rs");
        std::fs::write(&file_path, "// child").unwrap();

        let resolved = mgr
            .resolve_project_for_path(&ws.id, &file_path)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(resolved.id, child_proj.id);

        // A file in parent/src (not in child) should resolve to parent
        let file_path2 = parent_src.join("main.rs");
        std::fs::write(&file_path2, "// parent").unwrap();

        let resolved2 = mgr
            .resolve_project_for_path(&ws.id, &file_path2)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(resolved2.id, parent_proj.id);
    }

    #[tokio::test]
    async fn test_workspace_summary() {
        let mgr = setup();
        let ws = mgr.create_workspace("test-ws").await.unwrap();

        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("proj-1");
        let p2 = dir.path().join("proj-2");
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();

        mgr.add_project_by_path(&ws.id, &p1).await.unwrap();
        mgr.add_project_by_path(&ws.id, &p2).await.unwrap();

        let summary = mgr.workspace_summary(&ws.id).await.unwrap();
        assert_eq!(summary.workspace.id, ws.id);
        assert_eq!(summary.projects.len(), 2);
        assert!(summary.default_project.is_some()); // First project auto-set
    }

    #[test]
    fn test_detect_project_name_cargo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        std::fs::write(
            path.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        assert_eq!(detect_project_name(path), "test-crate");
    }

    #[test]
    fn test_detect_project_name_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        std::fs::write(path.join("package.json"), r#"{"name": "test-app"}"#).unwrap();

        assert_eq!(detect_project_name(path), "test-app");
    }

    #[test]
    fn test_detect_project_name_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        assert_eq!(
            detect_project_name(path),
            path.file_name().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn test_detect_project_name_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        std::fs::write(path.join("go.mod"), "module github.com/user/my-go-lib\n").unwrap();

        assert_eq!(detect_project_name(path), "my-go-lib");
    }

    #[test]
    fn test_detect_project_name_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        std::fs::write(
            path.join("pyproject.toml"),
            r#"[project]
name = "my-python-lib"
version = "0.1.0"
"#,
        )
        .unwrap();

        assert_eq!(detect_project_name(path), "my-python-lib");
    }
}
