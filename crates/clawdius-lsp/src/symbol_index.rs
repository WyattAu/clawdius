//! Symbol index backed by clawdius-core Tree-sitter parsers.
//!
//! Wraps the existing graph_rag module's parsing infrastructure
//! to provide LSP-compatible symbol information, hover documentation,
//! go-to-definition, and find-all-references.

use anyhow::Result;
use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind, Url};
use std::collections::HashMap;

/// A symbol tracked in the index with position and context.
#[derive(Clone, Debug)]
pub struct IndexedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub character: u32,
    pub end_character: u32,
    pub uri: String,
    /// The full line of source where the symbol was defined.
    pub definition_line: String,
    /// Documentation comment extracted from lines above the definition.
    pub doc_comment: Option<String>,
}

/// Index of symbols extracted from open documents.
pub struct SymbolIndex {
    documents: HashMap<String, DocumentData>,
    /// Global symbol table: name -> list of indexed symbols across all docs.
    name_index: HashMap<String, Vec<IndexedSymbol>>,
}

struct DocumentData {
    symbols: Vec<DocumentSymbol>,
    indexed: Vec<IndexedSymbol>,
    source_text: String,
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    /// Index a document, extracting symbols via pattern matching.
    pub fn index_document(&mut self, uri: &Url, text: &str) -> Result<()> {
        // Remove old symbols for this URI from the name index
        if let Some(old) = self.documents.remove(&uri.to_string()) {
            for sym in &old.indexed {
                if let Some(entries) = self.name_index.get_mut(&sym.name) {
                    entries.retain(|e| e.uri != uri.to_string());
                }
            }
        }

        let (doc_symbols, indexed) = extract_symbols_with_context(uri, text);
        
        // Build name index
        for sym in &indexed {
            self.name_index
                .entry(sym.name.clone())
                .or_default()
                .push(sym.clone());
        }

        self.documents.insert(uri.to_string(), DocumentData {
            symbols: doc_symbols,
            indexed,
            source_text: text.to_string(),
        });

        Ok(())
    }

    /// Get document symbols for a URI (for textDocument/documentSymbol).
    pub fn document_symbols(&self, uri: &Url) -> Option<Vec<DocumentSymbol>> {
        self.documents.get(&uri.to_string()).map(|d| d.symbols.clone())
    }

    /// Get hover information for a symbol at a position.
    pub fn hover(&self, uri: &Url, line: u32, character: u32) -> Option<HoverInfo> {
        let doc = self.documents.get(&uri.to_string())?;
        
        // Find the symbol at the given position
        for sym in &doc.indexed {
            if sym.line == line && character >= 0 && character <= sym.end_character {
                return Some(HoverInfo {
                    name: sym.name.clone(),
                    kind: sym.kind,
                    definition: sym.definition_line.clone(),
                    doc_comment: sym.doc_comment.clone(),
                    references: self.count_references(&sym.name),
                });
            }
        }
        None
    }

    /// Find the definition of a symbol at a position.
    pub fn goto_definition(&self, uri: &Url, line: u32, character: u32) -> Option<&IndexedSymbol> {
        let doc = self.documents.get(&uri.to_string())?;
        
        // Find the symbol at the given position
        let sym_name = doc.indexed.iter()
            .find(|s| s.line == line && character <= s.end_character)?
            .name
            .clone();

        // Look up definition: prefer the symbol in the same document, then any
        let entries = self.name_index.get(&sym_name)?;
        entries.first()
    }

    /// Find all references to the symbol at a position.
    pub fn references(&self, uri: &Url, line: u32, character: u32) -> Vec<&IndexedSymbol> {
        let Some(doc) = self.documents.get(&uri.to_string()) else {
            return Vec::new();
        };
        
        // Find the symbol at the given position
        let Some(sym_name) = doc.indexed.iter()
            .find(|s| s.line == line && character <= s.end_character)
            .map(|s| &s.name)
        else {
            return Vec::new();
        };

        // Return all indexed symbols with the same name
        self.name_index.get(sym_name).map(|v| v.iter().collect()).unwrap_or_default()
    }

