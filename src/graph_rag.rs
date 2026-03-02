//! Graph-RAG Component - Knowledge retrieval interface
//!
//! Combines structural (AST) and semantic (vector) code understanding
//! for intelligent code analysis, refactoring, and research synthesis.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::ast_store::{AstNode, AstQuery, AstStore, CallGraph, IndexStats, NodeId};
use crate::component::{Component, ComponentId, ComponentInfo, ComponentState};
use crate::error::{ClawdiusError, Result};
use crate::mcp::McpHost;
use crate::parser::{LanguageDetector, Parser, ParsedFile};
use crate::vector_store::{Chunker, SearchResult, VectorStore, VectorStoreConfig};

/// Graph-RAG component version
pub const GRAPH_RAG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Hybrid query combining structural and semantic search
#[derive(Debug, Clone)]
pub struct HybridQuery {
    /// Natural language query for semantic search
    pub semantic_query: Option<String>,
    /// AST query for structural search
    pub structural_query: Option<AstQuery>,
    /// Number of results
    pub k: usize,
    /// Weight for semantic results (0.0 to 1.0)
    pub semantic_weight: f32,
}

impl Default for HybridQuery {
    fn default() -> Self {
        Self {
            semantic_query: None,
            structural_query: None,
            k: 10,
            semantic_weight: 0.5,
        }
    }
}

/// Combined query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// AST nodes from structural search
    pub nodes: Vec<AstNode>,
    /// Chunks from semantic search
    pub chunks: Vec<SearchResult>,
    /// Combined relevance score
    pub score: f32,
}

/// Graph-RAG configuration
#[derive(Debug, Clone)]
pub struct GraphRagConfig {
    /// Root path for the graph database
    pub root_path: PathBuf,
    /// AST database path
    pub ast_db_path: PathBuf,
    /// Vector store path
    pub vector_store_path: PathBuf,
    /// Maximum chunk size
    pub max_chunk_size: usize,
    /// Chunk overlap
    pub chunk_overlap: usize,
}

impl Default for GraphRagConfig {
    fn default() -> Self {
        Self {
            root_path: PathBuf::from(".clawdius/graph"),
            ast_db_path: PathBuf::from(".clawdius/graph/ast.db"),
            vector_store_path: PathBuf::from(".clawdius/graph/vectors"),
            max_chunk_size: 1000,
            chunk_overlap: 100,
        }
    }
}

impl GraphRagConfig {
    /// Create config with custom root path
    #[must_use]
    pub fn with_root(root: &Path) -> Self {
        Self {
            root_path: root.to_path_buf(),
            ast_db_path: root.join("ast.db"),
            vector_store_path: root.join("vectors"),
            ..Default::default()
        }
    }
}

/// Graph-RAG component - the knowledge layer for Clawdius
#[derive(Debug)]
pub struct GraphRag {
    /// Component info
    info: ComponentInfo,
    /// Component state
    state: ComponentState,
    /// Configuration
    config: GraphRagConfig,
    /// AST storage (wrapped in Mutex for thread safety)
    ast_store: Option<Mutex<AstStore>>,
    /// Vector storage
    vector_store: Option<VectorStore>,
    /// Parser
    parser: Option<Parser>,
    /// Chunker
    chunker: Chunker,
    /// MCP host
    mcp_host: McpHost,
    /// Language detector
    detector: LanguageDetector,
}

impl GraphRag {
    /// Create a new Graph-RAG component
    pub fn new(config: GraphRagConfig) -> Result<Self> {
        let info = ComponentInfo::new(
            ComponentId::GRAPH,
            "Graph-RAG",
            GRAPH_RAG_VERSION,
        );

        Ok(Self {
            info,
            state: ComponentState::Uninitialized,
            config,
            ast_store: None,
            vector_store: None,
            parser: None,
            chunker: Chunker::new(1000, 100),
            mcp_host: McpHost::new(),
            detector: LanguageDetector::new(),
        })
    }

    /// Create with default configuration
    pub fn with_root(root: &Path) -> Result<Self> {
        Self::new(GraphRagConfig::with_root(root))
    }

