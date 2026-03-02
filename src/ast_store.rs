//! AST Store - SQLite-based structural code storage
//!
//! Provides persistent storage for AST nodes and edges with efficient
//! query capabilities for graph traversal and code analysis.

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::Result;

/// Unique identifier for an AST node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// Create a new random node ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from bytes
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// Get as bytes
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// AST node types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Module declaration
    Module,
    /// Function definition
    Function,
    /// Struct definition
    Struct,
    /// Enum definition
    Enum,
    /// Trait definition
    Trait,
    /// Impl block
    Impl,
    /// Type alias
    TypeAlias,
    /// Constant
    Constant,
    /// Static variable
    Static,
    /// Use/import statement
    Use,
    /// Mod declaration
    Mod,
    /// Macro definition
    Macro,
    /// Field in struct
    Field,
    /// Enum variant
    Variant,
    /// Function parameter
    Parameter,
    /// Local variable
    Local,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Module => write!(f, "module"),
            Self::Function => write!(f, "function"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Impl => write!(f, "impl"),
            Self::TypeAlias => write!(f, "type_alias"),
            Self::Constant => write!(f, "constant"),
            Self::Static => write!(f, "static"),
            Self::Use => write!(f, "use"),
            Self::Mod => write!(f, "mod"),
            Self::Macro => write!(f, "macro"),
            Self::Field => write!(f, "field"),
            Self::Variant => write!(f, "variant"),
            Self::Parameter => write!(f, "parameter"),
            Self::Local => write!(f, "local"),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "module" => Ok(Self::Module),
            "function" => Ok(Self::Function),
            "struct" => Ok(Self::Struct),
            "enum" => Ok(Self::Enum),
            "trait" => Ok(Self::Trait),
            "impl" => Ok(Self::Impl),
            "type_alias" => Ok(Self::TypeAlias),
            "constant" => Ok(Self::Constant),
            "static" => Ok(Self::Static),
            "use" => Ok(Self::Use),
            "mod" => Ok(Self::Mod),
            "macro" => Ok(Self::Macro),
            "field" => Ok(Self::Field),
            "variant" => Ok(Self::Variant),
            "parameter" => Ok(Self::Parameter),
            "local" => Ok(Self::Local),
            _ => Err(format!("Unknown node type: {s}")),
        }
    }
}

/// AST edge types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Function call
    Calls,
    /// Definition relationship
    Defines,
    /// Trait implementation
    Implements,
    /// Import relationship
    Imports,
    /// Containment relationship
    Contains,
    /// Reference relationship
    References,
    /// Extension relationship
    Extends,
    /// Compliance relationship (SOP, spec)
    CompliesWith,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Calls => write!(f, "calls"),
            Self::Defines => write!(f, "defines"),
            Self::Implements => write!(f, "implements"),
            Self::Imports => write!(f, "imports"),
            Self::Contains => write!(f, "contains"),
            Self::References => write!(f, "references"),
            Self::Extends => write!(f, "extends"),
            Self::CompliesWith => write!(f, "complies_with"),
        }
    }
}

impl std::str::FromStr for EdgeType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "calls" => Ok(Self::Calls),
            "defines" => Ok(Self::Defines),
            "implements" => Ok(Self::Implements),
            "imports" => Ok(Self::Imports),
            "contains" => Ok(Self::Contains),
            "references" => Ok(Self::References),
            "extends" => Ok(Self::Extends),
            "complies_with" => Ok(Self::CompliesWith),
            _ => Err(format!("Unknown edge type: {s}")),
        }
    }
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Rust
    Rust,
    /// TypeScript
    TypeScript,
    /// Python
    Python,
    /// C++
    Cpp,
    /// Go
    Go,
    /// Java
    Java,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => write!(f, "rust"),
            Self::TypeScript => write!(f, "typescript"),
            Self::Python => write!(f, "python"),
            Self::Cpp => write!(f, "cpp"),
            Self::Go => write!(f, "go"),
            Self::Java => write!(f, "java"),
        }
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" => Ok(Self::Rust),
            "typescript" | "ts" => Ok(Self::TypeScript),
            "python" | "py" => Ok(Self::Python),
            "cpp" | "c++" => Ok(Self::Cpp),
            "go" | "golang" => Ok(Self::Go),
            "java" => Ok(Self::Java),
            _ => Err(format!("Unknown language: {s}")),
        }
    }
}

