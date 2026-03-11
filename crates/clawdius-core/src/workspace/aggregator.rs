//! Context aggregator for multi-file context understanding

use crate::error::Result;
use crate::graph_rag::ast::Symbol;
use crate::graph_rag::GraphStore;
use std::path::Path;
use std::sync::Arc;

pub struct ContextAggregator {
    graph_store: Arc<GraphStore>,
    max_context_tokens: usize,
}

impl ContextAggregator {
    pub fn new(graph_store: Arc<GraphStore>) -> Self {
        Self {
            graph_store,
            max_context_tokens: 4000,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_context_tokens = max_tokens;
        self
    }

    pub fn gather_context(
        &self,
        query: &str,
        current_file: Option<&Path>,
    ) -> Result<AggregatedContext> {
        let mut context = AggregatedContext::default();

        let symbols = self.graph_store.search_symbols(query)?;
        context.relevant_symbols = symbols;

        if let Some(file) = current_file {
            if let Some(file_str) = file.to_str() {
                if let Some(file_id) = self.graph_store.get_file_id(file_str)? {
                    let file_symbols = self.graph_store.find_symbols_in_file(file_id)?;
                    context.current_file_symbols = file_symbols;
                }
            }
        }

        Ok(context)
    }

    pub fn max_context_tokens(&self) -> usize {
        self.max_context_tokens
    }

    pub fn graph_store(&self) -> &GraphStore {
        &self.graph_store
    }
}

#[derive(Debug, Default)]
pub struct AggregatedContext {
    pub relevant_symbols: Vec<Symbol>,
    pub current_file_symbols: Vec<Symbol>,
}

impl AggregatedContext {
    pub fn total_symbols(&self) -> usize {
        self.relevant_symbols.len() + self.current_file_symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relevant_symbols.is_empty() && self.current_file_symbols.is_empty()
    }

    pub fn all_symbols(&self) -> Vec<&Symbol> {
        let mut symbols: Vec<&Symbol> = self.relevant_symbols.iter().collect();
        for sym in &self.current_file_symbols {
            if !self.relevant_symbols.iter().any(|s| s.id == sym.id) {
                symbols.push(sym);
            }
        }
        symbols
    }
}
