//! Embedding generators for semantic search
//!
//! This module provides different embedding strategies for converting text into
//! dense vector representations for semantic search.
//!
//! # Available Embedders
//!
//! - **`SimpleEmbedder`**: Hash-based embedder for testing and fallback (always available)
//! - **`SentenceEmbedder`**: Real sentence transformer embeddings using BERT models (requires `embeddings` feature)
//!
//! # Configuration
//!
//! ```toml
//! [embedding]
//! type = "sentence_transformers"
//! model = "sentence-transformers/all-MiniLM-L6-v2"
//! model_path = ".clawdius/models/"
//! batch_size = 32
//! ```

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[cfg(feature = "embeddings")]
pub mod real;
pub mod simple;

#[cfg(feature = "embeddings")]
pub use real::SentenceEmbedder;
pub use simple::SimpleEmbedder;

#[async_trait]
pub trait EmbeddingGenerator: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimension(&self) -> usize;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderType {
    Simple,
    #[cfg(feature = "embeddings")]
    SentenceTransformers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConfig {
    #[serde(rename = "type")]
    pub embedder_type: EmbedderType,
    pub model: Option<String>,
    pub model_path: Option<String>,
    pub batch_size: Option<usize>,
}

impl Default for EmbedderConfig {
    fn default() -> Self {
        Self {
            embedder_type: EmbedderType::Simple,
            model: None,
            model_path: None,
            batch_size: Some(32),
        }
    }
}

pub fn create_embedder(config: &EmbedderConfig) -> Result<Box<dyn EmbeddingGenerator>> {
    match &config.embedder_type {
        EmbedderType::Simple => {
            let dimension = 384;
            Ok(Box::new(SimpleEmbedder::new(dimension)))
        },
        #[cfg(feature = "embeddings")]
        EmbedderType::SentenceTransformers => {
            let model_name = config
                .model
                .as_deref()
                .unwrap_or("sentence-transformers/all-MiniLM-L6-v2");

            let model_path = config.model_path.as_deref().map(std::path::Path::new);

            Ok(Box::new(SentenceEmbedder::new(model_name, model_path)?))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmbedderConfig::default();
        assert_eq!(config.embedder_type, EmbedderType::Simple);
        assert!(config.model.is_none());
    }

    #[test]
    fn test_create_simple_embedder() {
        let config = EmbedderConfig::default();
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 384);
    }

    #[tokio::test]
    async fn test_simple_embedder_through_factory() {
        let config = EmbedderConfig::default();
        let embedder = create_embedder(&config).unwrap();
        let embedding = embedder.embed("test text").await.unwrap();
        assert_eq!(embedding.len(), 384);
    }
}