    /// Index a project directory
    pub async fn index_project(&mut self, root: &Path) -> Result<IndexStats> {
        if !root.is_dir() {
            return Err(ClawdiusError::Database(format!(
                "Path is not a directory: {}",
                root.display()
            )));
        }

        tracing::info!(path = %root.display(), "Starting project indexing");

        let mut total_nodes = 0;
        let mut total_edges = 0;
        let mut files_indexed = 0;

        self.walk_and_index(root, root, &mut total_nodes, &mut total_edges, &mut files_indexed)?;

        let stats = self.ast_store()
            .ok_or_else(|| ClawdiusError::Database("AST store not initialized".into()))?
            .stats()?;

        tracing::info!(
            nodes = stats.node_count,
            edges = stats.edge_count,
            files = stats.files_indexed,
            "Project indexing complete"
        );

        Ok(stats)
    }

    fn walk_and_index(
        &mut self,
        root: &Path,
        current: &Path,
        total_nodes: &mut usize,
        total_edges: &mut usize,
        files_indexed: &mut usize,
    ) -> Result<()> {
        let entries = std::fs::read_dir(current)
            .map_err(|e| ClawdiusError::Database(format!("Failed to read directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| ClawdiusError::Database(format!("Failed to read entry: {e}")))?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
                    continue;
                }

                self.walk_and_index(root, &path, total_nodes, total_edges, files_indexed)?;
            } else if self.detector.is_supported(&path) {
                match self.index_file(&path) {
                    Ok(parsed) => {
                        *total_nodes += parsed.nodes.len();
                        *total_edges += parsed.edges.len();
                        *files_indexed += 1;
                    }
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "Failed to index file");
                    }
                }
            }
        }

        Ok(())
    }

    /// Index a single file
    pub fn index_file(&mut self, path: &Path) -> Result<ParsedFile> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ClawdiusError::Database(format!("Failed to read file {}: {e}", path.display())))?;

        let parser = self.parser.as_mut()
            .ok_or_else(|| ClawdiusError::Database("Parser not initialized".into()))?;

        let parsed = parser.parse(path, &content)?;

        if let Some(ast_store) = &self.ast_store {
            let store = ast_store.lock().map_err(|_| {
                ClawdiusError::Database("Failed to lock AST store".into())
            })?;
            store.delete_file_nodes(path)?;
            store.insert_nodes(&parsed.nodes)?;
            store.insert_edges(&parsed.edges)?;
        }

        Ok(parsed)
    }

    /// Query AST graph structurally
    pub fn query_structural(&self, query: AstQuery) -> Result<Vec<AstNode>> {
        let ast_store = self.ast_store()
            .ok_or_else(|| ClawdiusError::Database("AST store not initialized".into()))?;

        ast_store.query_nodes(&query)
    }

    /// Query vector store semantically
    pub async fn query_semantic(&self, query: &str, k: usize) -> Result<Vec<SearchResult>> {
        let _vector_store = self.vector_store()
            .ok_or_else(|| ClawdiusError::Database("Vector store not initialized".into()))?;

        let embedding = self.generate_embedding(query).await?;

        let vector_store = self.vector_store.as_ref()
            .ok_or_else(|| ClawdiusError::Database("Vector store not initialized".into()))?;

        vector_store.search(&embedding, k)
    }

    /// Hybrid query combining structural and semantic search
    pub async fn hybrid_query(&self, query: HybridQuery) -> Result<QueryResult> {
        let mut nodes = Vec::new();
        let mut chunks = Vec::new();

        if let Some(ref structural) = query.structural_query {
            nodes = self.query_structural(structural.clone())?;
        }

        if let Some(ref semantic) = query.semantic_query {
            chunks = self.query_semantic(semantic, query.k).await?;
        }

        let score = if !chunks.is_empty() {
            chunks.iter().map(|c| c.score).sum::<f32>() / chunks.len() as f32
        } else {
            0.0
        };

        Ok(QueryResult {
            nodes,
            chunks,
            score,
        })
    }

    /// Get call graph for a function
    pub fn get_call_graph(&self, function: &NodeId) -> Result<CallGraph> {
        let ast_store = self.ast_store()
            .ok_or_else(|| ClawdiusError::Database("AST store not initialized".into()))?;

        ast_store.get_call_graph(function)
    }

    /// Find all nodes impacted by changes to a node
    pub fn find_impact(&self, node: &NodeId) -> Result<Vec<NodeId>> {
        let ast_store = self.ast_store()
            .ok_or_else(|| ClawdiusError::Database("AST store not initialized".into()))?;

        ast_store.find_impact(node)
    }

    /// Get AST store reference (locked)
    pub fn ast_store(&self) -> Option<std::sync::MutexGuard<'_, AstStore>> {
        self.ast_store.as_ref().and_then(|m| m.lock().ok())
    }

    /// Get vector store reference
    #[must_use]
    pub fn vector_store(&self) -> Option<&VectorStore> {
        self.vector_store.as_ref()
    }

    /// Get MCP host reference
    #[must_use]
    pub fn mcp_host(&self) -> &McpHost {
        &self.mcp_host
    }

    /// Get MCP host mutable reference
    pub fn mcp_host_mut(&mut self) -> &mut McpHost {
        &mut self.mcp_host
    }

    /// Get index statistics
    pub fn stats(&self) -> Result<IndexStats> {
        let ast_store = self.ast_store()
            .ok_or_else(|| ClawdiusError::Database("AST store not initialized".into()))?;

        ast_store.stats()
    }

    async fn generate_embedding(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0; crate::vector_store::EMBEDDING_DIMENSION])
    }
}

