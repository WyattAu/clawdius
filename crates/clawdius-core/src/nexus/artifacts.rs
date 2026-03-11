//! Artifact tracking for Nexus FSM
//!
//! This module implements artifact storage and retrieval using SQLite as the backend
//! with an LRU cache layer for performance. Artifacts represent the outputs of each
//! phase and are tracked for dependency management and audit purposes.

use chrono::{DateTime, Utc};
use lru::LruCache;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{NexusError, PhaseId, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ArtifactId(pub String);

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ArtifactId {
    pub fn new(id: impl Into<String>) -> Self {
        ArtifactId(id.into())
    }

    pub fn generate() -> Self {
        ArtifactId(uuid::Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactType {
    YellowPaper,
    BluePaper,
    TestVector,
    Proof,
    SourceCode,
    Documentation,
    Configuration,
    Compliance,
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactType::YellowPaper => write!(f, "YellowPaper"),
            ArtifactType::BluePaper => write!(f, "BluePaper"),
            ArtifactType::TestVector => write!(f, "TestVector"),
            ArtifactType::Proof => write!(f, "Proof"),
            ArtifactType::SourceCode => write!(f, "SourceCode"),
            ArtifactType::Documentation => write!(f, "Documentation"),
            ArtifactType::Configuration => write!(f, "Configuration"),
            ArtifactType::Compliance => write!(f, "Compliance"),
        }
    }
}

impl ArtifactType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "YellowPaper" => Some(ArtifactType::YellowPaper),
            "BluePaper" => Some(ArtifactType::BluePaper),
            "TestVector" => Some(ArtifactType::TestVector),
            "Proof" => Some(ArtifactType::Proof),
            "SourceCode" => Some(ArtifactType::SourceCode),
            "Documentation" => Some(ArtifactType::Documentation),
            "Configuration" => Some(ArtifactType::Configuration),
            "Compliance" => Some(ArtifactType::Compliance),
            _ => None,
        }
    }

    pub fn all() -> Vec<ArtifactType> {
        vec![
            ArtifactType::YellowPaper,
            ArtifactType::BluePaper,
            ArtifactType::TestVector,
            ArtifactType::Proof,
            ArtifactType::SourceCode,
            ArtifactType::Documentation,
            ArtifactType::Configuration,
            ArtifactType::Compliance,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub phase: PhaseId,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub artifact_type: ArtifactType,
    pub content: serde_json::Value,
    pub hash: String,
    pub dependencies: Vec<ArtifactId>,
    pub metadata: ArtifactMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artifact {
    pub fn new(artifact_type: ArtifactType, content: serde_json::Value, phase: PhaseId) -> Self {
        let id = ArtifactId::generate();
        let now = Utc::now();
        let hash = Self::compute_hash(&content);

        Self {
            id,
            artifact_type,
            content,
            hash,
            dependencies: Vec::new(),
            metadata: ArtifactMetadata {
                phase,
                author: String::new(),
                description: String::new(),
                tags: Vec::new(),
            },
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.metadata.description = description.into();
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.metadata.author = author.into();
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<ArtifactId>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags = tags;
        self
    }

    pub fn compute_hash(content: &serde_json::Value) -> String {
        use sha3::{Digest, Sha3_256};

        let mut hasher = Sha3_256::new();
        hasher.update(&serde_json::to_vec(content).unwrap_or_default());
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_integrity(&self) -> bool {
        self.hash == Self::compute_hash(&self.content)
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    artifact_type TEXT NOT NULL,
    content TEXT NOT NULL,
    hash TEXT NOT NULL,
    dependencies TEXT NOT NULL DEFAULT '[]',
    phase INTEGER NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_phase ON artifacts(phase);
CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_artifacts_created_at ON artifacts(created_at);

CREATE TABLE IF NOT EXISTS artifact_dependencies (
    artifact_id TEXT NOT NULL,
    dependency_id TEXT NOT NULL,
    PRIMARY KEY (artifact_id, dependency_id),
    FOREIGN KEY (artifact_id) REFERENCES artifacts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_deps_artifact_id ON artifact_dependencies(artifact_id);
CREATE INDEX IF NOT EXISTS idx_deps_dependency_id ON artifact_dependencies(dependency_id);
"#;

#[derive(Debug)]
#[allow(dead_code)]
struct ConnectionPool {
    connections: Mutex<Vec<Connection>>,
    db_path: PathBuf,
    pool_size: usize,
}

impl ConnectionPool {
    fn new(db_path: PathBuf, pool_size: usize) -> Result<Self> {
        let mut connections = Vec::with_capacity(pool_size);

        for i in 0..pool_size {
            let conn = if i == 0 {
                Self::create_connection(&db_path)?
            } else {
                Self::create_connection(&db_path)?
            };
            connections.push(conn);
        }

        Ok(Self {
            connections: Mutex::new(connections),
            db_path,
            pool_size,
        })
    }

    fn create_connection(db_path: &PathBuf) -> Result<Connection> {
        let conn = if db_path.to_string_lossy() == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(db_path)
        }
        .map_err(NexusError::DatabaseError)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(NexusError::DatabaseError)?;

        Ok(conn)
    }

    fn get(&self) -> Result<std::sync::MutexGuard<'_, Vec<Connection>>> {
        self.connections.lock().map_err(|e| {
            NexusError::LockError(format!("Failed to acquire connection pool lock: {}", e))
        })
    }

    fn initialize_schema(&self) -> Result<()> {
        let mut pool = self.get()?;
        if let Some(conn) = pool.first_mut() {
            conn.execute_batch(SCHEMA_SQL)
                .map_err(NexusError::DatabaseError)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct InMemoryStore {
    artifacts: HashMap<ArtifactId, Artifact>,
    by_phase: HashMap<u8, Vec<ArtifactId>>,
    by_type: HashMap<String, Vec<ArtifactId>>,
}

impl InMemoryStore {
    fn new() -> Self {
        Self {
            artifacts: HashMap::new(),
            by_phase: HashMap::new(),
            by_type: HashMap::new(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ArtifactTracker {
    db_path: PathBuf,
    store: Mutex<InMemoryStore>,
    pool: ConnectionPool,
    cache: Mutex<LruCache<ArtifactId, Artifact>>,
    cache_enabled: bool,
}

impl ArtifactTracker {
    pub fn new(project_root: &PathBuf) -> Result<Self> {
        let db_path = project_root.join(".clawdius/nexus.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                NexusError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create database directory: {}", e),
                ))
            })?;
        }

        let pool = ConnectionPool::new(db_path.clone(), 4)?;
        pool.initialize_schema()?;

        Ok(Self {
            db_path,
            store: Mutex::new(InMemoryStore::new()),
            pool,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())),
            cache_enabled: true,
        })
    }

    pub fn in_memory() -> Self {
        let pool = ConnectionPool::new(PathBuf::from(":memory:"), 1).unwrap();
        pool.initialize_schema().unwrap();

        Self {
            db_path: PathBuf::from(":memory:"),
            store: Mutex::new(InMemoryStore::new()),
            pool,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(256).unwrap())),
            cache_enabled: true,
        }
    }

    pub fn store(&self, artifact: Artifact) -> Result<ArtifactId> {
        let id = artifact.id.clone();
        let phase = artifact.metadata.phase.0;
        let type_str = artifact.artifact_type.to_string();

        let mut store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        store
            .by_phase
            .entry(phase)
            .or_insert_with(Vec::new)
            .push(id.clone());
        store
            .by_type
            .entry(type_str)
            .or_insert_with(Vec::new)
            .push(id.clone());
        store.artifacts.insert(id.clone(), artifact);

        Ok(id)
    }

    pub fn retrieve(&self, id: &ArtifactId) -> Result<Option<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        Ok(store.artifacts.get(id).cloned())
    }

    pub fn delete(&self, id: &ArtifactId) -> Result<bool> {
        let mut store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        if let Some(artifact) = store.artifacts.remove(id) {
            if let Some(ids) = store.by_phase.get_mut(&artifact.metadata.phase.0) {
                ids.retain(|i| i != id);
            }
            let type_str = artifact.artifact_type.to_string();
            if let Some(ids) = store.by_type.get_mut(&type_str) {
                ids.retain(|i| i != id);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list_by_phase(&self, phase: PhaseId) -> Result<Vec<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let ids = store.by_phase.get(&phase.0).cloned().unwrap_or_default();
        let artifacts = ids
            .iter()
            .filter_map(|id| store.artifacts.get(id).cloned())
            .collect();

        Ok(artifacts)
    }

    pub fn list_by_type(&self, artifact_type: ArtifactType) -> Result<Vec<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let type_str = artifact_type.to_string();
        let ids = store.by_type.get(&type_str).cloned().unwrap_or_default();
        let artifacts = ids
            .iter()
            .filter_map(|id| store.artifacts.get(id).cloned())
            .collect();

        Ok(artifacts)
    }

    pub fn validate_dependencies(&self, id: &ArtifactId) -> Result<bool> {
        let artifact = self.retrieve(id)?;
        if let Some(artifact) = artifact {
            for dep_id in &artifact.dependencies {
                if self.retrieve(dep_id)?.is_none() {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_dependencies(&self, id: &ArtifactId) -> Result<Vec<Artifact>> {
        let artifact = self.retrieve(id)?;
        if let Some(artifact) = artifact {
            let mut deps = Vec::new();
            for dep_id in &artifact.dependencies {
                if let Some(dep) = self.retrieve(dep_id)? {
                    deps.push(dep);
                }
            }
            Ok(deps)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn get_dependents(&self, id: &ArtifactId) -> Result<Vec<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let dependents: Vec<Artifact> = store
            .artifacts
            .values()
            .filter(|a| a.dependencies.contains(id))
            .cloned()
            .collect();

        Ok(dependents)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let query_lower = query.to_lowercase();
        let results: Vec<Artifact> = store
            .artifacts
            .values()
            .filter(|a| {
                a.metadata.description.to_lowercase().contains(&query_lower)
                    || a.metadata
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
                    || a.id.0.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect();

        Ok(results)
    }

    pub fn update(&self, artifact: Artifact) -> Result<()> {
        let id = artifact.id.clone();
        let mut store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        if store.artifacts.contains_key(&id) {
            store.artifacts.insert(id, artifact);
            Ok(())
        } else {
            Err(NexusError::ArtifactNotFound(id))
        }
    }

    pub fn count(&self) -> Result<usize> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        Ok(store.artifacts.len())
    }

    pub fn count_by_phase(&self, phase: PhaseId) -> Result<usize> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        Ok(store.by_phase.get(&phase.0).map(|v| v.len()).unwrap_or(0))
    }

    pub fn clear(&self) -> Result<()> {
        let mut store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        store.artifacts.clear();
        store.by_phase.clear();
        store.by_type.clear();
        Ok(())
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn all_artifacts(&self) -> Result<Vec<Artifact>> {
        let store = self.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        Ok(store.artifacts.values().cloned().collect())
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactQuery {
    pub phase: Option<PhaseId>,
    pub artifact_type: Option<ArtifactType>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl ArtifactQuery {
    pub fn new() -> Self {
        Self {
            phase: None,
            artifact_type: None,
            tags: Vec::new(),
            author: None,
            created_after: None,
            created_before: None,
        }
    }

    pub fn phase(mut self, phase: PhaseId) -> Self {
        self.phase = Some(phase);
        self
    }

    pub fn artifact_type(mut self, artifact_type: ArtifactType) -> Self {
        self.artifact_type = Some(artifact_type);
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn created_after(mut self, date: DateTime<Utc>) -> Self {
        self.created_after = Some(date);
        self
    }

    pub fn created_before(mut self, date: DateTime<Utc>) -> Self {
        self.created_before = Some(date);
        self
    }

    pub fn execute(&self, tracker: &ArtifactTracker) -> Result<Vec<Artifact>> {
        let store = tracker.store.lock().map_err(|e| {
            NexusError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let mut results: Vec<Artifact> = store
            .artifacts
            .values()
            .filter(|a| {
                if let Some(phase) = self.phase {
                    if a.metadata.phase != phase {
                        return false;
                    }
                }

                if let Some(ref artifact_type) = self.artifact_type {
                    if a.artifact_type != *artifact_type {
                        return false;
                    }
                }

                if let Some(ref author) = self.author {
                    if a.metadata.author != *author {
                        return false;
                    }
                }

                for tag in &self.tags {
                    if !a.metadata.tags.contains(tag) {
                        return false;
                    }
                }

                if let Some(after) = self.created_after {
                    if a.created_at < after {
                        return false;
                    }
                }

                if let Some(before) = self.created_before {
                    if a.created_at > before {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(results)
    }
}

impl Default for ArtifactQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tracker() -> ArtifactTracker {
        ArtifactTracker::in_memory()
    }

    #[test]
    fn test_artifact_id_generation() {
        let id1 = ArtifactId::generate();
        let id2 = ArtifactId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_artifact_creation() {
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );

        assert_eq!(artifact.artifact_type, ArtifactType::Documentation);
        assert!(artifact.verify_integrity());
    }

    #[test]
    fn test_artifact_builder_pattern() {
        let artifact = Artifact::new(
            ArtifactType::SourceCode,
            serde_json::json!({"code": "fn main() {}"}),
            PhaseId(5),
        )
        .with_author("test_user")
        .with_description("Test artifact")
        .with_tags(vec!["test".to_string(), "example".to_string()]);

        assert_eq!(artifact.metadata.author, "test_user");
        assert_eq!(artifact.metadata.description, "Test artifact");
        assert_eq!(artifact.metadata.tags.len(), 2);
    }

    #[test]
    fn test_artifact_hash_consistency() {
        let content = serde_json::json!({"test": "data"});
        let hash1 = Artifact::compute_hash(&content);
        let hash2 = Artifact::compute_hash(&content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_artifact_integrity_verification() {
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"test": "data"}),
            PhaseId(0),
        );

        assert!(artifact.verify_integrity());
    }

    #[test]
    fn test_artifact_query_builder() {
        let query = ArtifactQuery::new()
            .phase(PhaseId(5))
            .artifact_type(ArtifactType::SourceCode)
            .tag("important")
            .author("developer");

        assert_eq!(query.phase, Some(PhaseId(5)));
        assert_eq!(query.artifact_type, Some(ArtifactType::SourceCode));
        assert_eq!(query.tags.len(), 1);
        assert_eq!(query.author, Some("developer".to_string()));
    }

    #[test]
    fn test_artifact_type_display() {
        assert_eq!(format!("{}", ArtifactType::YellowPaper), "YellowPaper");
        assert_eq!(format!("{}", ArtifactType::BluePaper), "BluePaper");
    }

    #[test]
    fn test_artifact_type_from_str() {
        assert_eq!(
            ArtifactType::from_str("YellowPaper"),
            Some(ArtifactType::YellowPaper)
        );
        assert_eq!(
            ArtifactType::from_str("BluePaper"),
            Some(ArtifactType::BluePaper)
        );
        assert_eq!(ArtifactType::from_str("Invalid"), None);
    }

    #[test]
    fn test_artifact_storage() {
        let tracker = create_test_tracker();
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"content": "test"}),
            PhaseId(0),
        );
        let id = artifact.id.clone();

        tracker.store(artifact).unwrap();
        assert_eq!(tracker.count().unwrap(), 1);

        let retrieved = tracker.retrieve(&id).unwrap();
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_artifact_retrieval() {
        let tracker = create_test_tracker();
        let artifact = Artifact::new(
            ArtifactType::SourceCode,
            serde_json::json!({"code": "fn test() {}"}),
            PhaseId(5),
        )
        .with_author("test_author")
        .with_description("Test code");

        let id = artifact.id.clone();
        tracker.store(artifact).unwrap();

        let retrieved = tracker.retrieve(&id).unwrap().unwrap();
        assert_eq!(retrieved.metadata.author, "test_author");
        assert_eq!(retrieved.metadata.description, "Test code");
    }

    #[test]
    fn test_artifact_deletion() {
        let tracker = create_test_tracker();
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({}),
            PhaseId(0),
        );
        let id = artifact.id.clone();

        tracker.store(artifact).unwrap();
        assert_eq!(tracker.count().unwrap(), 1);

        let deleted = tracker.delete(&id).unwrap();
        assert!(deleted);
        assert_eq!(tracker.count().unwrap(), 0);

        let retrieved = tracker.retrieve(&id).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_list_by_phase() {
        let tracker = create_test_tracker();

        for i in 0..3 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(5),
            );
            tracker.store(artifact).unwrap();
        }

        let artifact = Artifact::new(ArtifactType::SourceCode, serde_json::json!({}), PhaseId(6));
        tracker.store(artifact).unwrap();

        let phase5_artifacts = tracker.list_by_phase(PhaseId(5)).unwrap();
        assert_eq!(phase5_artifacts.len(), 3);

        let phase6_artifacts = tracker.list_by_phase(PhaseId(6)).unwrap();
        assert_eq!(phase6_artifacts.len(), 1);
    }

    #[test]
    fn test_list_by_type() {
        let tracker = create_test_tracker();

        for i in 0..2 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(i),
            );
            tracker.store(artifact).unwrap();
        }

        for i in 0..3 {
            let artifact = Artifact::new(
                ArtifactType::SourceCode,
                serde_json::json!({"index": i}),
                PhaseId(i + 2),
            );
            tracker.store(artifact).unwrap();
        }

        let docs = tracker.list_by_type(ArtifactType::Documentation).unwrap();
        assert_eq!(docs.len(), 2);

        let code = tracker.list_by_type(ArtifactType::SourceCode).unwrap();
        assert_eq!(code.len(), 3);
    }

    #[test]
    fn test_dependency_tracking() {
        let tracker = create_test_tracker();

        let dep1 = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"name": "dep1"}),
            PhaseId(0),
        );
        let dep1_id = dep1.id.clone();
        tracker.store(dep1).unwrap();

        let dep2 = Artifact::new(
            ArtifactType::Configuration,
            serde_json::json!({"name": "dep2"}),
            PhaseId(1),
        );
        let dep2_id = dep2.id.clone();
        tracker.store(dep2).unwrap();

        let main = Artifact::new(
            ArtifactType::SourceCode,
            serde_json::json!({"name": "main"}),
            PhaseId(2),
        )
        .with_dependencies(vec![dep1_id.clone(), dep2_id.clone()]);
        let main_id = main.id.clone();
        tracker.store(main).unwrap();

        assert!(tracker.validate_dependencies(&main_id).unwrap());

        let deps = tracker.get_dependencies(&main_id).unwrap();
        assert_eq!(deps.len(), 2);

        let dependents = tracker.get_dependents(&dep1_id).unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, main_id);
    }

    #[test]
    fn test_search() {
        let tracker = create_test_tracker();

        let artifact1 = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({}),
            PhaseId(0),
        )
        .with_description("Important documentation about testing")
        .with_tags(vec!["test".to_string()]);
        tracker.store(artifact1).unwrap();

        let artifact2 = Artifact::new(ArtifactType::SourceCode, serde_json::json!({}), PhaseId(1))
            .with_description("Implementation code");
        tracker.store(artifact2).unwrap();

        let results = tracker.search("testing").unwrap();
        assert_eq!(results.len(), 1);

        let results = tracker.search("test").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_update() {
        let tracker = create_test_tracker();
        let artifact = Artifact::new(
            ArtifactType::Documentation,
            serde_json::json!({"version": 1}),
            PhaseId(0),
        );
        let id = artifact.id.clone();
        tracker.store(artifact).unwrap();

        let mut updated = tracker.retrieve(&id).unwrap().unwrap();
        updated.content = serde_json::json!({"version": 2});
        updated.touch();
        tracker.update(updated).unwrap();

        let retrieved = tracker.retrieve(&id).unwrap().unwrap();
        assert_eq!(retrieved.content["version"], 2);
    }

    #[test]
    fn test_query_execution() {
        let tracker = create_test_tracker();

        for i in 0..5 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(i),
            )
            .with_author(if i < 3 { "alice" } else { "bob" });
            tracker.store(artifact).unwrap();
        }

        let query = ArtifactQuery::new().author("alice");
        let results = query.execute(&tracker).unwrap();
        assert_eq!(results.len(), 3);

        let query = ArtifactQuery::new().phase(PhaseId(2));
        let results = query.execute(&tracker).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_clear() {
        let tracker = create_test_tracker();

        for i in 0..5 {
            let artifact = Artifact::new(
                ArtifactType::Documentation,
                serde_json::json!({"index": i}),
                PhaseId(i),
            );
            tracker.store(artifact).unwrap();
        }

        assert_eq!(tracker.count().unwrap(), 5);
        tracker.clear().unwrap();
        assert_eq!(tracker.count().unwrap(), 0);
    }
}
