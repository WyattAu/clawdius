//! Workspace indexer for multi-file context understanding

use crate::error::Result;
use crate::graph_rag::ast::{FileInfo, Symbol};
use crate::graph_rag::languages::LanguageKind;
use crate::graph_rag::{
    parser::CodeParser, EmbeddingGenerator, GraphStore, SimpleEmbedder, VectorEntry, VectorStore,
};
use futures::channel::mpsc::{channel, Sender};
use futures::StreamExt;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

const EMBEDDING_DIMENSION: usize = 384;
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub symbols_found: usize,
    pub references_found: usize,
    pub embeddings_created: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

pub struct WorkspaceIndexer {
    graph_store: Arc<GraphStore>,
    vector_store: Arc<RwLock<VectorStore>>,
    parser: CodeParser,
    embedder: SimpleEmbedder,
    watcher: Option<RecommendedWatcher>,
    watch_sender: Option<Sender<PathBuf>>,
}

impl WorkspaceIndexer {
    #[allow(clippy::arc_with_non_send_sync)]
    pub async fn new(graph_path: &Path, vector_path: &Path) -> Result<Self> {
        let graph_store = Arc::new(GraphStore::open(graph_path)?);
        let vector_store = VectorStore::open(vector_path, EMBEDDING_DIMENSION).await?;
        vector_store.create_table_if_not_exists().await?;
        let vector_store = Arc::new(RwLock::new(vector_store));
        let parser = CodeParser::new()?;
        let embedder = SimpleEmbedder::new(EMBEDDING_DIMENSION);

        Ok(Self {
            graph_store,
            vector_store,
            parser,
            embedder,
            watcher: None,
            watch_sender: None,
        })
    }

    pub async fn index_workspace(&mut self, root: &Path) -> Result<IndexStats> {
        let start = std::time::Instant::now();
        let mut stats = IndexStats {
            files_indexed: 0,
            symbols_found: 0,
            references_found: 0,
            embeddings_created: 0,
            errors: Vec::new(),
            duration_ms: 0,
        };

        let supported_extensions = ["rs", "py", "js", "ts", "tsx", "go"];

        let files: Vec<PathBuf> = walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| supported_extensions.contains(&ext))
            })
            .filter(|e| {
                e.metadata()
                    .map(|m| m.len() < MAX_FILE_SIZE)
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        for file_path in files {
            match self.index_file(&file_path).await {
                Ok((symbols, refs, embeddings)) => {
                    stats.files_indexed += 1;
                    stats.symbols_found += symbols;
                    stats.references_found += refs;
                    stats.embeddings_created += embeddings;
                },
                Err(e) => {
                    stats.errors.push(format!("{}: {}", file_path.display(), e));
                },
            }
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    pub async fn index_file(&self, path: &Path) -> Result<(usize, usize, usize)> {
        let content = tokio::fs::read_to_string(path).await?;
        let hash = blake3::hash(content.as_bytes()).to_hex().to_string();

        let metadata = tokio::fs::metadata(path).await?;
        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);

        let language = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(LanguageKind::from_extension);

        let file_info = FileInfo {
            path: path.to_string_lossy().to_string(),
            hash: hash.clone(),
            language: language.map(|l| l.as_str().to_string()),
            last_modified,
        };

        let file_id = self.graph_store.insert_file(&file_info)?;

        self.graph_store.delete_symbols_for_file(file_id)?;
        self.graph_store.delete_symbol_refs_for_file(file_id)?;

        let lang = match language {
            Some(l) => l,
            None => return Ok((0, 0, 0)),
        };

        let tree = self.parser.parse(&content, lang)?;
        let symbols = self.parser.extract_symbols(&tree, &content, file_id, lang);
        let symbol_count = symbols.len();

        let mut symbol_ids = Vec::new();
        for symbol in symbols {
            let symbol_id = self.graph_store.insert_symbol(&symbol)?;
            symbol_ids.push((symbol_id, symbol));
        }

        let references = self.parser.extract_references(&tree, &content, file_id);
        let ref_count = references.len();

        for reference in references {
            self.graph_store.insert_reference(&reference)?;
        }

        let mut vector_entries = Vec::new();
        for (symbol_id, symbol) in &symbol_ids {
            let text = self.symbol_to_text(symbol, &content);
            let embedding = self.embedder.embed(&text).await?;

            let mut metadata = HashMap::new();
            metadata.insert("path".to_string(), path.to_string_lossy().to_string());
            metadata.insert("name".to_string(), symbol.name.clone());
            metadata.insert("kind".to_string(), symbol.kind.as_str().to_string());
            metadata.insert("symbol_id".to_string(), symbol_id.to_string());

            vector_entries.push(VectorEntry {
                id: format!("symbol_{symbol_id}"),
                embedding,
                metadata,
            });
        }

        let embedding_count = vector_entries.len();

        if !vector_entries.is_empty() {
            let store = self.vector_store.read().await;
            let ids: Vec<&str> = vector_entries.iter().map(|e| e.id.as_str()).collect();
            drop(store);

            {
                let store = self.vector_store.write().await;
                store.delete(&ids).await?;
                store.insert(vector_entries).await?;
            }
        }

        Ok((symbol_count, ref_count, embedding_count))
    }

    fn symbol_to_text(&self, symbol: &Symbol, source: &str) -> String {
        let mut text = format!("{} {} ", symbol.kind.as_str(), symbol.name);

        if let Some(ref sig) = symbol.signature {
            text.push_str(sig);
            text.push(' ');
        }

        if let Some(ref doc) = symbol.doc_comment {
            text.push_str(doc);
            text.push(' ');
        }

        let lines: Vec<&str> = source.lines().collect();
        let start = (symbol.start_line as usize).saturating_sub(1);
        let end = (symbol.end_line as usize).min(lines.len());

        if start < end {
            let context: Vec<&str> = lines[start..end].iter().map(|s| s.trim()).collect();
            text.push_str(&context.join(" "));
        }

        text
    }

    pub fn watch(&mut self, root: &Path) -> Result<()> {
        let (mut tx, mut rx) = channel::<PathBuf>(100);
        self.watch_sender = Some(tx.clone());

        let graph_store = self.graph_store.clone();
        let vector_store = self.vector_store.clone();
        let parser = self.parser.clone();
        let embedder = self.embedder.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if let EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) =
                        event.kind
                    {
                        for path in event.paths {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if ["rs", "py", "js", "ts", "tsx", "go"].contains(&ext) {
                                    let _ = tx.try_send(path);
                                }
                            }
                        }
                    }
                }
            })?;

        watcher.watch(root, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);

        let _root = root.to_path_buf();
        tokio::task::spawn_local(async move {
            while let Some(path) = rx.next().await {
                let indexer = FileIndexer {
                    graph_store: graph_store.clone(),
                    vector_store: vector_store.clone(),
                    parser: parser.clone(),
                    embedder: embedder.clone(),
                };

                if let Err(e) = indexer.index_file(&path).await {
                    tracing::warn!("Failed to re-index {}: {}", path.display(), e);
                } else {
                    tracing::info!("Re-indexed {}", path.display());
                }
            }
        });

        Ok(())
    }

    #[must_use]
    pub fn graph_store(&self) -> &GraphStore {
        &self.graph_store
    }

    #[must_use]
    pub fn graph_store_arc(&self) -> Arc<GraphStore> {
        self.graph_store.clone()
    }

    #[must_use]
    pub fn vector_store_arc(&self) -> Arc<RwLock<VectorStore>> {
        self.vector_store.clone()
    }

    pub async fn vector_store(&self) -> tokio::sync::RwLockReadGuard<'_, VectorStore> {
        self.vector_store.read().await
    }
}

