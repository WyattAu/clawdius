//! Incremental code generation
//!
//! This module provides utilities for generating code incrementally in small,
//! manageable chunks rather than all at once. This approach:
//!
//! - Reduces risk of large, error-prone generations
//! - Allows for progressive refinement
//! - Enables better context management
//! - Supports iterative development workflows

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::Result;

/// Configuration for incremental generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalConfig {
    /// Maximum chunk size in characters
    pub max_chunk_size: usize,
    /// Whether to include context from previous chunks
    pub include_context: bool,
    /// Number of context lines to include
    pub context_lines: usize,
    /// Whether to validate each chunk
    pub validate_chunks: bool,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 2000,
            include_context: true,
            context_lines: 10,
            validate_chunks: true,
        }
    }
}

/// A chunk of generated code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// Chunk index (0-based)
    pub index: usize,
    /// Total expected chunks
    pub total: usize,
    /// File path for this chunk
    pub file_path: PathBuf,
    /// Content of this chunk
    pub content: String,
    /// Whether this is the final chunk
    pub is_final: bool,
    /// Line number where this chunk starts
    pub start_line: usize,
    /// Line number where this chunk ends
    pub end_line: usize,
    /// Chunk metadata
    pub metadata: HashMap<String, String>,
}

impl CodeChunk {
    /// Create a new code chunk
    #[must_use]
    pub fn new(
        index: usize,
        total: usize,
        file_path: PathBuf,
        content: String,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        Self {
            index,
            total,
            file_path,
            content,
            is_final: index == total - 1,
            start_line,
            end_line,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to this chunk
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get progress percentage (0-100)
    #[must_use]
    pub fn progress_percent(&self) -> u8 {
        if self.total == 0 {
            return 100;
        }
        ((self.index + 1) * 100 / self.total) as u8
    }
}

/// State for incremental generation
#[derive(Debug, Clone, Default)]
pub enum GenerationState {
    /// Not started
    #[default]
    Idle,
    /// Currently generating
    InProgress {
        /// Current chunk index
        current_chunk: usize,
        /// Total chunks expected
        total_chunks: usize,
        /// File being generated
        file_path: PathBuf,
    },
    /// Generation complete
    Complete {
        /// Total chunks generated
        total_chunks: usize,
        /// Files generated
        files: Vec<PathBuf>,
    },
    /// Generation failed
    Failed {
        /// Error message
        error: String,
        /// Chunk where failure occurred
        failed_at_chunk: Option<usize>,
    },
}

/// Incremental code generator
pub struct IncrementalGenerator {
    config: IncrementalConfig,
    state: Arc<RwLock<GenerationState>>,
    chunks: Arc<RwLock<Vec<CodeChunk>>>,
    assembled: Arc<RwLock<String>>,
}

impl IncrementalGenerator {
    /// Create a new incremental generator
    #[must_use]
    pub fn new(config: IncrementalConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(GenerationState::Idle)),
            chunks: Arc::new(RwLock::new(Vec::new())),
            assembled: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Get current generation state
    pub async fn state(&self) -> GenerationState {
        self.state.read().await.clone()
    }

    /// Start a new incremental generation
    pub async fn start(&self, file_path: PathBuf, estimated_chunks: usize) {
        let mut state = self.state.write().await;
        *state = GenerationState::InProgress {
            current_chunk: 0,
            total_chunks: estimated_chunks,
            file_path,
        };
        
        // Clear previous chunks
        self.chunks.write().await.clear();
        self.assembled.write().await.clear();
    }

    /// Add a chunk to the generation
    pub async fn add_chunk(&self, chunk: CodeChunk) -> Result<()> {
        // Update state
        {
            let mut state = self.state.write().await;
            if let GenerationState::InProgress { current_chunk, .. } = &mut *state {
                *current_chunk = chunk.index + 1;
            }
        }
        
        // Append to assembled content
        {
            let mut assembled = self.assembled.write().await;
            assembled.push_str(&chunk.content);
        }
        
        // Store chunk
        {
            let mut chunks = self.chunks.write().await;
            chunks.push(chunk);
        }
        
        Ok(())
    }

    /// Complete the generation
    pub async fn complete(&self) -> Result<String> {
        let mut state = self.state.write().await;
        let chunks = self.chunks.read().await;
        let assembled = self.assembled.read().await.clone();
        
        let files: Vec<PathBuf> = chunks
            .iter()
            .map(|c| c.file_path.clone())
            .collect();
        
        *state = GenerationState::Complete {
            total_chunks: chunks.len(),
            files,
        };
        
        Ok(assembled)
    }

    /// Fail the generation
    pub async fn fail(&self, error: String, failed_at_chunk: Option<usize>) {
        let mut state = self.state.write().await;
        *state = GenerationState::Failed {
            error,
            failed_at_chunk,
        };
    }

    /// Get all chunks
    pub async fn chunks(&self) -> Vec<CodeChunk> {
        self.chunks.read().await.clone()
    }

    /// Get assembled content
    pub async fn assembled(&self) -> String {
        self.assembled.read().await.clone()
    }

    /// Split content into chunks
    #[must_use]
    pub fn split_into_chunks(&self, content: &str, file_path: PathBuf) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let total_chars: usize = lines.iter().map(|l| l.len() + 1).sum();
        
        if total_chars <= self.config.max_chunk_size {
            // Content fits in a single chunk
            return vec![CodeChunk::new(
                0,
                1,
                file_path,
                content.to_string(),
                1,
                lines.len(),
            )];
        }
        
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut chunk_start_line = 1;
        let mut current_line = 1;
        
        for line in &lines {
            if current_chunk.len() + line.len() + 1 > self.config.max_chunk_size && !current_chunk.is_empty() {
                // Save current chunk
                let chunk_end_line = current_line - 1;
                chunks.push(CodeChunk::new(
                    chunks.len(),
                    0, // Will update total later
                    file_path.clone(),
                    current_chunk.clone(),
                    chunk_start_line,
                    chunk_end_line,
                ));
                
                // Start new chunk with context
                current_chunk.clear();
                chunk_start_line = current_line;
                
                // Add context from previous chunk
                if self.config.include_context && chunks.len() > 0 {
                    let prev_lines: Vec<&str> = lines
                        .iter()
                        .skip(chunk_start_line.saturating_sub(self.config.context_lines + 1))
                        .take(self.config.context_lines)
                        .copied()
                        .collect();
                    
                    for prev_line in prev_lines {
                        current_chunk.push_str(prev_line);
                        current_chunk.push('\n');
                    }
                }
            }
            
            current_chunk.push_str(line);
            current_chunk.push('\n');
            current_line += 1;
        }
        
        // Add final chunk
        if !current_chunk.is_empty() {
            chunks.push(CodeChunk::new(
                chunks.len(),
                chunks.len() + 1,
                file_path,
                current_chunk,
                chunk_start_line,
                lines.len(),
            ));
        }
        
        // Update total count in all chunks
        let total = chunks.len();
        for chunk in &mut chunks {
            chunk.total = total;
            chunk.is_final = chunk.index == total - 1;
        }
        
        chunks
    }
}

/// Chunk validator trait
pub trait ChunkValidator: Send + Sync {
    /// Validate a code chunk
    fn validate(&self, chunk: &CodeChunk, previous_chunks: &[CodeChunk]) -> Result<ValidationResult>;
}

/// Result of chunk validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the chunk is valid
    pub is_valid: bool,
    /// Validation messages
    pub messages: Vec<String>,
    /// Suggested fixes (if any)
    pub suggested_fixes: Vec<String>,
}

