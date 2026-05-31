//! Persistent vector store abstraction layer.
//!
//! Provides a [`PersistentVectorStore`] trait for pluggable vector backends
//! with in-memory and LanceDB implementations.

use crate::error::{Error, Result};
use async_trait::async_trait;
use std::collections::HashMap;

#[cfg(feature = "lance-db")]
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub name: String,
    pub count: usize,
    pub dimension: usize,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    pub id: String,
    pub values: Vec<f32>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SimilarityResult {
    pub id: String,
    pub score: f32,
    pub metadata: HashMap<String, String>,
}

#[async_trait]
pub trait PersistentVectorStore: Send + Sync {
    async fn store_embeddings(
        &self,
        collection: &str,
        embeddings: Vec<StoredEmbedding>,
    ) -> Result<()>;
    async fn query_similar(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<SimilarityResult>>;
    async fn delete_collection(&self, collection: &str) -> Result<()>;
    async fn collection_info(&self, collection: &str) -> Result<CollectionInfo>;
    async fn health_check(&self) -> Result<HealthStatus>;
}

pub struct InMemoryVectorStore {
    collections: parking_lot::RwLock<HashMap<String, Vec<StoredEmbedding>>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            collections: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }
        dot / (mag_a * mag_b)
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PersistentVectorStore for InMemoryVectorStore {
    async fn store_embeddings(
        &self,
        collection: &str,
        embeddings: Vec<StoredEmbedding>,
    ) -> Result<()> {
        let mut collections = self.collections.write();
        let entries = collections.entry(collection.to_string()).or_default();

        let mut existing_ids = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if embeddings.iter().any(|new| new.id == e.id) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        existing_ids.sort_unstable_by(|a, b| b.cmp(a));
        for idx in existing_ids {
            entries.remove(idx);
        }

        entries.extend(embeddings);
        Ok(())
    }

    async fn query_similar(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<SimilarityResult>> {
        let collections = self.collections.read();
        let entries = collections
            .get(collection)
            .ok_or_else(|| Error::NotFound(format!("collection '{collection}' not found")))?;

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let mut scored: Vec<SimilarityResult> = entries
            .iter()
            .map(|e| {
                let score = Self::cosine_similarity(query, &e.values);
                SimilarityResult {
                    id: e.id.clone(),
                    score,
                    metadata: e.metadata.clone(),
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(k);
        Ok(scored)
    }

    async fn delete_collection(&self, collection: &str) -> Result<()> {
        let mut collections = self.collections.write();
        if collections.remove(collection).is_none() {
            return Err(Error::NotFound(format!(
                "collection '{collection}' not found"
            )));
        }
        Ok(())
    }

    async fn collection_info(&self, collection: &str) -> Result<CollectionInfo> {
        let collections = self.collections.read();
        let entries = collections
            .get(collection)
            .ok_or_else(|| Error::NotFound(format!("collection '{collection}' not found")))?;

        let dimension = entries.first().map(|e| e.values.len()).unwrap_or(0);

        Ok(CollectionInfo {
            name: collection.to_string(),
            count: entries.len(),
            dimension,
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus {
            ok: true,
            message: "in-memory store is healthy".to_string(),
        })
    }
}

#[cfg(feature = "lance-db")]
pub struct LanceDBVectorStore {
    _path: std::path::PathBuf,
}

#[cfg(feature = "lance-db")]
impl LanceDBVectorStore {
    pub async fn open(path: &Path) -> Result<Self> {
        let _path = path.to_path_buf();
        todo!(
            "LanceDBVectorStore::open — connect to LanceDB at {:?}",
            _path
        )
    }
}

#[cfg(feature = "lance-db")]
#[async_trait]
impl PersistentVectorStore for LanceDBVectorStore {
    async fn store_embeddings(
        &self,
        _collection: &str,
        _embeddings: Vec<StoredEmbedding>,
    ) -> Result<()> {
        todo!("LanceDBVectorStore::store_embeddings — insert embeddings into LanceDB table")
    }

    async fn query_similar(
        &self,
        _collection: &str,
        _query: &[f32],
        _k: usize,
    ) -> Result<Vec<SimilarityResult>> {
        todo!("LanceDBVectorStore::query_similar — execute vector search on LanceDB")
    }

    async fn delete_collection(&self, _collection: &str) -> Result<()> {
        todo!("LanceDBVectorStore::delete_collection — drop LanceDB table")
    }

    async fn collection_info(&self, _collection: &str) -> Result<CollectionInfo> {
        todo!("LanceDBVectorStore::collection_info — read table metadata from LanceDB")
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        todo!("LanceDBVectorStore::health_check — verify LanceDB connection is alive")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embedding(id: &str, values: Vec<f32>) -> StoredEmbedding {
        StoredEmbedding {
            id: id.to_string(),
            values,
            metadata: HashMap::new(),
        }
    }

    fn test_embedding_with_meta(
        id: &str,
        values: Vec<f32>,
        key: &str,
        val: &str,
    ) -> StoredEmbedding {
        let mut meta = HashMap::new();
        meta.insert(key.to_string(), val.to_string());
        StoredEmbedding {
            id: id.to_string(),
            values,
            metadata: meta,
        }
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let store = InMemoryVectorStore::new();
        let embeddings = vec![test_embedding("a", vec![1.0, 0.0, 0.0])];
        store.store_embeddings("col1", embeddings).await.unwrap();

        let results = store
            .query_similar("col1", &[1.0, 0.0, 0.0], 5)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }

    #[tokio::test]
    async fn test_store_multiple_and_query() {
        let store = InMemoryVectorStore::new();
        let embeddings = vec![
            test_embedding("x", vec![1.0, 0.0, 0.0]),
            test_embedding("y", vec![0.0, 1.0, 0.0]),
            test_embedding("z", vec![0.0, 0.0, 1.0]),
        ];
        store.store_embeddings("col", embeddings).await.unwrap();

        let results = store
            .query_similar("col", &[0.9, 0.1, 0.0], 2)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "x");
    }

    #[tokio::test]
    async fn test_query_nonexistent_collection() {
        let store = InMemoryVectorStore::new();
        let result = store.query_similar("missing", &[1.0, 0.0], 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_collection() {
        let store = InMemoryVectorStore::new();
        store
            .store_embeddings("col", vec![test_embedding("a", vec![1.0, 0.0])])
            .await
            .unwrap();
        store.delete_collection("col").await.unwrap();

        let result = store.collection_info("col").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_collection() {
        let store = InMemoryVectorStore::new();
        let result = store.delete_collection("nope").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_collection_info() {
        let store = InMemoryVectorStore::new();
        store
            .store_embeddings(
                "info",
                vec![
                    test_embedding("a", vec![1.0, 0.0]),
                    test_embedding("b", vec![0.0, 1.0]),
                ],
            )
            .await
            .unwrap();

        let info = store.collection_info("info").await.unwrap();
        assert_eq!(info.name, "info");
        assert_eq!(info.count, 2);
        assert_eq!(info.dimension, 2);
    }

    #[tokio::test]
    async fn test_collection_info_nonexistent() {
        let store = InMemoryVectorStore::new();
        let result = store.collection_info("nope").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let store = InMemoryVectorStore::new();
        let status = store.health_check().await.unwrap();
        assert!(status.ok);
        assert!(!status.message.is_empty());
    }

    #[tokio::test]
    async fn test_empty_store_queries() {
        let store = InMemoryVectorStore::new();
        store.store_embeddings("empty", Vec::new()).await.unwrap();

        let results = store.query_similar("empty", &[1.0, 0.0], 5).await.unwrap();
        assert!(results.is_empty());

        let info = store.collection_info("empty").await.unwrap();
        assert_eq!(info.count, 0);
        assert_eq!(info.dimension, 0);
    }

    #[tokio::test]
    async fn test_duplicate_handling_overwrites() {
        let store = InMemoryVectorStore::new();

        store
            .store_embeddings(
                "dup",
                vec![test_embedding_with_meta("id1", vec![1.0, 0.0], "k", "v1")],
            )
            .await
            .unwrap();

        store
            .store_embeddings(
                "dup",
                vec![test_embedding_with_meta("id1", vec![0.0, 1.0], "k", "v2")],
            )
            .await
            .unwrap();

        let info = store.collection_info("dup").await.unwrap();
        assert_eq!(info.count, 1);

        let results = store.query_similar("dup", &[0.0, 1.0], 1).await.unwrap();
        assert_eq!(results[0].id, "id1");
        assert_eq!(results[0].metadata.get("k").unwrap(), "v2");
    }

    #[tokio::test]
    async fn test_multiple_collections_isolated() {
        let store = InMemoryVectorStore::new();

        store
            .store_embeddings("alpha", vec![test_embedding("a", vec![1.0, 0.0])])
            .await
            .unwrap();
        store
            .store_embeddings("beta", vec![test_embedding("b", vec![0.0, 1.0])])
            .await
            .unwrap();

        assert_eq!(store.collection_info("alpha").await.unwrap().count, 1);
        assert_eq!(store.collection_info("beta").await.unwrap().count, 1);

        store.delete_collection("alpha").await.unwrap();

        let err = store.collection_info("alpha").await;
        assert!(err.is_err());
        assert_eq!(store.collection_info("beta").await.unwrap().count, 1);
    }

    #[tokio::test]
    async fn test_store_empty_batch_is_ok() {
        let store = InMemoryVectorStore::new();
        store.store_embeddings("col", Vec::new()).await.unwrap();
    }

    #[tokio::test]
    async fn test_query_limit_truncates() {
        let store = InMemoryVectorStore::new();
        let embeddings: Vec<StoredEmbedding> = (0..10)
            .map(|i| test_embedding(&format!("e{i}"), vec![i as f32 / 10.0, 0.0]))
            .collect();
        store.store_embeddings("col", embeddings).await.unwrap();

        let results = store.query_similar("col", &[1.0, 0.0], 3).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_metadata_preserved_in_query() {
        let store = InMemoryVectorStore::new();
        store
            .store_embeddings(
                "meta",
                vec![test_embedding_with_meta(
                    "m1",
                    vec![1.0, 0.0],
                    "file",
                    "main.rs",
                )],
            )
            .await
            .unwrap();

        let results = store.query_similar("meta", &[1.0, 0.0], 1).await.unwrap();
        assert_eq!(results[0].metadata.get("file").unwrap(), "main.rs");
    }

    #[tokio::test]
    async fn test_dimension_mismatch_in_query() {
        let store = InMemoryVectorStore::new();
        store
            .store_embeddings("col", vec![test_embedding("a", vec![1.0, 0.0, 0.0])])
            .await
            .unwrap();

        let results = store.query_similar("col", &[1.0], 1).await.unwrap();
        assert_eq!(results[0].score, 0.0);
    }
}
