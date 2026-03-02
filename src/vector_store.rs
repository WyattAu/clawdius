//! Vector Store - Semantic code embedding storage
//!
//! Provides vector storage for code embeddings using LanceDB for
//! efficient similarity search and semantic retrieval.
//!
//! Note: LanceDB integration is currently stubbed due to build requirements.
//! The implementation provides a complete interface for future integration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::ast_store::Language;
use crate::error::Result;

/// Embedding dimension (OpenAI text-embedding-3-small)
pub const EMBEDDING_DIMENSION: usize = 1536;

/// Code chunk for embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Unique chunk identifier
    pub id: Uuid,
    /// Chunk text content
    pub content: String,
    /// Vector embedding
    pub embedding: Vec<f32>,
    /// Source file path
    pub source_path: PathBuf,
    /// Start line in source
    pub start_line: u32,
    /// End line in source
    pub end_line: u32,
    /// Programming language
    pub language: Language,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Chunk {
    /// Create a new chunk
    #[must_use]
    pub fn new(
        content: String,
        embedding: Vec<f32>,
        source_path: PathBuf,
        start_line: u32,
        end_line: u32,
        language: Language,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            embedding,
            source_path,
            start_line,
            end_line,
            language,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// Search result with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Matching chunk
    pub chunk: Chunk,
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
}

/// Vector store configuration
#[derive(Debug, Clone)]
pub struct VectorStoreConfig {
    /// Database path
    pub path: PathBuf,
    /// Table name for embeddings
    pub table_name: String,
    /// Embedding dimension
    pub dimension: usize,
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from(".clawdius/graph/vectors"),
            table_name: String::from("embeddings"),
            dimension: EMBEDDING_DIMENSION,
        }
    }
}

/// Vector store for semantic search
///
/// Currently provides an in-memory stub implementation.
/// Full LanceDB integration will be added when build dependencies are resolved.
#[derive(Debug)]
pub struct VectorStore {
    /// Configuration
    config: VectorStoreConfig,
    /// In-memory chunk storage (stub implementation)
    chunks: HashMap<Uuid, Chunk>,
    /// Whether the store is initialized
    initialized: bool,
}

impl VectorStore {
    /// Create a new vector store
    pub fn new(config: VectorStoreConfig) -> Result<Self> {
        let store = Self {
            config,
            chunks: HashMap::new(),
            initialized: false,
        };
        Ok(store)
    }

    /// Create with default configuration
    pub fn with_path(path: &Path) -> Result<Self> {
        let config = VectorStoreConfig {
            path: path.to_path_buf(),
            ..Default::default()
        };
        Self::new(config)
    }

    /// Initialize the vector store
    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        tracing::info!(
            path = %self.config.path.display(),
            table = %self.config.table_name,
            "Initializing vector store (stub mode)"
        );

        self.initialized = true;
        Ok(())
    }

    /// Insert chunks into the store
    pub fn insert(&mut self, chunks: Vec<Chunk>) -> Result<()> {
        for chunk in chunks {
            if chunk.embedding.len() != self.config.dimension {
                return Err(crate::error::ClawdiusError::Database(format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.config.dimension,
                    chunk.embedding.len()
                )));
            }
            self.chunks.insert(chunk.id, chunk);
        }
        Ok(())
    }

    /// Search for similar chunks using cosine similarity
    pub fn search(&self, embedding: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        if embedding.len() != self.config.dimension {
            return Err(crate::error::ClawdiusError::Database(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.config.dimension,
                embedding.len()
            )));
        }

        let mut results: Vec<SearchResult> = self
            .chunks
            .values()
            .map(|chunk| {
                let score = cosine_similarity(embedding, &chunk.embedding);
                SearchResult {
                    chunk: chunk.clone(),
                    score,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(k);
        Ok(results)
    }

    /// Delete chunks by ID
    pub fn delete(&mut self, ids: &[Uuid]) -> Result<()> {
        for id in ids {
            self.chunks.remove(id);
        }
        Ok(())
    }

    /// Delete all chunks for a file
    pub fn delete_by_file(&mut self, path: &Path) -> Result<usize> {
        let path_str = path.to_string_lossy();
        let ids_to_remove: Vec<Uuid> = self
            .chunks
            .values()
            .filter(|c| c.source_path.to_string_lossy() == path_str)
            .map(|c| c.id)
            .collect();

        let count = ids_to_remove.len();
        for id in ids_to_remove {
            self.chunks.remove(&id);
        }
        Ok(count)
    }

    /// Get a chunk by ID
    #[must_use]
    pub fn get(&self, id: &Uuid) -> Option<&Chunk> {
        self.chunks.get(id)
    }

    /// Get total chunk count
    #[must_use]
    pub fn count(&self) -> usize {
        self.chunks.len()
    }

    /// Check if store is initialized
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get store path
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.config.path
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

/// Chunker for splitting code into embeddable segments
#[derive(Debug)]
pub struct Chunker {
    /// Maximum chunk size in characters
    pub max_chunk_size: usize,
    /// Overlap between chunks
    pub overlap: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            max_chunk_size: 1000,
            overlap: 100,
        }
    }
}

