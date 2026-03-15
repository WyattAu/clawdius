//! Tree-sitter based code parser

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tree_sitter::{Node, Parser, Tree};

use crate::graph_rag::ast::{Reference, Symbol, SymbolKind};
use crate::graph_rag::languages::LanguageKind;

#[derive(Clone)]
pub struct CodeParser {
    parsers: Arc<Mutex<HashMap<LanguageKind, Parser>>>,
}

impl CodeParser {
    pub fn new() -> Result<Self> {
        let mut parsers = HashMap::new();

        for lang in &[
            LanguageKind::Rust,
            LanguageKind::Python,
            LanguageKind::JavaScript,
            LanguageKind::TypeScript,
            LanguageKind::TypeScriptJsx,
            LanguageKind::Go,
        ] {
            let mut parser = Parser::new();
            parser.set_language(&lang.tree_sitter_language())?;
            parsers.insert(*lang, parser);
        }

        Ok(Self {
            parsers: Arc::new(Mutex::new(parsers)),
        })
    }

    pub fn parse(&self, source: &str, lang: LanguageKind) -> Result<Tree> {
        let mut parsers = self.parsers.lock().unwrap();
        let parser = parsers
            .get_mut(&lang)
            .ok_or_else(|| anyhow::anyhow!("No parser available for language: {lang}"))?;

        parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source"))
    }

    #[must_use]
    pub fn extract_symbols(
        &self,
        tree: &Tree,
        source: &str,
        file_id: i64,
        lang: LanguageKind,
    ) -> Vec<Symbol> {
        let root = tree.root_node();
        let mut symbols = Vec::new();
        self.extract_symbols_recursive(&root, source, file_id, lang, &mut symbols);
        symbols
    }

    fn extract_symbols_recursive(
        &self,
        node: &Node<'_>,
        source: &str,
        file_id: i64,
        lang: LanguageKind,
        symbols: &mut Vec<Symbol>,
    ) {
        if let Some(symbol) = self.node_to_symbol(node, source, file_id, lang) {
            symbols.push(symbol);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols_recursive(&child, source, file_id, lang, symbols);
        }
    }

    fn node_to_symbol(
        &self,
        node: &Node<'_>,
        source: &str,
        file_id: i64,
        lang: LanguageKind,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let symbol_kind = self.node_kind_to_symbol_kind(kind, lang)?;

        let name = self.extract_name(node, source, lang)?;

        let start_point = node.start_position();
        let end_point = node.end_position();

        let signature = self.extract_signature(node, source, lang);
        let doc_comment = self.extract_doc_comment(node, source, lang);

        Some(Symbol {
            id: None,
            file_id,
            name,
            kind: symbol_kind,
            signature,
            doc_comment,
            start_line: start_point.row as i32 + 1,
            end_line: end_point.row as i32 + 1,
            start_col: start_point.column as i32 + 1,
            end_col: end_point.column as i32 + 1,
        })
    }

    fn node_kind_to_symbol_kind(&self, kind: &str, lang: LanguageKind) -> Option<SymbolKind> {
        match lang {
            LanguageKind::Rust => match kind {
                "function_item" => Some(SymbolKind::Function),
                "struct_item" => Some(SymbolKind::Struct),
                "enum_item" => Some(SymbolKind::Enum),
                "trait_item" => Some(SymbolKind::Trait),
                "impl_item" => Some(SymbolKind::Trait),
                "mod_item" => Some(SymbolKind::Module),
                "const_item" => Some(SymbolKind::Constant),
                "static_item" => Some(SymbolKind::Constant),
                "type_item" => Some(SymbolKind::Type),
                "macro_definition" => Some(SymbolKind::Macro),
                "macro_invocation" => Some(SymbolKind::Macro),
                _ => None,
            },
            LanguageKind::Python => match kind {
                "function_definition" => Some(SymbolKind::Function),
                "class_definition" => Some(SymbolKind::Class),
                "decorated_definition" => Some(SymbolKind::Function),
                _ => None,
            },
            LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::TypeScriptJsx => {
                match kind {
                    "function_declaration" => Some(SymbolKind::Function),
                    "function_expression" => Some(SymbolKind::Function),
                    "arrow_function" => Some(SymbolKind::Function),
                    "method_definition" => Some(SymbolKind::Method),
                    "class_declaration" => Some(SymbolKind::Class),
                    "interface_declaration" => Some(SymbolKind::Interface),
                    "type_alias_declaration" => Some(SymbolKind::Type),
                    "enum_declaration" => Some(SymbolKind::Enum),
                    "variable_declaration" => Some(SymbolKind::Variable),
                    "lexical_declaration" => Some(SymbolKind::Variable),
                    "export_statement" => None,
                    _ => None,
                }
            }
            LanguageKind::Go => match kind {
                "function_declaration" => Some(SymbolKind::Function),
                "method_declaration" => Some(SymbolKind::Method),
                "type_declaration" => Some(SymbolKind::Type),
                "type_spec" => Some(SymbolKind::Type),
                "const_declaration" => Some(SymbolKind::Constant),
                "var_declaration" => Some(SymbolKind::Variable),
                _ => None,
            },
        }
    }