struct FileIndexer {
    graph_store: Arc<GraphStore>,
    vector_store: Arc<RwLock<VectorStore>>,
    parser: CodeParser,
    embedder: SimpleEmbedder,
}

impl FileIndexer {
    async fn index_file(&self, path: &Path) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await?;
        let hash = blake3::hash(content.as_bytes()).to_hex().to_string();

        let metadata = tokio::fs::metadata(path).await?;
        let last_modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);

        let language = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(LanguageKind::from_extension);

        let file_info = FileInfo {
            path: path.to_string_lossy().to_string(),
            hash,
            language: language.map(|l| l.as_str().to_string()),
            last_modified,
        };

        let file_id = self.graph_store.insert_file(&file_info)?;

        self.graph_store.delete_symbols_for_file(file_id)?;
        self.graph_store.delete_symbol_refs_for_file(file_id)?;

        let lang = match language {
            Some(l) => l,
            None => return Ok(()),
        };

        let tree = self.parser.parse(&content, lang)?;
        let symbols = self.parser.extract_symbols(&tree, &content, file_id, lang);

        let mut vector_entries = Vec::new();
        for symbol in symbols {
            let symbol_id = self.graph_store.insert_symbol(&symbol)?;

            let text = self.symbol_to_text(&symbol, &content);
            let embedding = self.embedder.embed(&text).await?;

            let mut metadata = HashMap::new();
            metadata.insert("path".to_string(), path.to_string_lossy().to_string());
            metadata.insert("name".to_string(), symbol.name.clone());
            metadata.insert("kind".to_string(), symbol.kind.as_str().to_string());
            metadata.insert("symbol_id".to_string(), symbol_id.to_string());

            vector_entries.push(VectorEntry {
                id: format!("symbol_{symbol_id}"),
                embedding,
                metadata,
            });
        }

        let references = self.parser.extract_references(&tree, &content, file_id);
        for reference in references {
            self.graph_store.insert_reference(&reference)?;
        }

        if !vector_entries.is_empty() {
            let ids: Vec<&str> = vector_entries.iter().map(|e| e.id.as_str()).collect();
            let store = self.vector_store.write().await;
            store.delete(&ids).await?;
            store.insert(vector_entries).await?;
        }

        Ok(())
    }

    fn symbol_to_text(&self, symbol: &Symbol, source: &str) -> String {
        let mut text = format!("{} {} ", symbol.kind.as_str(), symbol.name);

        if let Some(ref sig) = symbol.signature {
            text.push_str(sig);
            text.push(' ');
        }

        if let Some(ref doc) = symbol.doc_comment {
            text.push_str(doc);
            text.push(' ');
        }

        let lines: Vec<&str> = source.lines().collect();
        let start = (symbol.start_line as usize).saturating_sub(1);
        let end = (symbol.end_line as usize).min(lines.len());

        if start < end {
            let context: Vec<&str> = lines[start..end].iter().map(|s| s.trim()).collect();
            text.push_str(&context.join(" "));
        }

        text
    }
}

impl LanguageKind {
    fn from_extension(ext: &str) -> Option<LanguageKind> {
        match ext {
            "rs" => Some(LanguageKind::Rust),
            "py" => Some(LanguageKind::Python),
            "js" => Some(LanguageKind::JavaScript),
            "ts" => Some(LanguageKind::TypeScript),
            "tsx" => Some(LanguageKind::TypeScriptJsx),
            "go" => Some(LanguageKind::Go),
            _ => None,
        }
    }
}
