//! Symbol index backed by clawdius-core Tree-sitter parsers.
//!
//! Wraps the existing graph_rag module's parsing infrastructure
//! to provide LSP-compatible symbol information.

use anyhow::Result;
use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind, Url};
use std::collections::HashMap;

/// Index of symbols extracted from open documents.
pub struct SymbolIndex {
    documents: HashMap<String, DocumentSymbols>,
}

struct DocumentSymbols {
    symbols: Vec<DocumentSymbol>,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    /// Index a document, extracting symbols via Tree-sitter.
    pub fn index_document(&mut self, uri: &Url, text: &str) -> Result<()> {
        let symbols = extract_symbols(uri, text)?;
        self.documents.insert(uri.to_string(), DocumentSymbols { symbols });
        Ok(())
    }

    /// Get document symbols for a URI.
    pub fn document_symbols(&self, uri: &Url) -> Option<Vec<DocumentSymbol>> {
        self.documents.get(&uri.to_string()).map(|d| d.symbols.clone())
    }

    /// Get a summary of the index state.
    pub fn summary(&self) -> HashMap<String, usize> {
        let total_symbols: usize = self.documents.values().map(|d| d.symbols.len()).sum();
        let mut summary = HashMap::new();
        summary.insert("documents".to_string(), self.documents.len());
        summary.insert("symbols".to_string(), total_symbols);
        summary
    }
}

/// Extract symbols from source text.
///
/// In production, this delegates to clawdius_core::graph_rag parser.
/// This scaffold provides basic symbol extraction for Rust, Python, and Go.
fn extract_symbols(uri: &Url, text: &str) -> Result<Vec<DocumentSymbol>> {
    let ext = uri.path().rsplit('.').next().unwrap_or("");
    let mut symbols = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        match ext {
            "rs" => {
                if let Some(name) = extract_fn_name(trimmed) {
                    symbols.push(make_symbol(&name, SymbolKind::FUNCTION, i, line.len()));
                } else if let Some(name) = extract_struct_name(trimmed) {
                    symbols.push(make_symbol(&name, SymbolKind::STRUCT, i, line.len()));
                } else if let Some(name) = extract_trait_name(trimmed) {
                    symbols.push(make_symbol(&name, SymbolKind::INTERFACE, i, line.len()));
                }
            }
            "py" => {
                if trimmed.starts_with("def ") {
                    if let Some(name) = trimmed.strip_prefix("def ") {
                        let name = name.split('(').next().unwrap_or(name).trim();
                        symbols.push(make_symbol(name, SymbolKind::FUNCTION, i, line.len()));
                    }
                } else if trimmed.starts_with("class ") {
                    if let Some(name) = trimmed.strip_prefix("class ") {
                        let name = name.split('(').next().unwrap_or(name).split(':').next().unwrap_or(name).trim();
                        symbols.push(make_symbol(name, SymbolKind::CLASS, i, line.len()));
                    }
                }
            }
            "go" => {
                if trimmed.starts_with("func ") {
                    // func Name() {} or func (r Receiver) Name() {}
                    let rest = trimmed.strip_prefix("func ").unwrap_or(trimmed);
                    let name = if rest.starts_with('(') {
                        // Method with receiver: skip (receiver) prefix
                        if let Some(close) = rest.find(") ") {
                            &rest[close + 2..]
                        } else {
                            rest
                        }
                    } else {
                        rest
                    };
                    let name = name.split('(').next().unwrap_or(name).trim();
                    if !name.is_empty() {
                        symbols.push(make_symbol(name, SymbolKind::FUNCTION, i, line.len()));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(symbols)
}

fn extract_fn_name(line: &str) -> Option<String> {
    if line.starts_with("pub fn ") || line.starts_with("fn ") || line.starts_with("pub async fn ") || line.starts_with("async fn ") {
        let stripped = line
            .strip_prefix("pub async fn ")
            .or_else(|| line.strip_prefix("async fn "))
            .or_else(|| line.strip_prefix("pub fn "))
            .or_else(|| line.strip_prefix("fn "))?;
        let name = stripped.split('(').next()?.trim();
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_struct_name(line: &str) -> Option<String> {
    if line.starts_with("pub struct ") || line.starts_with("struct ") {
        let stripped = line.strip_prefix("pub struct ").or_else(|| line.strip_prefix("struct "))?;
        let name = stripped.split('<').next()?.split('{').next()?.split(';').next()?.trim();
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_trait_name(line: &str) -> Option<String> {
    if line.starts_with("pub trait ") || line.starts_with("trait ") {
        let stripped = line.strip_prefix("pub trait ").or_else(|| line.strip_prefix("trait "))?;
        let name = stripped.split('<').next()?.split('{').next()?.split(':').next()?.trim();
        Some(name.to_string())
    } else {
        None
    }
}

fn make_symbol(name: &str, kind: SymbolKind, line: usize, end_char: usize) -> DocumentSymbol {
    use tower_lsp::lsp_types::{Position, Range};

    DocumentSymbol {
        name: name.to_string(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range: Range::new(Position::new(line as u32, 0), Position::new(line as u32, end_char as u32)),
        selection_range: Range::new(Position::new(line as u32, 0), Position::new(line as u32, end_char as u32)),
        children: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_symbols() {
        let code = r#"
pub fn hello() {}
fn world(x: i32) -> bool { true }
pub struct Config { name: String }
pub trait Handler { fn handle(&self); }
"#;
        let uri = Url::parse("file:///test.rs").unwrap();
        let symbols = extract_symbols(&uri, code).unwrap();
        assert_eq!(symbols.len(), 4);
        assert_eq!(symbols[0].name, "hello");
        assert_eq!(symbols[1].name, "world");
        assert_eq!(symbols[2].name, "Config");
        assert_eq!(symbols[3].name, "Handler");
    }

    #[test]
    fn test_extract_python_symbols() {
        let code = r#"
def greet(name):
    pass

class MyClass(Base):
    pass
"#;
        let uri = Url::parse("file:///test.py").unwrap();
        let symbols = extract_symbols(&uri, code).unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[1].name, "MyClass");
    }

    #[test]
    fn test_extract_go_symbols() {
        let code = r#"
func Hello() {}
func (s *Server) Handle() {}
"#;
        let uri = Url::parse("file:///test.go").unwrap();
        let symbols = extract_symbols(&uri, code).unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Hello");
        assert_eq!(symbols[1].name, "Handle");
    }

    #[test]
    fn test_index_document() {
        let mut index = SymbolIndex::new();
        let uri = Url::parse("file:///test.rs").unwrap();
        index.index_document(&uri, "fn main() {}").unwrap();
        let symbols = index.document_symbols(&uri).unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");
    }

    #[test]
    fn test_index_summary() {
        let mut index = SymbolIndex::new();
        let uri = Url::parse("file:///test.rs").unwrap();
        index.index_document(&uri, "fn foo() {}\nfn bar() {}").unwrap();
        let summary = index.summary();
        assert_eq!(summary["documents"], 1);
        assert_eq!(summary["symbols"], 2);
    }
}
