//! Context aggregator for multi-file context understanding

use crate::context::ContextItem;
use crate::error::Result;
use crate::graph_rag::ast::Symbol;
use crate::graph_rag::{EmbeddingGenerator, GraphStore, VectorStore};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const DEFAULT_MAX_TOKENS: usize = 50_000;
const VECTOR_SEARCH_LIMIT: usize = 20;
#[allow(dead_code)]
const SYMBOL_EXPANSION_DEPTH: usize = 2;

#[derive(Debug, Clone)]
pub struct AggregatedContext {
    pub files: Vec<FileContext>,
    pub symbols: Vec<SymbolContext>,
    pub total_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct FileContext {
    pub path: PathBuf,
    pub language: Option<String>,
    pub symbols: Vec<String>,
    pub imports: Vec<String>,
    pub token_count: usize,
}

#[derive(Debug, Clone)]
pub struct SymbolContext {
    pub name: String,
    pub kind: String,
    pub location: String,
    pub definition: String,
    pub references: Vec<String>,
    pub related_symbols: Vec<String>,
    pub token_count: usize,
}

pub struct ContextAggregator {
    graph_store: Arc<GraphStore>,
    vector_store: Arc<RwLock<VectorStore>>,
    workspace_root: PathBuf,
}

impl ContextAggregator {
    pub fn new(
        graph_store: Arc<GraphStore>,
        vector_store: Arc<RwLock<VectorStore>>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            graph_store,
            vector_store,
            workspace_root,
        }
    }

    pub async fn gather_context(
        &self,
        query: &str,
        max_tokens: usize,
    ) -> Result<AggregatedContext> {
        let max_tokens = if max_tokens == 0 {
            DEFAULT_MAX_TOKENS
        } else {
            max_tokens
        };
        let mut context = AggregatedContext {
            files: Vec::new(),
            symbols: Vec::new(),
            total_tokens: 0,
        };

        let embedder = crate::graph_rag::SimpleEmbedder::new(384);
        let query_embedding = embedder.embed(query).await?;

        let store = self.vector_store.read().await;
        let vector_results = store.search(query_embedding, VECTOR_SEARCH_LIMIT).await?;
        drop(store);

        let mut symbol_ids = HashSet::new();
        for result in &vector_results {
            if let Some(symbol_id_str) = result.metadata.get("symbol_id") {
                if let Ok(symbol_id) = symbol_id_str.parse::<i64>() {
                    symbol_ids.insert(symbol_id);
                }
            }
        }

        let text_results = self.graph_store.search_symbols(query)?;
        for symbol in text_results {
            if let Some(id) = symbol.id {
                symbol_ids.insert(id);
            }
        }

        let mut processed_symbols = HashSet::new();
        let mut processed_files = HashSet::new();

        for symbol_id in symbol_ids {
            if context.total_tokens >= max_tokens {
                break;
            }

            if let Some(symbol_ctx) = self
                .gather_symbol_context(symbol_id, &mut processed_symbols)
                .await?
            {
                let tokens = symbol_ctx.token_count;
                if context.total_tokens + tokens <= max_tokens {
                    context.symbols.push(symbol_ctx);
                    context.total_tokens += tokens;
                }
            }
        }

        for symbol in &context.symbols {
            if let Some(file_context) = self
                .gather_file_context(&symbol.location, &mut processed_files)
                .await?
            {
                let tokens = file_context.token_count;
                if context.total_tokens + tokens <= max_tokens {
                    context.files.push(file_context);
                    context.total_tokens += tokens;
                }
            }
        }

        Ok(context)
    }

