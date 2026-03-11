//! Simple hash-based embedder for testing and fallback

use crate::error::Result;
use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::EmbeddingGenerator;

#[derive(Clone)]
pub struct SimpleEmbedder {
    dimension: usize,
}

impl SimpleEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    fn hash_text(&self, text: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for SimpleEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

#[async_trait]
impl EmbeddingGenerator for SimpleEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut embedding = vec![0.0; self.dimension];

        if words.is_empty() {
            return Ok(embedding);
        }

        for word in words.iter() {
            let hash = self.hash_text(word);
            let idx = (hash as usize) % self.dimension;
            let value = ((hash % 1000) as f32 / 500.0) - 1.0;
            embedding[idx] += value;
        }

        let norm: f32 = embedding.iter().map(|x| x * x).sum();
        if norm > 0.0 {
            let scale = 1.0 / norm.sqrt();
            for val in embedding.iter_mut() {
                *val *= scale;
            }
        }

        Ok(embedding)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_embedder() {
        let embedder = SimpleEmbedder::new(128);
        assert_eq!(embedder.dimension(), 128);

        let embedding = embedder.embed("hello world").await.unwrap();
        assert_eq!(embedding.len(), 128);

        let norm: f32 = embedding.iter().map(|x| x * x).sum();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_empty_text() {
        let embedder = SimpleEmbedder::new(64);
        let embedding = embedder.embed("").await.unwrap();
        assert_eq!(embedding.len(), 64);
        assert!(embedding.iter().all(|&x| x == 0.0));
    }

    #[tokio::test]
    async fn test_similar_texts() {
        let embedder = SimpleEmbedder::new(128);

        let emb1 = embedder.embed("function process data").await.unwrap();
        let emb2 = embedder.embed("function process data").await.unwrap();

        assert_eq!(emb1, emb2);
    }

    #[tokio::test]
    async fn test_default_dimension() {
        let embedder = SimpleEmbedder::default();
        assert_eq!(embedder.dimension(), 384);
    }
}