impl ValidationResult {
    /// Create a valid result
    #[must_use]
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            messages: Vec::new(),
            suggested_fixes: Vec::new(),
        }
    }

    /// Create an invalid result
    #[must_use]
    pub fn invalid(messages: Vec<String>) -> Self {
        Self {
            is_valid: false,
            messages,
            suggested_fixes: Vec::new(),
        }
    }

    /// Add a suggested fix
    #[must_use]
    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.suggested_fixes.push(fix.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_chunk_creation() {
        let chunk = CodeChunk::new(
            0,
            3,
            PathBuf::from("test.rs"),
            "fn main() {}".to_string(),
            1,
            1,
        );
        
        assert_eq!(chunk.index, 0);
        assert_eq!(chunk.total, 3);
        assert!(!chunk.is_final);
        assert_eq!(chunk.progress_percent(), 33);
    }

    #[test]
    fn test_code_chunk_final() {
        let chunk = CodeChunk::new(
            2,
            3,
            PathBuf::from("test.rs"),
            "}".to_string(),
            10,
            10,
        );
        
        assert!(chunk.is_final);
        assert_eq!(chunk.progress_percent(), 100);
    }

    #[tokio::test]
    async fn test_incremental_generator() {
        let config = IncrementalConfig::default();
        let generator = IncrementalGenerator::new(config);
        
        // Check initial state
        assert!(matches!(generator.state().await, GenerationState::Idle));
        
        // Start generation
        generator.start(PathBuf::from("test.rs"), 2).await;
        
        // Check state
        let state = generator.state().await;
        assert!(matches!(state, GenerationState::InProgress { .. }));
        
        // Add chunks
        let chunk1 = CodeChunk::new(0, 2, PathBuf::from("test.rs"), "fn main() {\n".to_string(), 1, 1);
        let chunk2 = CodeChunk::new(1, 2, PathBuf::from("test.rs"), "}\n".to_string(), 2, 2);
        
        generator.add_chunk(chunk1).await.unwrap();
        generator.add_chunk(chunk2).await.unwrap();
        
        // Complete
        let result = generator.complete().await.unwrap();
        assert!(result.contains("fn main()"));
        
        // Check final state
        let state = generator.state().await;
        assert!(matches!(state, GenerationState::Complete { .. }));
    }

    #[test]
    fn test_split_into_chunks_single() {
        let config = IncrementalConfig {
            max_chunk_size: 1000,
            ..Default::default()
        };
        let generator = IncrementalGenerator::new(config);
        
        let content = "fn main() {}\n";
        let chunks = generator.split_into_chunks(content, PathBuf::from("test.rs"));
        
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_final);
    }

    #[test]
    fn test_split_into_chunks_multiple() {
        let config = IncrementalConfig {
            max_chunk_size: 20,
            include_context: false,
            context_lines: 0,
            validate_chunks: false,
        };
        let generator = IncrementalGenerator::new(config);
        
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let chunks = generator.split_into_chunks(content, PathBuf::from("test.rs"));
        
        assert!(chunks.len() > 1);
        assert!(chunks.last().unwrap().is_final);
    }

    #[test]
    fn test_validation_result() {
        let valid = ValidationResult::valid();
        assert!(valid.is_valid);
        
        let invalid = ValidationResult::invalid(vec!["Error".to_string()]);
        assert!(!invalid.is_valid);
        assert_eq!(invalid.messages.len(), 1);
    }
}