    async fn gather_symbol_context(
        &self,
        symbol_id: i64,
        processed: &mut HashSet<i64>,
    ) -> Result<Option<SymbolContext>> {
        if processed.contains(&symbol_id) {
            return Ok(None);
        }

        let symbol = match self.graph_store.find_symbol_by_id(symbol_id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        processed.insert(symbol_id);

        let file_info = self.graph_store.get_file_by_id(symbol.file_id)?;

        let location = match file_info {
            Some(ref file) => format!("{}:{}", file.path, symbol.start_line),
            None => format!("unknown:{}", symbol.start_line),
        };

        let content = tokio::fs::read_to_string(
            self.workspace_root
                .join(file_info.as_ref().map(|f| f.path.as_str()).unwrap_or("")),
        )
        .await
        .unwrap_or_default();

        let definition = self.extract_symbol_content(&symbol, &content);

        let refs = self.graph_store.find_symbol_refs(symbol_id)?;
        let references: Vec<String> = refs
            .iter()
            .take(10)
            .filter_map(|r| {
                self.graph_store.get_file_by_id(r.file_id).ok()?.map(|f| {
                    format!(
                        "{}:{}:{}",
                        f.path,
                        r.line,
                        r.context.as_deref().unwrap_or("")
                    )
                })
            })
            .collect();

        let relationships = self.graph_store.find_outgoing_relationships(symbol_id)?;
        let related_symbols: Vec<String> = relationships
            .iter()
            .take(5)
            .filter_map(|rel| {
                self.graph_store
                    .find_symbol_by_id(rel.to_symbol)
                    .ok()?
                    .map(|s| format!("{}:{}", rel.relationship_type.as_str(), s.name))
            })
            .collect();

        let token_count = (definition.len() + references.len() * 50) / 4;

        Ok(Some(SymbolContext {
            name: symbol.name,
            kind: symbol.kind.as_str().to_string(),
            location,
            definition,
            references,
            related_symbols,
            token_count,
        }))
    }

    async fn gather_file_context(
        &self,
        location: &str,
        processed: &mut HashSet<String>,
    ) -> Result<Option<FileContext>> {
        let parts: Vec<&str> = location.split(':').collect();
        let file_path = parts.first().unwrap_or(&"");

        if processed.contains(*file_path) || file_path.is_empty() {
            return Ok(None);
        }

        processed.insert(file_path.to_string());

        let full_path = self.workspace_root.join(file_path);
        let content = match tokio::fs::read_to_string(&full_path).await {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let language = full_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_string());

        let file_id = match self.graph_store.get_file_id(file_path)? {
            Some(id) => id,
            None => return Ok(None),
        };

        let symbols = self.graph_store.find_symbols_in_file(file_id)?;
        let symbol_names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();

        let imports = Vec::new();

        let token_count = content.len() / 4;

        Ok(Some(FileContext {
            path: full_path,
            language,
            symbols: symbol_names,
            imports,
            token_count,
        }))
    }

    fn extract_symbol_content(&self, symbol: &Symbol, source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();
        let start = (symbol.start_line as usize).saturating_sub(1);
        let end = (symbol.end_line as usize).min(lines.len());

        if start < end {
            lines[start..end].join("\n")
        } else {
            String::new()
        }
    }

    pub async fn get_related_files(&self, file: &Path) -> Result<Vec<PathBuf>> {
        let file_path = file.to_string_lossy().to_string();
        let file_id = match self.graph_store.get_file_id(&file_path)? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let symbols = self.graph_store.find_symbols_in_file(file_id)?;
        let mut related_files = HashSet::new();

        for symbol in symbols {
            let symbol_id = match symbol.id {
                Some(id) => id,
                None => continue,
            };

            let refs = self.graph_store.find_symbol_refs(symbol_id)?;
            for r#ref in refs {
                if r#ref.file_id != file_id {
                    if let Some(ref_file) = self.graph_store.get_file_by_id(r#ref.file_id)? {
                        related_files.insert(PathBuf::from(ref_file.path));
                    }
                }
            }

            let relationships = self.graph_store.find_outgoing_relationships(symbol_id)?;
            for rel in relationships {
                if let Some(target_symbol) = self.graph_store.find_symbol_by_id(rel.to_symbol)? {
                    if target_symbol.file_id != file_id {
                        if let Some(target_file) =
                            self.graph_store.get_file_by_id(target_symbol.file_id)?
                        {
                            related_files.insert(PathBuf::from(target_file.path));
                        }
                    }
                }
            }
        }

        Ok(related_files.into_iter().collect())
    }

    pub async fn get_symbol_context(&self, symbol_name: &str) -> Result<Option<SymbolContext>> {
        let symbols = self.graph_store.find_symbol(symbol_name)?;

        if let Some(symbol) = symbols.first() {
            if let Some(symbol_id) = symbol.id {
                let mut processed = HashSet::new();
                self.gather_symbol_context(symbol_id, &mut processed).await
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn to_context_items(&self, aggregated: &AggregatedContext) -> Vec<ContextItem> {
        let mut items = Vec::new();

        for file_ctx in &aggregated.files {
            if let Ok(content) = std::fs::read_to_string(&file_ctx.path) {
                items.push(ContextItem::File {
                    path: file_ctx.path.to_string_lossy().to_string(),
                    content,
                    language: file_ctx.language.clone(),
                });
            }
        }

        for symbol_ctx in &aggregated.symbols {
            items.push(ContextItem::Symbol {
                name: symbol_ctx.name.clone(),
                location: symbol_ctx.location.clone(),
                content: symbol_ctx.definition.clone(),
            });
        }

        items
    }
}