    fn extract_name(&self, node: &Node<'_>, source: &str, lang: LanguageKind) -> Option<String> {
        let kind = node.kind();

        let name_field = match lang {
            LanguageKind::Rust => match kind {
                "function_item" | "struct_item" | "enum_item" | "trait_item" | "mod_item"
                | "const_item" | "static_item" | "type_item" => Some("name"),
                "macro_definition" => Some("name"),
                _ => None,
            },
            LanguageKind::Python => match kind {
                "function_definition" | "class_definition" => Some("name"),
                "decorated_definition" => {
                    let child = node.child(0)?;
                    return self.extract_name(&child, source, lang);
                }
                _ => None,
            },
            LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::TypeScriptJsx => {
                match kind {
                    "function_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "type_alias_declaration"
                    | "enum_declaration" => Some("name"),
                    "variable_declaration" | "lexical_declaration" => {
                        let declarator = node.child_by_field_name("declarator")?;
                        let name_node = declarator.child_by_field_name("name")?;
                        return Some(self.node_text(&name_node, source));
                    }
                    "function_expression" | "arrow_function" => None,
                    "method_definition" => Some("name"),
                    _ => None,
                }
            }
            LanguageKind::Go => match kind {
                "function_declaration" | "method_declaration" => Some("name"),
                "type_declaration" => {
                    let spec = node.child_by_field_name("type")?;
                    return self.extract_name(&spec, source, lang);
                }
                "type_spec" => Some("name"),
                "const_declaration" | "var_declaration" => {
                    let spec = node.child(0)?;
                    if spec.kind() == "const_spec" || spec.kind() == "var_spec" {
                        let name_node = spec.child_by_field_name("name")?;
                        return Some(self.node_text(&name_node, source));
                    }
                    return None;
                }
                _ => None,
            },
        };

        if let Some(field) = name_field {
            if let Some(name_node) = node.child_by_field_name(field) {
                return Some(self.node_text(&name_node, source));
            }
        }

        None
    }

    fn extract_signature(
        &self,
        node: &Node<'_>,
        source: &str,
        lang: LanguageKind,
    ) -> Option<String> {
        let kind = node.kind();

        match lang {
            LanguageKind::Rust => {
                if kind == "function_item" {
                    let params = node.child_by_field_name("parameters")?;
                    Some(self.node_text(&params, source))
                } else {
                    None
                }
            }
            LanguageKind::Python => {
                if kind == "function_definition" {
                    let params = node.child_by_field_name("parameters")?;
                    Some(self.node_text(&params, source))
                } else {
                    None
                }
            }
            LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::TypeScriptJsx => {
                if matches!(
                    kind,
                    "function_declaration"
                        | "function_expression"
                        | "arrow_function"
                        | "method_definition"
                ) {
                    let params = node.child_by_field_name("parameters")?;
                    Some(self.node_text(&params, source))
                } else {
                    None
                }
            }
            LanguageKind::Go => {
                if matches!(kind, "function_declaration" | "method_declaration") {
                    let params = node.child_by_field_name("parameters")?;
                    Some(self.node_text(&params, source))
                } else {
                    None
                }
            }
        }
    }

    fn extract_doc_comment(
        &self,
        node: &Node<'_>,
        source: &str,
        lang: LanguageKind,
    ) -> Option<String> {
        let prev = node.prev_sibling()?;
        let kind = prev.kind();

        match lang {
            LanguageKind::Rust => {
                if kind == "line_comment" || kind == "block_comment" {
                    let text = self.node_text(&prev, source);
                    if text.starts_with("///") || text.starts_with("/**") {
                        return Some(text);
                    }
                }
                None
            }
            LanguageKind::Python => {
                if kind == "expression_statement" {
                    if let Some(string) = prev.child(0) {
                        if string.kind() == "string" {
                            return Some(self.node_text(&string, source));
                        }
                    }
                }
                None
            }
            LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::TypeScriptJsx => {
                if kind == "comment" {
                    return Some(self.node_text(&prev, source));
                }
                None
            }
            LanguageKind::Go => {
                if kind == "comment" {
                    return Some(self.node_text(&prev, source));
                }
                None
            }
        }
    }

    fn node_text(&self, node: &Node<'_>, source: &str) -> String {
        node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
    }

    #[must_use]
    pub fn extract_references(&self, tree: &Tree, source: &str, file_id: i64) -> Vec<Reference> {
        let root = tree.root_node();
        let mut references = Vec::new();
        self.extract_references_recursive(&root, source, file_id, &mut references);
        references
    }