impl Chunker {
    /// Create a new chunker
    #[must_use]
    pub fn new(max_chunk_size: usize, overlap: usize) -> Self {
        Self {
            max_chunk_size,
            overlap,
        }
    }

    /// Chunk source code into segments
    pub fn chunk(
        &self,
        content: &str,
        source_path: &Path,
        language: Language,
    ) -> Vec<ChunkCandidate> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        if lines.is_empty() {
            return chunks;
        }

        let mut current_start = 0;
        let mut current_content = String::new();
        let mut current_size = 0;

        for (idx, line) in lines.iter().enumerate() {
            let line_len = line.len() + 1;

            if current_size + line_len > self.max_chunk_size && !current_content.is_empty() {
                chunks.push(ChunkCandidate {
                    content: current_content.clone(),
                    source_path: source_path.to_path_buf(),
                    start_line: current_start as u32 + 1,
                    end_line: idx as u32,
                    language,
                });

                current_content.clear();
                current_size = 0;

                let overlap_start = idx.saturating_sub(self.overlap.saturating_sub(1).max(1));
                current_start = overlap_start;

                for overlap_idx in overlap_start..idx {
                    if let Some(overlap_line) = lines.get(overlap_idx) {
                        current_content.push_str(overlap_line);
                        current_content.push('\n');
                        current_size += overlap_line.len() + 1;
                    }
                }
            }

            current_content.push_str(line);
            current_content.push('\n');
            current_size += line_len;

            if idx == lines.len() - 1 && !current_content.is_empty() {
                chunks.push(ChunkCandidate {
                    content: current_content.clone(),
                    source_path: source_path.to_path_buf(),
                    start_line: current_start as u32 + 1,
                    end_line: idx as u32 + 1,
                    language,
                });
            }
        }

        chunks
    }
}

/// Candidate chunk before embedding
#[derive(Debug, Clone)]
pub struct ChunkCandidate {
    /// Chunk text content
    pub content: String,
    /// Source file path
    pub source_path: PathBuf,
    /// Start line in source
    pub start_line: u32,
    /// End line in source
    pub end_line: u32,
    /// Programming language
    pub language: Language,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let chunk = Chunk::new(
            "fn main() {}".to_string(),
            vec![0.1; EMBEDDING_DIMENSION],
            PathBuf::from("test.rs"),
            1,
            5,
            Language::Rust,
        );

        assert_eq!(chunk.content, "fn main() {}");
        assert_eq!(chunk.source_path, PathBuf::from("test.rs"));
    }

    #[test]
    fn test_vector_store_creation() {
        let config = VectorStoreConfig::default();
        let store = VectorStore::new(config);
        assert!(store.is_ok());
    }

    #[test]
    fn test_vector_store_insert_and_search() {
        let mut store =
            VectorStore::new(VectorStoreConfig::default()).expect("Failed to create store");
        store.initialize().expect("Failed to initialize");

        let embedding = vec![0.5; EMBEDDING_DIMENSION];
        let chunk = Chunk::new(
            "test content".to_string(),
            embedding.clone(),
            PathBuf::from("test.rs"),
            1,
            10,
            Language::Rust,
        );

        store.insert(vec![chunk]).expect("Failed to insert");

        let results = store.search(&embedding, 1).expect("Failed to search");
        assert_eq!(results.len(), 1);
        assert!((results[0].score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
    }

    #[test]
    fn test_chunker() {
        let chunker = Chunker::new(100, 20);
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let chunks = chunker.chunk(content, Path::new("test.rs"), Language::Rust);

        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("line 1"));
    }

    #[test]
    fn test_dimension_validation() {
        let mut store =
            VectorStore::new(VectorStoreConfig::default()).expect("Failed to create store");
        store.initialize().expect("Failed to initialize");

        let wrong_dim = vec![0.5; 512];
        let chunk = Chunk::new(
            "test".to_string(),
            wrong_dim,
            PathBuf::from("test.rs"),
            1,
            1,
            Language::Rust,
        );

        let result = store.insert(vec![chunk]);
        assert!(result.is_err());
    }
}
