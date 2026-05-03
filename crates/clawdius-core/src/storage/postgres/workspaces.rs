use super::PostgresBackend;
use crate::error::Result;
use crate::storage::backend::WorkspaceRepository;
use crate::storage::error::StorageError;
use crate::workspace::{Project, ProjectId, Workspace, WorkspaceId};
use std::path::{Path, PathBuf};
use tokio_postgres::types::ToSql;

impl WorkspaceRepository for PostgresBackend {
    fn create_workspace(&self, workspace: &Workspace) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO workspaces (id, name, default_project_id, created_at) VALUES ($1, $2, $3, $4)",
                    &[
                        &workspace.id.0 as &(dyn ToSql + Sync),
                        &workspace.name,
                        &workspace.default_project_id.as_ref().map(|pid| pid.0.clone()),
                        &workspace.created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn load_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<Option<Workspace>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id, name, default_project_id, created_at FROM workspaces WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Workspace {
                id: WorkspaceId(r.get(0)),
                name: r.get(1),
                default_project_id: r.get::<_, Option<String>>(2).map(ProjectId),
                created_at: r.get(3),
            }))
        }
    }

    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<Vec<Workspace>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    "SELECT id, name, default_project_id, created_at FROM workspaces ORDER BY created_at DESC",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces list".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Workspace {
                    id: WorkspaceId(r.get(0)),
                    name: r.get(1),
                    default_project_id: r.get::<_, Option<String>>(2).map(ProjectId),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn delete_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute("DELETE FROM workspaces WHERE id = $1", &[&id.0 as &(dyn ToSql + Sync)])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn add_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO projects (id, name, root_path, created_at) VALUES ($1, $2, $3, $4)",
                    &[
                        &project.id.0 as &(dyn ToSql + Sync),
                        &project.name,
                        &project.root_path.to_string_lossy().as_ref(),
                        &project.created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn load_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }

    fn load_project_by_path(&self, path: &Path) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE root_path = $1",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects by path".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }

    fn list_projects(&self) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    "SELECT id, name, root_path, created_at FROM projects ORDER BY created_at DESC",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects list".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Project {
                    id: ProjectId(r.get(0)),
                    name: r.get(1),
                    root_path: PathBuf::from(r.get::<_, String>(2)),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn update_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "UPDATE projects SET name = $2, root_path = $3 WHERE id = $1",
                    &[
                        &project.id.0 as &(dyn ToSql + Sync),
                        &project.name,
                        &project.root_path.to_string_lossy().as_ref(),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn remove_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute("DELETE FROM projects WHERE id = $1", &[&id.0 as &(dyn ToSql + Sync)])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn add_project_to_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO workspace_projects (workspace_id, project_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM workspace_projects WHERE workspace_id = $1 AND project_id = $2",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn list_workspace_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"SELECT p.id, p.name, p.root_path, p.created_at
                       FROM projects p
                       INNER JOIN workspace_projects wp ON wp.project_id = p.id
                       WHERE wp.workspace_id = $1
                       ORDER BY wp.added_at DESC",
                    &[&workspace_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspace projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Project {
                    id: ProjectId(r.get(0)),
                    name: r.get(1),
                    root_path: PathBuf::from(r.get::<_, String>(2)),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn set_default_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "UPDATE workspaces SET default_project_id = $2 WHERE id = $1",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE workspaces default_project_id".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn get_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let default_id: Option<String> = client
                .query_opt(
                    "SELECT default_project_id FROM workspaces WHERE id = $1",
                    &[&workspace_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces default_project_id".to_string(),
                    reason: e.to_string(),
                })?
                .and_then(|r| r.get(0));

            let Some(default_id) = default_id else {
                return Ok(None);
            };

            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = $1",
                    &[&default_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects by id".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }
}