impl Component for GraphRag {
    fn id(&self) -> ComponentId {
        self.info.id
    }

    fn name(&self) -> &'static str {
        self.info.name
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        if self.state != ComponentState::Uninitialized {
            return Err(ClawdiusError::Database("Graph-RAG already initialized".into()));
        }

        tracing::info!("Initializing Graph-RAG component");

        if let Some(parent) = self.config.root_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ClawdiusError::Database(format!("Failed to create directories: {e}")))?;
        }
        std::fs::create_dir_all(&self.config.root_path)
            .map_err(|e| ClawdiusError::Database(format!("Failed to create graph directory: {e}")))?;

        self.ast_store = Some(Mutex::new(AstStore::new(&self.config.ast_db_path)?));

        let vector_config = VectorStoreConfig {
            path: self.config.vector_store_path.clone(),
            ..Default::default()
        };
        let mut vector_store = VectorStore::new(vector_config)?;
        vector_store.initialize()?;
        self.vector_store = Some(vector_store);

        self.parser = Some(Parser::new()?);

        self.state = ComponentState::Initialized;
        self.info.state = ComponentState::Initialized;

        tracing::info!("Graph-RAG component initialized");
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        if self.state != ComponentState::Initialized {
            return Err(ClawdiusError::Database(
                "Graph-RAG must be initialized before starting".into()
            ));
        }

        tracing::info!("Starting Graph-RAG component");
        self.state = ComponentState::Running;
        self.info.state = ComponentState::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if self.state != ComponentState::Running {
            return Err(ClawdiusError::Database("Graph-RAG is not running".into()));
        }

        tracing::info!("Stopping Graph-RAG component");
        self.state = ComponentState::Stopped;
        self.info.state = ComponentState::Stopped;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_graph_rag_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = GraphRagConfig::with_root(dir.path());
        let graph_rag = GraphRag::new(config);
        assert!(graph_rag.is_ok());
    }

    #[test]
    fn test_graph_rag_component_trait() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = GraphRagConfig::with_root(dir.path());
        let graph_rag = GraphRag::new(config).expect("Failed to create Graph-RAG");

        assert_eq!(graph_rag.id(), ComponentId::GRAPH);
        assert_eq!(graph_rag.name(), "Graph-RAG");
        assert_eq!(graph_rag.state(), ComponentState::Uninitialized);
    }

    #[test]
    fn test_graph_rag_initialize() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = GraphRagConfig::with_root(dir.path());
        let mut graph_rag = GraphRag::new(config).expect("Failed to create Graph-RAG");

        let result = graph_rag.initialize();
        assert!(result.is_ok());
        assert_eq!(graph_rag.state(), ComponentState::Initialized);
    }

    #[test]
    fn test_graph_rag_start_stop() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = GraphRagConfig::with_root(dir.path());
        let mut graph_rag = GraphRag::new(config).expect("Failed to create Graph-RAG");

        graph_rag.initialize().expect("Failed to initialize");
        let result = graph_rag.start();
        assert!(result.is_ok());
        assert_eq!(graph_rag.state(), ComponentState::Running);

        let result = graph_rag.stop();
        assert!(result.is_ok());
        assert_eq!(graph_rag.state(), ComponentState::Stopped);
    }

    #[test]
    fn test_mcp_host_available() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = GraphRagConfig::with_root(dir.path());
        let graph_rag = GraphRag::new(config).expect("Failed to create Graph-RAG");

        let host = graph_rag.mcp_host();
        assert!(host.tool_count() > 0);
    }

    #[test]
    fn test_hybrid_query_default() {
        let query = HybridQuery::default();
        assert_eq!(query.k, 10);
        assert!((query.semantic_weight - 0.5).abs() < 0.001);
    }
}