/// AST node representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Node type
    pub node_type: NodeType,
    /// Node name (function name, struct name, etc.)
    pub name: Option<String>,
    /// Source file path
    pub file_path: PathBuf,
    /// Start byte offset
    pub start_byte: u32,
    /// End byte offset
    pub end_byte: u32,
    /// Start line number
    pub start_line: u32,
    /// End line number
    pub end_line: u32,
    /// Programming language
    pub language: Language,
    /// Documentation comments
    pub documentation: Option<String>,
    /// Additional metadata as JSON
    pub metadata: Option<String>,
    /// Content hash for change detection
    pub hash: [u8; 32],
}

/// AST edge representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstEdge {
    /// Unique edge identifier
    pub id: Uuid,
    /// Source node
    pub from: NodeId,
    /// Target node
    pub to: NodeId,
    /// Edge type
    pub edge_type: EdgeType,
    /// Edge weight (for ranking)
    pub weight: f32,
    /// Additional metadata as JSON
    pub metadata: Option<String>,
}

/// Query for AST nodes
#[derive(Debug, Clone, Default)]
pub struct AstQuery {
    /// Filter by node type
    pub node_type: Option<NodeType>,
    /// Filter by name pattern (supports wildcards)
    pub name_pattern: Option<String>,
    /// Filter by file path
    pub file_path: Option<PathBuf>,
    /// Filter by language
    pub language: Option<Language>,
    /// Maximum results
    pub limit: Option<usize>,
}

/// Statistics about the AST index
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    /// Total number of nodes
    pub node_count: usize,
    /// Total number of edges
    pub edge_count: usize,
    /// Nodes by type
    pub nodes_by_type: HashMap<String, usize>,
    /// Edges by type
    pub edges_by_type: HashMap<String, usize>,
    /// Files indexed
    pub files_indexed: usize,
}

/// Call graph for a function
#[derive(Debug, Clone)]
pub struct CallGraph {
    /// Root function node
    pub root: NodeId,
    /// All nodes in the call graph
    pub nodes: Vec<AstNode>,
    /// All edges in the call graph
    pub edges: Vec<AstEdge>,
}

/// SQLite-based AST storage
#[derive(Debug)]
pub struct AstStore {
    /// Database connection
    conn: Connection,
    /// Database path
    path: PathBuf,
}

