//! Hybrid search combining vector and symbolic search

use crate::error::Result;
use crate::graph_rag::ast::Symbol;
use crate::graph_rag::embedding::EmbeddingGenerator;
use crate::graph_rag::store::GraphStore;
use crate::graph_rag::vector::{SearchResult, VectorStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ResultSource {
    VectorSearch,
    SymbolicSearch,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridResult {
    pub symbol: Symbol,
    pub semantic_score: f32,
    pub source: ResultSource,
}

pub struct HybridSearcher {
    vector_store: Arc<VectorStore>,
    graph_store: Arc<GraphStore>,
    embedder: Arc<dyn EmbeddingGenerator>,
}

impl HybridSearcher {
    pub fn new(
        vector_store: Arc<VectorStore>,
        graph_store: Arc<GraphStore>,
        embedder: Arc<dyn EmbeddingGenerator>,
    ) -> Self {
        Self {
            vector_store,
            graph_store,
            embedder,
        }
    }

    pub async fn open(
        vector_path: &Path,
        graph_path: &Path,
        dimension: usize,
        embedder: Arc<dyn EmbeddingGenerator>,
    ) -> Result<Self> {
        let vector_store = Arc::new(VectorStore::open(vector_path, dimension).await?);
        let graph_store = Arc::new(GraphStore::open(graph_path)?);

        Ok(Self::new(vector_store, graph_store, embedder))
    }

    pub async fn search(&self, query: &str, k: usize) -> Result<Vec<HybridResult>> {
        let embedding = self.embedder.embed(query).await?;

        let vector_results = self.vector_store.search(embedding, k).await?;

        let symbolic_results = self.graph_store.search_symbols(query)?;

        Ok(self.fuse_results(vector_results, symbolic_results))
    }

    fn fuse_results(
        &self,
        vector_results: Vec<SearchResult>,
        symbolic_results: Vec<Symbol>,
    ) -> Vec<HybridResult> {
        let mut vector_map: HashMap<String, (f32, HashMap<String, String>)> = HashMap::new();

        for result in vector_results {
            vector_map.insert(result.id.clone(), (result.score, result.metadata));
        }

        let mut hybrid_results = Vec::new();

        for symbol in symbolic_results {
            let symbol_id = symbol.id.map(|id| id.to_string()).unwrap_or_default();
            let (semantic_score, source) = if vector_map.contains_key(&symbol_id) {
                let (score, _) = vector_map.remove(&symbol_id).unwrap();
                (score, ResultSource::Hybrid)
            } else {
                (0.5, ResultSource::SymbolicSearch)
            };

            hybrid_results.push(HybridResult {
                symbol,
                semantic_score,
                source,
            });
        }

        for (id, (score, metadata)) in vector_map {
            if let Some(file_path) = metadata.get("file") {
                if let Some(file_id) = self.graph_store.get_file_id(file_path).ok().flatten() {
                    if let Some(symbols) = self.graph_store.find_symbols_in_file(file_id).ok() {
                        if let Some(symbol) = symbols
                            .into_iter()
                            .find(|s| s.id.map(|sid| sid.to_string()).unwrap_or_default() == id)
                        {
                            hybrid_results.push(HybridResult {
                                symbol,
                                semantic_score: score,
                                source: ResultSource::VectorSearch,
                            });
                            continue;
                        }
                    }
                }
            }

            let placeholder_symbol = Symbol {
                id: None,
                file_id: 0,
                name: metadata.get("name").cloned().unwrap_or_else(|| id.clone()),
                kind: crate::graph_rag::ast::SymbolKind::Other("unknown".to_string()),
                signature: None,
                doc_comment: None,
                start_line: 0,
                end_line: 0,
                start_col: 0,
                end_col: 0,
            };

            hybrid_results.push(HybridResult {
                symbol: placeholder_symbol,
                semantic_score: score,
                source: ResultSource::VectorSearch,
            });
        }

        hybrid_results.sort_by(|a, b| {
            b.semantic_score
                .partial_cmp(&a.semantic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        hybrid_results
    }

    pub async fn index_symbol(&self, symbol: &Symbol, context: &str) -> Result<()> {
        use crate::graph_rag::vector::VectorEntry;

        let embedding = self.embedder.embed(context).await?;

        let id = symbol.id.map(|id| id.to_string()).unwrap_or_default();

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), symbol.name.clone());
        if let Some(ref signature) = symbol.signature {
            metadata.insert("signature".to_string(), signature.clone());
        }
        if let Some(ref doc) = symbol.doc_comment {
            metadata.insert("doc".to_string(), doc.clone());
        }

        let entry = VectorEntry {
            id,
            embedding,
            metadata,
        };

        self.vector_store.insert(vec![entry]).await?;

        Ok(())
    }

    pub fn vector_store(&self) -> &VectorStore {
        &self.vector_store
    }

    pub fn graph_store(&self) -> &GraphStore {
        &self.graph_store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_rag::ast::{FileInfo, SymbolKind};
    use crate::graph_rag::embedding::SimpleEmbedder;
    use tempfile::TempDir;

    async fn create_test_searcher() -> HybridSearcher {
        let temp_dir = TempDir::new().unwrap();
        let vector_path = temp_dir.path().join("vectors");
        let graph_path = temp_dir.path().join("graph.db");
        let embedder = Arc::new(SimpleEmbedder::new(128));

        HybridSearcher::open(&vector_path, &graph_path, 128, embedder)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_hybrid_searcher_creation() {
        let searcher = create_test_searcher().await;
        assert_eq!(searcher.vector_store().dimension(), 128);
    }

    #[tokio::test]
    async fn test_symbolic_search() {
        let searcher = create_test_searcher().await;
        searcher
            .vector_store()
            .create_table_if_not_exists()
            .await
            .unwrap();

        let file = FileInfo {
            path: "test.rs".to_string(),
            hash: "abc123".to_string(),
            language: Some("rust".to_string()),
            last_modified: None,
        };
        let file_id = searcher.graph_store().insert_file(&file).unwrap();

        let symbol = Symbol {
            id: None,
            file_id,
            name: "process_data".to_string(),
            kind: SymbolKind::Function,
            signature: Some("fn process_data(input: &str) -> String".to_string()),
            doc_comment: Some("Process the input data".to_string()),
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        searcher.graph_store().insert_symbol(&symbol).unwrap();

        let results = searcher.search("process", 10).await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.symbol.name == "process_data"));
    }

    #[tokio::test]
    async fn test_index_and_search() {
        let searcher = create_test_searcher().await;

        searcher
            .vector_store()
            .create_table_if_not_exists()
            .await
            .unwrap();

        let file = FileInfo {
            path: "src/lib.rs".to_string(),
            hash: "hash123".to_string(),
            language: Some("rust".to_string()),
            last_modified: None,
        };
        let file_id = searcher.graph_store().insert_file(&file).unwrap();

        let symbol = Symbol {
            id: None,
            file_id,
            name: "parse_input".to_string(),
            kind: SymbolKind::Function,
            signature: Some("fn parse_input(data: &str) -> Result<Input>".to_string()),
            doc_comment: Some("Parse the input data".to_string()),
            start_line: 10,
            end_line: 20,
            start_col: 1,
            end_col: 1,
        };
        let symbol_id = searcher.graph_store().insert_symbol(&symbol).unwrap();

        let mut indexed_symbol = symbol.clone();
        indexed_symbol.id = Some(symbol_id);

        searcher
            .index_symbol(&indexed_symbol, "parse input data function")
            .await
            .unwrap();

        let results = searcher.search("parse input", 10).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_fuse_results() {
        let searcher = create_test_searcher().await;

        let vector_results = vec![SearchResult {
            id: "1".to_string(),
            score: 0.9,
            metadata: {
                let mut m = HashMap::new();
                m.insert("name".to_string(), "test".to_string());
                m
            },
        }];

        let file = FileInfo {
            path: "test.rs".to_string(),
            hash: "hash".to_string(),
            language: None,
            last_modified: None,
        };
        let file_id = searcher.graph_store().insert_file(&file).unwrap();

        let mut symbol = Symbol {
            id: None,
            file_id,
            name: "test".to_string(),
            kind: SymbolKind::Function,
            signature: None,
            doc_comment: None,
            start_line: 1,
            end_line: 5,
            start_col: 1,
            end_col: 1,
        };
        let symbol_id = searcher.graph_store().insert_symbol(&symbol).unwrap();
        symbol.id = Some(symbol_id);

        let symbolic_results = vec![symbol];

        let fused = searcher.fuse_results(vector_results, symbolic_results);

        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].source, ResultSource::Hybrid);
        assert!((fused[0].semantic_score - 0.9).abs() < 0.01);
    }
}
