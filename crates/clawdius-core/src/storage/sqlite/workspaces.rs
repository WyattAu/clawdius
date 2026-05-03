use super::SqliteBackend;
use crate::error::Result;
use crate::storage::backend::WorkspaceRepository;
use crate::storage::error::StorageError;
use crate::workspace::{Project, ProjectId, Workspace, WorkspaceId};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

impl WorkspaceRepository for SqliteBackend {
    fn create_workspace(&self, workspace: &Workspace) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO workspaces (id, name, default_project_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![workspace.id.0, workspace.name, workspace.default_project_id.as_ref().map(|p| &p.0), workspace.created_at.to_rfc3339()],
                ).map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspaces".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn load_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<Option<Workspace>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, default_project_id, created_at FROM workspaces WHERE id = ?1",
                )?;
                let row = stmt
                    .query_row(params![id.0], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT workspace".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(row.map(|(id, name, default_project_id, created_at)| Workspace {
                    id: WorkspaceId(id),
                    name,
                    default_project_id: default_project_id.map(ProjectId),
                    created_at: created_at
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                }))
            })
        }
    }

    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<Vec<Workspace>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, default_project_id, created_at FROM workspaces ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map(params![], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                let workspaces: Vec<Workspace> = rows
                    .map(|r| {
                        r.map(|(id, name, default_project_id, created_at)| Workspace {
                            id: WorkspaceId(id),
                            name,
                            default_project_id: default_project_id.map(ProjectId),
                            created_at: created_at
                                .parse::<DateTime<Utc>>()
                                .unwrap_or_else(|_| Utc::now()),
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::RowConversion { reason: e.to_string() })?;
                Ok(workspaces)
            })
        }
    }

    fn delete_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute("DELETE FROM workspace_projects WHERE workspace_id = ?1", params![id.0])?;
                conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id.0])?;
                Ok(())
            })
        }
    }

    fn add_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![project.id.0, project.name, project.root_path.to_string_lossy().as_ref(), project.created_at.to_rfc3339()],
                ).map_err(|e| StorageError::Query {
                    statement: "INSERT INTO projects".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn load_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = ?1",
                )?;
                let row = stmt
                    .query_row(params![id.0], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT project".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(row.map(|(id, name, root_path, created_at)| Project {
                    id: ProjectId(id),
                    name,
                    root_path: PathBuf::from(root_path),
                    created_at: created_at
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                }))
            })
        }
    }

    fn load_project_by_path(&self, path: &Path) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, root_path, created_at FROM projects WHERE root_path = ?1",
                )?;
                let path_str = path.to_string_lossy().to_string();
                let row = stmt
                    .query_row(params![path_str], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT project by path".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(row.map(|(id, name, root_path, created_at)| Project {
                    id: ProjectId(id),
                    name,
                    root_path: PathBuf::from(root_path),
                    created_at: created_at
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                }))
            })
        }
    }

    fn list_projects(&self) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, name, root_path, created_at FROM projects ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map(params![], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                let projects: Vec<Project> = rows
                    .map(|r| {
                        r.map(|(id, name, root_path, created_at)| Project {
                            id: ProjectId(id),
                            name,
                            root_path: PathBuf::from(root_path),
                            created_at: created_at
                                .parse::<DateTime<Utc>>()
                                .unwrap_or_else(|_| Utc::now()),
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::RowConversion { reason: e.to_string() })?;
                Ok(projects)
            })
        }
    }

    fn update_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "UPDATE projects SET name = ?2, root_path = ?3 WHERE id = ?1",
                    params![project.id.0, project.name, project.root_path.to_string_lossy().as_ref()],
                ).map_err(|e| StorageError::Query {
                    statement: "UPDATE projects".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn remove_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute("DELETE FROM workspace_projects WHERE project_id = ?1", params![id.0])?;
                conn.execute("DELETE FROM projects WHERE id = ?1", params![id.0])?;
                Ok(())
            })
        }
    }

    fn add_project_to_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO workspace_projects (workspace_id, project_id, added_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
                    params![workspace_id.0, project_id.0],
                ).map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM workspace_projects WHERE workspace_id = ?1 AND project_id = ?2",
                    params![workspace_id.0, project_id.0],
                ).map_err(|e| StorageError::Query {
                    statement: "DELETE FROM workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn list_workspace_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT p.id, p.name, p.root_path, p.created_at
                     FROM projects p
                     INNER JOIN workspace_projects wp ON wp.project_id = p.id
                     WHERE wp.workspace_id = ?1
                     ORDER BY wp.added_at DESC",
                )?;
                let rows = stmt.query_map(params![workspace_id.0], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                let projects: Vec<Project> = rows
                    .map(|r| {
                        r.map(|(id, name, root_path, created_at)| Project {
                            id: ProjectId(id),
                            name,
                            root_path: PathBuf::from(root_path),
                            created_at: created_at
                                .parse::<DateTime<Utc>>()
                                .unwrap_or_else(|_| Utc::now()),
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::RowConversion { reason: e.to_string() })?;
                Ok(projects)
            })
        }
    }

    fn set_default_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "UPDATE workspaces SET default_project_id = ?2 WHERE id = ?1",
                    params![workspace_id.0, project_id.0],
                ).map_err(|e| StorageError::Query {
                    statement: "UPDATE workspaces default_project_id".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn get_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT default_project_id FROM workspaces WHERE id = ?1",
                )?;
                let default_id: Option<String> = stmt
                    .query_row(params![workspace_id.0], |row| row.get::<_, Option<String>>(0))
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT default_project_id".to_string(),
                        reason: e.to_string(),
                    })?
                    .flatten();

                let Some(default_id) = default_id else {
                    return Ok(None);
                };

                let mut stmt = conn.prepare(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = ?1",
                )?;
                let row = stmt
                    .query_row(params![default_id], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT project by id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(row.map(|(id, name, root_path, created_at)| Project {
                    id: ProjectId(id),
                    name,
                    root_path: PathBuf::from(root_path),
                    created_at: created_at
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                }))
            })
        }
    }
}