    fn extract_references_recursive(
        &self,
        node: &Node<'_>,
        source: &str,
        file_id: i64,
        references: &mut Vec<Reference>,
    ) {
        let kind = node.kind();

        if matches!(
            kind,
            "identifier"
                | "type_identifier"
                | "field_identifier"
                | "property_identifier"
                | "simple_identifier"
        ) {
            let point = node.start_position();
            let _text = self.node_text(node, source);

            let context = self.extract_context(node, source);

            references.push(Reference {
                id: None,
                symbol_id: 0,
                file_id,
                line: point.row as i32 + 1,
                col: point.column as i32 + 1,
                context: Some(context),
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_references_recursive(&child, source, file_id, references);
        }
    }

    fn extract_context(&self, node: &Node<'_>, source: &str) -> String {
        let mut current = *node;

        for _ in 0..3 {
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }

        self.node_text(&current, source)
    }

    #[must_use]
    pub fn extract_imports(&self, tree: &Tree, source: &str, lang: LanguageKind) -> Vec<String> {
        let root = tree.root_node();
        let mut imports = Vec::new();
        self.extract_imports_recursive(&root, source, lang, &mut imports);
        imports
    }

    fn extract_imports_recursive(
        &self,
        node: &Node<'_>,
        source: &str,
        lang: LanguageKind,
        imports: &mut Vec<String>,
    ) {
        let kind = node.kind();

        let is_import = match lang {
            LanguageKind::Rust => matches!(kind, "use_declaration"),
            LanguageKind::Python => matches!(kind, "import_statement" | "import_from_statement"),
            LanguageKind::JavaScript | LanguageKind::TypeScript | LanguageKind::TypeScriptJsx => {
                matches!(kind, "import_statement" | "export_statement")
            }
            LanguageKind::Go => matches!(kind, "import_declaration"),
        };

        if is_import {
            imports.push(self.node_text(node, source));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_imports_recursive(&child, source, lang, imports);
        }
    }
}

impl Default for CodeParser {
    fn default() -> Self {
        Self::new().expect("Failed to initialize CodeParser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_rag::languages::LanguageKind;

    fn get_parser() -> CodeParser {
        CodeParser::new().expect("Failed to create parser")
    }

    #[test]
    fn test_parse_rust() {
        let parser = get_parser();
        let source = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let result = parser.parse(source, LanguageKind::Rust);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_python() {
        let parser = get_parser();
        let source = r#"
def hello():
    print("Hello, world!")
"#;
        let result = parser.parse(source, LanguageKind::Python);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_javascript() {
        let parser = get_parser();
        let source = r#"
function hello() {
    console.log("Hello, world!");
}
"#;
        let result = parser.parse(source, LanguageKind::JavaScript);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_typescript() {
        let parser = get_parser();
        let source = r"
function hello(name: string): void {
    console.log(`Hello, ${name}!`);
}
";
        let result = parser.parse(source, LanguageKind::TypeScript);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_go() {
        let parser = get_parser();
        let source = r#"
package main

func main() {
    println("Hello, world!")
}
"#;
        let result = parser.parse(source, LanguageKind::Go);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_rust_symbols() {
        let parser = get_parser();
        let source = r"
fn hello() {}
struct Foo {}
enum Bar { A, B }
";
        let tree = parser.parse(source, LanguageKind::Rust).unwrap();
        let symbols = parser.extract_symbols(&tree, source, 1, LanguageKind::Rust);

        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Struct));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Bar" && s.kind == SymbolKind::Enum));
    }

    #[test]
    fn test_extract_python_symbols() {
        let parser = get_parser();
        let source = r"
def hello():
    pass

class Foo:
    pass
";
        let tree = parser.parse(source, LanguageKind::Python).unwrap();
        let symbols = parser.extract_symbols(&tree, source, 1, LanguageKind::Python);

        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_extract_javascript_symbols() {
        let parser = get_parser();
        let source = r"
function hello() {}
class Foo {}
const bar = 42;
";
        let tree = parser.parse(source, LanguageKind::JavaScript).unwrap();
        let symbols = parser.extract_symbols(&tree, source, 1, LanguageKind::JavaScript);

        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Class));
    }

    #[test]
    fn test_extract_go_symbols() {
        let parser = get_parser();
        let source = r"
package main

func hello() {}
type Foo struct {}
";
        let tree = parser.parse(source, LanguageKind::Go).unwrap();
        let symbols = parser.extract_symbols(&tree, source, 1, LanguageKind::Go);

        assert!(symbols
            .iter()
            .any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_extract_rust_imports() {
        let parser = get_parser();
        let source = r"
use std::collections::HashMap;
use anyhow::Result;
";
        let tree = parser.parse(source, LanguageKind::Rust).unwrap();
        let imports = parser.extract_imports(&tree, source, LanguageKind::Rust);

        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_extract_python_imports() {
        let parser = get_parser();
        let source = r"
import os
from typing import List
";
        let tree = parser.parse(source, LanguageKind::Python).unwrap();
        let imports = parser.extract_imports(&tree, source, LanguageKind::Python);

        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_extract_javascript_imports() {
        let parser = get_parser();
        let source = r"
import { foo } from 'bar';
import * as baz from 'qux';
";
        let tree = parser.parse(source, LanguageKind::JavaScript).unwrap();
        let imports = parser.extract_imports(&tree, source, LanguageKind::JavaScript);

        assert_eq!(imports.len(), 2);
    }
}