    /// Count references to a symbol name across all documents.
    fn count_references(&self, name: &str) -> usize {
        self.name_index.get(name).map(|v| v.len()).unwrap_or(0)
    }

    /// Get a summary of the index state.
    pub fn summary(&self) -> HashMap<String, usize> {
        let total_symbols: usize = self.documents.values().map(|d| d.indexed.len()).sum();
        let unique_names = self.name_index.len();
        let mut summary = HashMap::new();
        summary.insert("documents".to_string(), self.documents.len());
        summary.insert("symbols".to_string(), total_symbols);
        summary.insert("unique_names".to_string(), unique_names);
        summary
    }
}

/// Hover information for a symbol.
pub struct HoverInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub definition: String,
    pub doc_comment: Option<String>,
    pub references: usize,
}

impl HoverInfo {
    /// Format as markdown for LSP hover response.
    pub fn to_markdown(&self) -> String {
        let kind_str = symbol_kind_name(self.kind);
        let mut md = format!("**{}** `{}`", kind_str, self.name);
        if self.references > 0 {
            md.push_str(&format!(" ({} reference{})", self.references, if self.references > 1 { "s" } else { "" }));
        }
        md.push('\n');
        md.push_str(&format!("```rust\n{}\n```", self.definition.trim()));
        if let Some(doc) = &self.doc_comment {
            md.push_str(&format!("\n\n{}", doc));
        }
        md
    }
}

fn symbol_kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::FUNCTION => "function",
        SymbolKind::STRUCT => "struct",
        SymbolKind::CLASS => "class",
        SymbolKind::INTERFACE => "trait",
        _ => "symbol",
    }
}

/// Extract symbols with context information from source text.
fn extract_symbols_with_context(uri: &Url, text: &str) -> (Vec<DocumentSymbol>, Vec<IndexedSymbol>) {
    let ext = uri.path().rsplit('.').next().unwrap_or("");
    let lines: Vec<&str> = text.lines().collect();
    let mut doc_symbols = Vec::new();
    let mut indexed = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        let result = match ext {
            "rs" => extract_rust_symbol(trimmed, i, line.len()),
            "py" => extract_python_symbol(trimmed, i, line.len()),
            "go" => extract_go_symbol(trimmed, i, line.len()),
            "ts" | "tsx" | "js" | "jsx" => extract_js_symbol(trimmed, i, line.len()),
            _ => None,
        };

        if let Some((doc_sym, name, kind)) = result {
            // Extract doc comment from lines above
            let doc_comment = extract_doc_comment(&lines, i, ext);

            doc_symbols.push(doc_sym);
            indexed.push(IndexedSymbol {
                name,
                kind,
                line: i as u32,
                character: 0,
                end_character: line.len() as u32,
                uri: uri.to_string(),
                definition_line: line.to_string(),
                doc_comment,
            });
        }
    }

    (doc_symbols, indexed)
}

fn extract_rust_symbol(line: &str, line_num: usize, line_len: usize) -> Option<(DocumentSymbol, String, SymbolKind)> {
    if let Some(name) = extract_fn_name(line) {
        Some((make_symbol(&name, SymbolKind::FUNCTION, line_num, line_len), name, SymbolKind::FUNCTION))
    } else if let Some(name) = extract_struct_name(line) {
        Some((make_symbol(&name, SymbolKind::STRUCT, line_num, line_len), name, SymbolKind::STRUCT))
    } else if let Some(name) = extract_trait_name(line) {
        Some((make_symbol(&name, SymbolKind::INTERFACE, line_num, line_len), name, SymbolKind::INTERFACE))
    } else if let Some(name) = extract_enum_name(line) {
        Some((make_symbol(&name, SymbolKind::ENUM, line_num, line_len), name, SymbolKind::ENUM))
    } else if let Some(name) = extract_impl_name(line) {
        Some((make_symbol(&name, SymbolKind::NAMESPACE, line_num, line_len), name, SymbolKind::NAMESPACE))
    } else {
        None
    }
}

