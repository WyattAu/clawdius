//! Vector storage for semantic search using LanceDB

use crate::error::{Error, Result};
use arrow_array::{
    ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use futures::StreamExt;
use lancedb::{
    connect,
    query::{ExecutableQuery, QueryBase},
    Connection,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_TABLE_NAME: &str = "code_embeddings";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub values: Vec<f32>,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: HashMap<String, String>,
}

pub struct VectorStore {
    db: Connection,
    table_name: String,
    dimension: usize,
}

impl VectorStore {
    pub async fn open(path: &Path, dimension: usize) -> Result<Self> {
        Self::open_with_table(path, dimension, DEFAULT_TABLE_NAME).await
    }

    pub async fn open_with_table(path: &Path, dimension: usize, table_name: &str) -> Result<Self> {
        let uri = path.to_string_lossy().to_string();
        let db = connect(&uri)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to connect to LanceDB: {}", e)))?;

        let store = Self {
            db,
            table_name: table_name.to_string(),
            dimension,
        };

        Ok(store)
    }

    pub async fn create_table_if_not_exists(&self) -> Result<()> {
        let tables = self
            .db
            .table_names()
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to list tables: {}", e)))?;

        if !tables.contains(&self.table_name) {
            let schema = self.create_schema();
            let empty_batch = self.create_empty_batch(&schema)?;
            let batches = RecordBatchIterator::new(vec![Ok(empty_batch)], schema);

            self.db
                .create_table(&self.table_name, Box::new(batches))
                .execute()
                .await
                .map_err(|e| Error::Generic(format!("Failed to create table: {}", e)))?;
        }

        Ok(())
    }

    fn create_schema(&self) -> Arc<Schema> {
        let fields = vec![
            Field::new("id", DataType::Utf8, false),
            Field::new(
                "embedding",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    self.dimension as i32,
                ),
                false,
            ),
            Field::new("metadata", DataType::Utf8, false),
        ];
        Arc::new(Schema::new(fields))
    }

    fn create_empty_batch(&self, schema: &Arc<Schema>) -> Result<RecordBatch> {
        let id_array: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));

        let values = Float32Array::from(Vec::<f32>::new());
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let embedding_array: ArrayRef = Arc::new(
            FixedSizeListArray::try_new(field, self.dimension as i32, Arc::new(values), None)
                .map_err(|e| {
                    Error::Generic(format!("Failed to create empty embedding array: {}", e))
                })?,
        );
        let metadata_array: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));

        RecordBatch::try_new(
            schema.clone(),
            vec![id_array, embedding_array, metadata_array],
        )
        .map_err(|e| Error::Generic(format!("Failed to create empty batch: {}", e)))
    }

    pub async fn insert(&self, entries: Vec<VectorEntry>) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let schema = self.create_schema();

        let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        let embeddings: Vec<f32> = entries.iter().flat_map(|e| e.embedding.clone()).collect();
        let metadata: Vec<String> = entries
            .iter()
            .map(|e| serde_json::to_string(&e.metadata).unwrap_or_else(|_| "{}".to_string()))
            .collect();

        let id_array: ArrayRef = Arc::new(StringArray::from(ids));

        let values = Float32Array::from(embeddings);
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let embedding_array: ArrayRef = Arc::new(
            FixedSizeListArray::try_new(field, self.dimension as i32, Arc::new(values), None)
                .map_err(|e| Error::Generic(format!("Failed to create embedding array: {}", e)))?,
        );
        let metadata_array: ArrayRef = Arc::new(StringArray::from(metadata));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![id_array, embedding_array, metadata_array],
        )
        .map_err(|e| Error::Generic(format!("Failed to create batch: {}", e)))?;

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to open table: {}", e)))?;

        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(Box::new(batches))
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to insert entries: {}", e)))?;

        Ok(())
    }

    pub async fn search(&self, query: Vec<f32>, k: usize) -> Result<Vec<SearchResult>> {
        if query.len() != self.dimension {
            return Err(Error::Generic(format!(
                "Query dimension {} does not match store dimension {}",
                query.len(),
                self.dimension
            )));
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to open table: {}", e)))?;

        let mut results = table
            .vector_search(query)
            .map_err(|e| Error::Generic(format!("Failed to create vector search: {}", e)))?
            .limit(k)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to execute search: {}", e)))?;

        let mut search_results = Vec::new();
        while let Some(batch) = results.next().await {
            let batch = batch.map_err(|e| Error::Generic(format!("Failed to get batch: {}", e)))?;

            let id_column = batch
                .column_by_name("id")
                .ok_or_else(|| Error::Generic("Missing id column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::Generic("Invalid id column type".to_string()))?;

            let metadata_column = batch
                .column_by_name("metadata")
                .ok_or_else(|| Error::Generic("Missing metadata column".to_string()))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::Generic("Invalid metadata column type".to_string()))?;

            let distance_column = batch
                .column_by_name("_distance")
                .ok_or_else(|| Error::Generic("Missing distance column".to_string()))?
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| Error::Generic("Invalid distance column type".to_string()))?;

            for i in 0..batch.num_rows() {
                let id = id_column.value(i).to_string();
                let metadata_str = metadata_column.value(i);
                let distance = distance_column.value(i);
                let metadata: HashMap<String, String> =
                    serde_json::from_str(metadata_str).unwrap_or_else(|_| HashMap::new());

                search_results.push(SearchResult {
                    id,
                    score: 1.0 / (1.0 + distance),
                    metadata,
                });
            }
        }

        Ok(search_results)
    }

    pub async fn delete(&self, ids: &[&str]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to open table: {}", e)))?;

        let filter = ids
            .iter()
            .map(|id| format!("id = \"{}\"", id))
            .collect::<Vec<_>>()
            .join(" OR ");

        table
            .delete(&filter)
            .await
            .map_err(|e| Error::Generic(format!("Failed to delete entries: {}", e)))?;

        Ok(())
    }

    pub async fn count(&self) -> Result<usize> {
        let table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| Error::Generic(format!("Failed to open table: {}", e)))?;

        let count = table
            .count_rows(None)
            .await
            .map_err(|e| Error::Generic(format!("Failed to count rows: {}", e)))?;

        Ok(count)
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_vector_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = VectorStore::open(temp_dir.path(), 128).await.unwrap();
        assert_eq!(store.dimension(), 128);
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let store = VectorStore::open(temp_dir.path(), 4).await.unwrap();
        store.create_table_if_not_exists().await.unwrap();

        let entry = VectorEntry {
            id: "test1".to_string(),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            metadata: {
                let mut m = HashMap::new();
                m.insert("file".to_string(), "test.rs".to_string());
                m
            },
        };

        store.insert(vec![entry]).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        let query = vec![1.0, 0.1, 0.0, 0.0];
        let results = store.search(query, 5).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "test1");
    }

    #[tokio::test]
    async fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = VectorStore::open(temp_dir.path(), 4).await.unwrap();
        store.create_table_if_not_exists().await.unwrap();

        let entries = vec![
            VectorEntry {
                id: "id1".to_string(),
                embedding: vec![1.0, 0.0, 0.0, 0.0],
                metadata: HashMap::new(),
            },
            VectorEntry {
                id: "id2".to_string(),
                embedding: vec![0.0, 1.0, 0.0, 0.0],
                metadata: HashMap::new(),
            },
        ];

        store.insert(entries).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        store.delete(&["id1"]).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let store = VectorStore::open(temp_dir.path(), 4).await.unwrap();
        store.create_table_if_not_exists().await.unwrap();

        let entries = vec![
            VectorEntry {
                id: "func1".to_string(),
                embedding: vec![1.0, 0.0, 0.0, 0.0],
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "process_data".to_string());
                    m
                },
            },
            VectorEntry {
                id: "func2".to_string(),
                embedding: vec![0.0, 1.0, 0.0, 0.0],
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "parse_input".to_string());
                    m
                },
            },
            VectorEntry {
                id: "func3".to_string(),
                embedding: vec![0.5, 0.5, 0.0, 0.0],
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "validate_data".to_string());
                    m
                },
            },
        ];

        store.insert(entries).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 3);

        let query = vec![0.9, 0.1, 0.0, 0.0];
        let results = store.search(query, 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "func1");
    }
}