impl AstStore {
    /// Create a new AST store at the given path
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn,
            path: path.to_path_buf(),
        };
        store.initialize()?;
        Ok(store)
    }

    /// Create an in-memory AST store
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn,
            path: PathBuf::from(":memory:"),
        };
        store.initialize()?;
        Ok(store)
    }

    /// Initialize database schema
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS nodes (
                id BLOB PRIMARY KEY,
                type TEXT NOT NULL,
                name TEXT,
                file_path TEXT NOT NULL,
                start_byte INTEGER NOT NULL,
                end_byte INTEGER NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL,
                documentation TEXT,
                metadata TEXT,
                hash BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS edges (
                id BLOB PRIMARY KEY,
                source_id BLOB NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
                target_id BLOB NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
                type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes(type);
            CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
            CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file_path);
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(type);
            ",
        )?;
        Ok(())
    }

    /// Insert a node into the store
    pub fn insert_node(&self, node: &AstNode) -> Result<()> {
        self.conn.execute(
            r"
            INSERT OR REPLACE INTO nodes (
                id, type, name, file_path, start_byte, end_byte,
                start_line, end_line, language, documentation, metadata, hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                node.id.as_bytes(),
                node.node_type.to_string(),
                node.name,
                node.file_path.to_string_lossy(),
                node.start_byte,
                node.end_byte,
                node.start_line,
                node.end_line,
                node.language.to_string(),
                node.documentation,
                node.metadata,
                node.hash,
            ],
        )?;
        Ok(())
    }

    /// Insert multiple nodes in a batch
    pub fn insert_nodes(&self, nodes: &[AstNode]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for node in nodes {
            self.insert_node(node)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Insert an edge into the store
    pub fn insert_edge(&self, edge: &AstEdge) -> Result<()> {
        self.conn.execute(
            r"
            INSERT OR REPLACE INTO edges (id, source_id, target_id, type, weight, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                edge.id.as_bytes(),
                edge.from.as_bytes(),
                edge.to.as_bytes(),
                edge.edge_type.to_string(),
                edge.weight,
                edge.metadata,
            ],
        )?;
        Ok(())
    }

    /// Insert multiple edges in a batch
    pub fn insert_edges(&self, edges: &[AstEdge]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for edge in edges {
            self.insert_edge(edge)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Result<Option<AstNode>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, type, name, file_path, start_byte, end_byte,
                   start_line, end_line, language, documentation, metadata, hash
            FROM nodes WHERE id = ?1
            ",
        )?;

        let mut rows = stmt.query(params![id.as_bytes()])?;

        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_node(row)?))
        } else {
            Ok(None)
        }
    }

    /// Query nodes with filters
    pub fn query_nodes(&self, query: &AstQuery) -> Result<Vec<AstNode>> {
        let mut sql = String::from(
            "SELECT id, type, name, file_path, start_byte, end_byte, \
             start_line, end_line, language, documentation, metadata, hash FROM nodes WHERE 1=1",
        );
        let mut p: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref node_type) = query.node_type {
            sql.push_str(" AND type = ?");
            p.push(Box::new(node_type.to_string()));
        }

        if let Some(ref name_pattern) = query.name_pattern {
            sql.push_str(" AND name LIKE ?");
            p.push(Box::new(name_pattern.replace('*', "%")));
        }

        if let Some(ref file_path) = query.file_path {
            sql.push_str(" AND file_path = ?");
            p.push(Box::new(file_path.to_string_lossy().to_string()));
        }

        if let Some(ref language) = query.language {
            sql.push_str(" AND language = ?");
            p.push(Box::new(language.to_string()));
        }

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let params: Vec<&dyn rusqlite::ToSql> = p.iter().map(|x| x.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| self.row_to_node(row))?;

        let mut nodes = Vec::new();
        for node in rows {
            nodes.push(node?);
        }

        Ok(nodes)
    }

    /// Convert a row to an AstNode
    fn row_to_node(&self, row: &Row<'_>) -> rusqlite::Result<AstNode> {
        let id_bytes: [u8; 16] = row
            .get::<_, Vec<u8>>(0)?
            .try_into()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

        let hash_bytes: [u8; 32] = row
            .get::<_, Vec<u8>>(11)?
            .try_into()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

        let node_type_str: String = row.get(1)?;
        let language_str: String = row.get(8)?;

        Ok(AstNode {
            id: NodeId::from_bytes(id_bytes),
            node_type: node_type_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            name: row.get(2)?,
            file_path: PathBuf::from(row.get::<_, String>(3)?),
            start_byte: row.get(4)?,
            end_byte: row.get(5)?,
            start_line: row.get(6)?,
            end_line: row.get(7)?,
            language: language_str
                .parse()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            documentation: row.get(9)?,
            metadata: row.get(10)?,
            hash: hash_bytes,
        })
    }

    /// Get all edges from a node
    pub fn get_outgoing_edges(&self, node_id: &NodeId) -> Result<Vec<AstEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, type, weight, metadata FROM edges WHERE source_id = ?1"
        )?;

        let rows = stmt.query_map(params![node_id.as_bytes()], |row| {
            let id_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(0)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let source_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(1)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let target_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(2)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            let edge_type_str: String = row.get(3)?;

            Ok(AstEdge {
                id: Uuid::from_bytes(id_bytes),
                from: NodeId::from_bytes(source_bytes),
                to: NodeId::from_bytes(target_bytes),
                edge_type: edge_type_str
                    .parse()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                weight: row.get(4)?,
                metadata: row.get(5)?,
            })
        })?;

        let mut edges = Vec::new();
        for edge in rows {
            edges.push(edge?);
        }

        Ok(edges)
    }

    /// Get all edges to a node
    pub fn get_incoming_edges(&self, node_id: &NodeId) -> Result<Vec<AstEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_id, target_id, type, weight, metadata FROM edges WHERE target_id = ?1"
        )?;

        let rows = stmt.query_map(params![node_id.as_bytes()], |row| {
            let id_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(0)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let source_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(1)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let target_bytes: [u8; 16] = row
                .get::<_, Vec<u8>>(2)?
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            let edge_type_str: String = row.get(3)?;

            Ok(AstEdge {
                id: Uuid::from_bytes(id_bytes),
                from: NodeId::from_bytes(source_bytes),
                to: NodeId::from_bytes(target_bytes),
                edge_type: edge_type_str
                    .parse()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                weight: row.get(4)?,
                metadata: row.get(5)?,
            })
        })?;

        let mut edges = Vec::new();
        for edge in rows {
            edges.push(edge?);
        }

        Ok(edges)
    }

    /// Delete all nodes for a file
    pub fn delete_file_nodes(&self, file_path: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM nodes WHERE file_path = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(count)
    }

    /// Get call graph for a function
    pub fn get_call_graph(&self, function_id: &NodeId) -> Result<CallGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![*function_id];

        while let Some(current_id) = stack.pop() {
            if visited.contains(&current_id) {
                continue;
            }
            visited.insert(current_id);

            if let Some(node) = self.get_node(&current_id)? {
                nodes.push(node);
            }

            let outgoing = self.get_outgoing_edges(&current_id)?;
            for edge in outgoing {
                if edge.edge_type == EdgeType::Calls {
                    edges.push(edge.clone());
                    if !visited.contains(&edge.to) {
                        stack.push(edge.to);
                    }
                }
            }
        }

        Ok(CallGraph {
            root: *function_id,
            nodes,
            edges,
        })
    }

    /// Find all nodes impacted by changes to a node
    pub fn find_impact(&self, node_id: &NodeId) -> Result<Vec<NodeId>> {
        let mut impacted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![*node_id];

        while let Some(current_id) = stack.pop() {
            if visited.contains(&current_id) {
                continue;
            }
            visited.insert(current_id);

            let incoming = self.get_incoming_edges(&current_id)?;
            for edge in incoming {
                if matches!(
                    edge.edge_type,
                    EdgeType::Calls | EdgeType::References | EdgeType::Imports
                ) {
                    if !visited.contains(&edge.from) {
                        impacted.push(edge.from);
                        stack.push(edge.from);
                    }
                }
            }
        }

        Ok(impacted)
    }

    /// Get index statistics
    pub fn stats(&self) -> Result<IndexStats> {
        let node_count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get::<_, i64>(0))?
            as usize;
        let edge_count: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get::<_, i64>(0))?
            as usize;
        let files_indexed: usize =
            self.conn
                .query_row("SELECT COUNT(DISTINCT file_path) FROM nodes", [], |row| {
                    row.get::<_, i64>(0)
                })? as usize;

        let mut nodes_by_type = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT type, COUNT(*) FROM nodes GROUP BY type")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        for row in rows {
            let (t, c) = row?;
            nodes_by_type.insert(t, c);
        }

        let mut edges_by_type = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT type, COUNT(*) FROM edges GROUP BY type")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        for row in rows {
            let (t, c) = row?;
            edges_by_type.insert(t, c);
        }

        Ok(IndexStats {
            node_count,
            edge_count,
            nodes_by_type,
            edges_by_type,
            files_indexed,
        })
    }

    /// Get the database path
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Compute SHA3-256 hash of content
#[must_use]
pub fn compute_hash(content: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(content);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_node_id_creation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_ast_store_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let db_path = dir.path().join("test.db");
        let store = AstStore::new(&db_path);
        assert!(store.is_ok());
    }

    #[test]
    fn test_insert_and_get_node() {
        let store = AstStore::in_memory().expect("Failed to create store");

        let node = AstNode {
            id: NodeId::new(),
            node_type: NodeType::Function,
            name: Some("test_function".to_string()),
            file_path: PathBuf::from("test.rs"),
            start_byte: 0,
            end_byte: 100,
            start_line: 1,
            end_line: 10,
            language: Language::Rust,
            documentation: None,
            metadata: None,
            hash: compute_hash(b"test content"),
        };

        store.insert_node(&node).expect("Failed to insert node");

        let retrieved = store.get_node(&node.id).expect("Failed to get node");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, Some("test_function".to_string()));
        assert_eq!(retrieved.node_type, NodeType::Function);
    }

    #[test]
    fn test_query_nodes() {
        let store = AstStore::in_memory().expect("Failed to create store");

        let node1 = AstNode {
            id: NodeId::new(),
            node_type: NodeType::Function,
            name: Some("func_a".to_string()),
            file_path: PathBuf::from("test.rs"),
            start_byte: 0,
            end_byte: 100,
            start_line: 1,
            end_line: 10,
            language: Language::Rust,
            documentation: None,
            metadata: None,
            hash: compute_hash(b"test"),
        };

        let node2 = AstNode {
            id: NodeId::new(),
            node_type: NodeType::Struct,
            name: Some("StructB".to_string()),
            file_path: PathBuf::from("test.rs"),
            start_byte: 101,
            end_byte: 200,
            start_line: 12,
            end_line: 20,
            language: Language::Rust,
            documentation: None,
            metadata: None,
            hash: compute_hash(b"test2"),
        };

        store
            .insert_nodes(&[node1, node2])
            .expect("Failed to insert nodes");

        let query = AstQuery {
            node_type: Some(NodeType::Function),
            ..Default::default()
        };

        let results = store.query_nodes(&query).expect("Failed to query nodes");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, Some("func_a".to_string()));
    }

    #[test]
    fn test_insert_and_get_edges() {
        let store = AstStore::in_memory().expect("Failed to create store");

        let node1_id = NodeId::new();
        let node2_id = NodeId::new();

        let node1 = AstNode {
            id: node1_id,
            node_type: NodeType::Function,
            name: Some("caller".to_string()),
            file_path: PathBuf::from("test.rs"),
            start_byte: 0,
            end_byte: 100,
            start_line: 1,
            end_line: 10,
            language: Language::Rust,
            documentation: None,
            metadata: None,
            hash: compute_hash(b"test"),
        };

        let node2 = AstNode {
            id: node2_id,
            node_type: NodeType::Function,
            name: Some("callee".to_string()),
            file_path: PathBuf::from("test.rs"),
            start_byte: 101,
            end_byte: 200,
            start_line: 12,
            end_line: 20,
            language: Language::Rust,
            documentation: None,
            metadata: None,
            hash: compute_hash(b"test2"),
        };

        store
            .insert_nodes(&[node1, node2])
            .expect("Failed to insert nodes");

        let edge = AstEdge {
            id: Uuid::new_v4(),
            from: node1_id,
            to: node2_id,
            edge_type: EdgeType::Calls,
            weight: 1.0,
            metadata: None,
        };

        store.insert_edge(&edge).expect("Failed to insert edge");

        let outgoing = store
            .get_outgoing_edges(&node1_id)
            .expect("Failed to get outgoing edges");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].edge_type, EdgeType::Calls);

        let incoming = store
            .get_incoming_edges(&node2_id)
            .expect("Failed to get incoming edges");
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn test_index_stats() {
        let store = AstStore::in_memory().expect("Failed to create store");

        let stats = store.stats().expect("Failed to get stats");
        assert_eq!(stats.node_count, 0);
        assert_eq!(stats.edge_count, 0);
    }
}