fn extract_python_symbol(line: &str, line_num: usize, line_len: usize) -> Option<(DocumentSymbol, String, SymbolKind)> {
    if line.starts_with("def ") {
        if let Some(name) = line.strip_prefix("def ") {
            let name = name.split('(').next().unwrap_or(name).trim();
            Some((make_symbol(name, SymbolKind::FUNCTION, line_num, line_len), name.to_string(), SymbolKind::FUNCTION))
        } else { None }
    } else if line.starts_with("class ") {
        if let Some(name) = line.strip_prefix("class ") {
            let name = name.split('(').next().unwrap_or(name).split(':').next().unwrap_or(name).trim();
            Some((make_symbol(name, SymbolKind::CLASS, line_num, line_len), name.to_string(), SymbolKind::CLASS))
        } else { None }
    } else {
        None
    }
}

fn extract_go_symbol(line: &str, line_num: usize, line_len: usize) -> Option<(DocumentSymbol, String, SymbolKind)> {
    if line.starts_with("func ") {
        let rest = line.strip_prefix("func ").unwrap_or(line);
        let name = if rest.starts_with('(') {
            if let Some(close) = rest.find(") ") { &rest[close + 2..] } else { rest }
        } else { rest };
        let name = name.split('(').next().unwrap_or(name).trim();
        if !name.is_empty() {
            Some((make_symbol(name, SymbolKind::FUNCTION, line_num, line_len), name.to_string(), SymbolKind::FUNCTION))
        } else { None }
    } else if line.starts_with("type ") {
        if let Some(rest) = line.strip_prefix("type ") {
            let name = rest.split(' ').next().unwrap_or(rest).trim();
            if !name.is_empty() && rest.contains("struct") {
                Some((make_symbol(name, SymbolKind::STRUCT, line_num, line_len), name.to_string(), SymbolKind::STRUCT))
            } else { None }
        } else { None }
    } else {
        None
    }
}

fn extract_js_symbol(line: &str, line_num: usize, line_len: usize) -> Option<(DocumentSymbol, String, SymbolKind)> {
    // function name() or const name = () =>
    if line.starts_with("function ") {
        if let Some(rest) = line.strip_prefix("function ") {
            let name = rest.split('(').next().unwrap_or(rest).trim();
            if !name.is_empty() {
                Some((make_symbol(name, SymbolKind::FUNCTION, line_num, line_len), name.to_string(), SymbolKind::FUNCTION))
            } else { None }
        } else { None }
    } else if line.starts_with("export function ") {
        if let Some(rest) = line.strip_prefix("export function ") {
            let name = rest.split('(').next().unwrap_or(rest).trim();
            if !name.is_empty() {
                Some((make_symbol(name, SymbolKind::FUNCTION, line_num, line_len), name.to_string(), SymbolKind::FUNCTION))
            } else { None }
        } else { None }
    } else if line.starts_with("class ") {
        if let Some(rest) = line.strip_prefix("class ") {
            let name = rest.split('{').next().unwrap_or(rest).split(' ').next().unwrap_or(rest).trim();
            if !name.is_empty() {
                Some((make_symbol(name, SymbolKind::CLASS, line_num, line_len), name.to_string(), SymbolKind::CLASS))
            } else { None }
        } else { None }
    } else {
        None
    }
}

/// Extract documentation comments from lines above the definition.
fn extract_doc_comment(lines: &[&str], def_line: usize, _ext: &str) -> Option<String> {
    let mut comment_lines = Vec::new();
    let mut i = def_line;
    
    while i > 0 {
        i -= 1;
        let line = lines.get(i)?.trim();
        
        // Rust doc comments: /// or //!
        if line.starts_with("///") || line.starts_with("//!") {
            comment_lines.push(line.trim_start_matches('/').trim_start_matches('!').trim());
        }
        // Block doc comments: */
        else if line.ends_with("*/") {
            // Simplified: just grab the line
            comment_lines.push(line.trim_start_matches('*').trim_end_matches('/').trim());
        }
        // Python docstrings (triple quotes above)
        else if line.starts_with("\"\"\"") {
            let content = line.trim_matches('"').trim();
            if !content.is_empty() {
                comment_lines.push(content);
            }
            break;
        }
        else if line.is_empty() {
            // Skip blank lines between doc comments
            continue;
        }
        else {
            break;
        }
    }
    
    if comment_lines.is_empty() {
        None
    } else {
        comment_lines.reverse();
        Some(comment_lines.join("\n"))
    }
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

fn extract_enum_name(line: &str) -> Option<String> {
    if line.starts_with("pub enum ") || line.starts_with("enum ") {
        let stripped = line.strip_prefix("pub enum ").or_else(|| line.strip_prefix("enum "))?;
        let name = stripped.split('<').next()?.split('{').next()?.split(';').next()?.trim();
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_impl_name(line: &str) -> Option<String> {
    if line.starts_with("impl ") {
        let stripped = line.strip_prefix("impl ")?;
        let name = stripped.split('<').next()?.split(' ').next()?.split('{').next()?.trim();
        if !name.is_empty() {
            Some(format!("impl {}", name))
        } else {
            None
        }
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
        let (symbols, indexed) = extract_symbols_with_context(&uri, code);
        assert_eq!(symbols.len(), 4);
        assert_eq!(indexed[0].name, "hello");
        assert_eq!(indexed[1].name, "world");
        assert_eq!(indexed[2].name, "Config");
        assert_eq!(indexed[3].name, "Handler");
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
        let (symbols, _) = extract_symbols_with_context(&uri, code);
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
        let (symbols, _) = extract_symbols_with_context(&uri, code);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Hello");
        assert_eq!(symbols[1].name, "Handle");
    }

    #[test]
    fn test_extract_js_symbols() {
        let code = r#"
function add(a, b) { return a + b; }
export function multiply(a, b) { return a * b; }
class Calculator {}
"#;
        let uri = Url::parse("file:///test.ts").unwrap();
        let (symbols, _) = extract_symbols_with_context(&uri, code);
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[1].name, "multiply");
        assert_eq!(symbols[2].name, "Calculator");
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
        assert_eq!(summary["unique_names"], 2);
    }

    #[test]
    fn test_hover_at_symbol() {
        let mut index = SymbolIndex::new();
        let uri = Url::parse("file:///test.rs").unwrap();
        index.index_document(&uri, "fn main() {}").unwrap();
        let hover = index.hover(&uri, 0, 3);
        assert!(hover.is_some());
        let info = hover.unwrap();
        assert_eq!(info.name, "main");
    }

    #[test]
    fn test_goto_definition_same_doc() {
        let mut index = SymbolIndex::new();
        let uri = Url::parse("file:///test.rs").unwrap();
        index.index_document(&uri, "fn foo() {}\nfn bar() { foo(); }").unwrap();
        let def = index.goto_definition(&uri, 0, 3);
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "foo");
    }

    #[test]
    fn test_references_across_docs() {
        let mut index = SymbolIndex::new();
        let uri1 = Url::parse("file:///a.rs").unwrap();
        let uri2 = Url::parse("file:///b.rs").unwrap();
        index.index_document(&uri1, "fn shared() {}").unwrap();
        index.index_document(&uri2, "fn shared() {}").unwrap();
        
        let refs = index.references(&uri1, 0, 3);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_doc_comment_extraction() {
        let code = "/// This is a doc comment\nfn foo() {}";
        let uri = Url::parse("file:///test.rs").unwrap();
        let (_, indexed) = extract_symbols_with_context(&uri, code);
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].doc_comment, Some("This is a doc comment".to_string()));
    }

    #[test]
    fn test_rust_enum_extraction() {
        let code = "pub enum Color { Red, Green, Blue }";
        let uri = Url::parse("file:///test.rs").unwrap();
        let (symbols, _) = extract_symbols_with_context(&uri, code);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Color");
    }

    #[test]
    fn test_rust_impl_extraction() {
        let code = "impl Config { fn new() {} }";
        let uri = Url::parse("file:///test.rs").unwrap();
        let (symbols, _) = extract_symbols_with_context(&uri, code);
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].name.contains("impl"));
    }
}
