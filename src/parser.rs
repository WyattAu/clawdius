//! Parser - Multi-language AST extraction using tree-sitter
//!
//! Provides language detection and AST extraction for multiple
//! programming languages using tree-sitter grammars.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::ast_store::{compute_hash, AstEdge, AstNode, Language, NodeId, NodeType};
use crate::error::Result;

/// Language detector based on file extension
#[derive(Debug, Default)]
pub struct LanguageDetector {
    /// Extension to language mapping
    extensions: HashMap<String, Language>,
}

impl LanguageDetector {
    /// Create a new language detector
    #[must_use]
    pub fn new() -> Self {
        let mut extensions = HashMap::new();

        extensions.insert("rs".to_string(), Language::Rust);
        extensions.insert("ts".to_string(), Language::TypeScript);
        extensions.insert("tsx".to_string(), Language::TypeScript);
        extensions.insert("py".to_string(), Language::Python);
        extensions.insert("cpp".to_string(), Language::Cpp);
        extensions.insert("cc".to_string(), Language::Cpp);
        extensions.insert("cxx".to_string(), Language::Cpp);
        extensions.insert("c".to_string(), Language::Cpp);
        extensions.insert("h".to_string(), Language::Cpp);
        extensions.insert("hpp".to_string(), Language::Cpp);
        extensions.insert("go".to_string(), Language::Go);
        extensions.insert("java".to_string(), Language::Java);

        Self { extensions }
    }

    /// Detect language from file extension
    #[must_use]
    pub fn detect(&self, path: &Path) -> Option<Language> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.extensions.get(ext.to_lowercase().as_str()))
            .copied()
    }

    /// Check if a file extension is supported
    #[must_use]
    pub fn is_supported(&self, path: &Path) -> bool {
        self.detect(path).is_some()
    }

    /// Get all supported extensions
    #[must_use]
    pub fn supported_extensions(&self) -> &[String] {
        static EMPTY: &[String] = &[];
        EMPTY
    }
}

/// Parsed file result
#[derive(Debug)]
pub struct ParsedFile {
    /// Source file path
    pub path: PathBuf,
    /// Detected language
    pub language: Language,
    /// Extracted AST nodes
    pub nodes: Vec<AstNode>,
    /// Extracted AST edges
    pub edges: Vec<AstEdge>,
    /// Parse errors (non-fatal)
    pub errors: Vec<String>,
}

/// Tree-sitter based parser
pub struct Parser {
    /// Language detector
    detector: LanguageDetector,
    /// tree-sitter parser
    parser: tree_sitter::Parser,
}

impl fmt::Debug for Parser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parser")
            .field("detector", &self.detector)
            .finish_non_exhaustive()
    }
}

impl Parser {
    /// Create a new parser
    pub fn new() -> Result<Self> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| {
                crate::error::ClawdiusError::Database(format!(
                    "Failed to set tree-sitter language: {e}"
                ))
            })?;

        Ok(Self {
            detector: LanguageDetector::new(),
            parser,
        })
    }

    /// Get the language detector
    #[must_use]
    pub fn detector(&self) -> &LanguageDetector {
        &self.detector
    }

    /// Parse a source file
    pub fn parse(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        let language = self.detector.detect(path).ok_or_else(|| {
            crate::error::ClawdiusError::Database(format!(
                "Unsupported file type: {}",
                path.display()
            ))
        })?;

        self.parse_with_language(path, content, language)
    }

    /// Parse with explicit language
    pub fn parse_with_language(
        &mut self,
        path: &Path,
        content: &str,
        language: Language,
    ) -> Result<ParsedFile> {
        match language {
            Language::Rust => self.parse_rust(path, content),
            Language::TypeScript => self.parse_typescript(path, content),
            Language::Python => self.parse_python(path, content),
            Language::Cpp => self.parse_cpp(path, content),
            Language::Go => self.parse_go(path, content),
            Language::Java => self.parse_java(path, content),
        }
    }

    fn parse_rust(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        self.parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| {
                crate::error::ClawdiusError::Database(format!("Failed to set Rust language: {e}"))
            })?;

        let tree = self.parser.parse(content, None).ok_or_else(|| {
            crate::error::ClawdiusError::Database("Failed to parse Rust file".into())
        })?;

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut errors = Vec::new();

        self.extract_rust_nodes(
            tree.root_node(),
            content,
            path,
            &mut nodes,
            &mut edges,
            &mut errors,
        );

        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Rust,
            nodes,
            edges,
            errors,
        })
    }

    fn extract_rust_nodes(
        &self,
        node: tree_sitter::Node<'_>,
        content: &str,
        path: &Path,
        nodes: &mut Vec<AstNode>,
        edges: &mut Vec<AstEdge>,
        errors: &mut Vec<String>,
    ) {
        let node_kind = node.kind();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_rust_nodes(child, content, path, nodes, edges, errors);
        }

        if node.is_error() {
            let start = node.start_position();
            errors.push(format!(
                "Parse error at line {}: {}",
                start.row + 1,
                node_kind
            ));
            return;
        }

        let node_type = match node_kind {
            "function_item" | "function_signature_item" => Some(NodeType::Function),
            "struct_item" => Some(NodeType::Struct),
            "enum_item" => Some(NodeType::Enum),
            "trait_item" => Some(NodeType::Trait),
            "impl_item" => Some(NodeType::Impl),
            "type_item" => Some(NodeType::TypeAlias),
            "const_item" => Some(NodeType::Constant),
            "static_item" => Some(NodeType::Static),
            "mod_item" => Some(NodeType::Module),
            "macro_definition" | "macro_invocation" => Some(NodeType::Macro),
            "field_declaration" => Some(NodeType::Field),
            "enum_variant" => Some(NodeType::Variant),
            "parameter" | "self_parameter" => Some(NodeType::Parameter),
            "use_declaration" => Some(NodeType::Use),
            _ => None,
        };

        if let Some(nt) = node_type {
            let name = self.extract_name(node, content);
            let start_byte = node.start_byte() as u32;
            let end_byte = node.end_byte() as u32;
            let start_line = node.start_position().row as u32 + 1;
            let end_line = node.end_position().row as u32 + 1;

            let node_content =
                &content[start_byte as usize..(end_byte as usize).min(content.len())];
            let hash = compute_hash(node_content.as_bytes());

            let ast_node = AstNode {
                id: NodeId::new(),
                node_type: nt,
                name,
                file_path: path.to_path_buf(),
                start_byte,
                end_byte,
                start_line,
                end_line,
                language: Language::Rust,
                documentation: self.extract_docs(node, content),
                metadata: None,
                hash,
            };

            nodes.push(ast_node);
        }
    }

    fn extract_name(&self, node: tree_sitter::Node<'_>, content: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                let start = child.start_byte();
                let end = child.end_byte();
                return Some(content[start..end].to_string());
            }
            if child.kind() == "name" {
                let start = child.start_byte();
                let end = child.end_byte();
                return Some(content[start..end].to_string());
            }
        }
        None
    }

    fn extract_docs(&self, node: tree_sitter::Node<'_>, content: &str) -> Option<String> {
        let mut cursor = node.walk();
        let mut doc_lines = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "line_comment" || child.kind() == "block_comment" {
                let start = child.start_byte();
                let end = child.end_byte();
                doc_lines.push(content[start..end].to_string());
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join("\n"))
        }
    }

    fn parse_typescript(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::TypeScript,
            nodes: self.extract_generic_nodes(content, path, Language::TypeScript),
            edges: Vec::new(),
            errors: vec!["TypeScript parsing not fully implemented".into()],
        })
    }

    fn parse_python(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Python,
            nodes: self.extract_generic_nodes(content, path, Language::Python),
            edges: Vec::new(),
            errors: vec!["Python parsing not fully implemented".into()],
        })
    }

    fn parse_cpp(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Cpp,
            nodes: self.extract_generic_nodes(content, path, Language::Cpp),
            edges: Vec::new(),
            errors: vec!["C++ parsing not fully implemented".into()],
        })
    }

    fn parse_go(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Go,
            nodes: self.extract_generic_nodes(content, path, Language::Go),
            edges: Vec::new(),
            errors: vec!["Go parsing not fully implemented".into()],
        })
    }

    fn parse_java(&mut self, path: &Path, content: &str) -> Result<ParsedFile> {
        Ok(ParsedFile {
            path: path.to_path_buf(),
            language: Language::Java,
            nodes: self.extract_generic_nodes(content, path, Language::Java),
            edges: Vec::new(),
            errors: vec!["Java parsing not fully implemented".into()],
        })
    }

    fn extract_generic_nodes(
        &self,
        content: &str,
        path: &Path,
        language: Language,
    ) -> Vec<AstNode> {
        let mut nodes = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            let (node_type, name) = if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest.split('(').next().unwrap_or("unknown").trim();
                (NodeType::Function, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                let name = rest.split('{').next().unwrap_or("unknown").trim();
                (NodeType::Struct, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                let name = rest.split('{').next().unwrap_or("unknown").trim();
                (NodeType::Enum, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("trait ") {
                let name = rest.split('{').next().unwrap_or("unknown").trim();
                (NodeType::Trait, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("impl ") {
                let name = rest.split('{').next().unwrap_or("unknown").trim();
                (NodeType::Impl, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("def ") {
                let name = rest.split('(').next().unwrap_or("unknown").trim();
                (NodeType::Function, Some(name.to_string()))
            } else if let Some(rest) = trimmed.strip_prefix("class ") {
                let name = rest.split('{').next().unwrap_or("unknown").trim();
                (NodeType::Struct, Some(name.to_string()))
            } else if trimmed.starts_with("func ") {
                let name = trimmed
                    .split('(')
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                    .unwrap_or("unknown");
                (NodeType::Function, Some(name.to_string()))
            } else if trimmed.starts_with("public ") || trimmed.starts_with("private ") {
                if trimmed.contains(" class ") {
                    let name = trimmed
                        .split(" class ")
                        .nth(1)
                        .and_then(|s| s.split('{').next())
                        .unwrap_or("unknown")
                        .trim();
                    (NodeType::Struct, Some(name.to_string()))
                } else if trimmed.contains(" void ") || trimmed.contains(" int ") {
                    let name = trimmed
                        .split_whitespace()
                        .nth(2)
                        .and_then(|s| s.split('(').next())
                        .unwrap_or("unknown");
                    (NodeType::Function, Some(name.to_string()))
                } else {
                    continue;
                }
            } else {
                continue;
            };

            let start_line = idx as u32 + 1;
            let end_line = start_line;

            nodes.push(AstNode {
                id: NodeId::new(),
                node_type,
                name,
                file_path: path.to_path_buf(),
                start_byte: 0,
                end_byte: 0,
                start_line,
                end_line,
                language,
                documentation: None,
                metadata: None,
                hash: compute_hash(line.as_bytes()),
            });
        }

        nodes
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new().expect("Failed to create default parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detector() {
        let detector = LanguageDetector::new();

        assert_eq!(detector.detect(Path::new("test.rs")), Some(Language::Rust));
        assert_eq!(
            detector.detect(Path::new("test.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            detector.detect(Path::new("test.py")),
            Some(Language::Python)
        );
        assert_eq!(detector.detect(Path::new("test.cpp")), Some(Language::Cpp));
        assert_eq!(detector.detect(Path::new("test.go")), Some(Language::Go));
        assert_eq!(
            detector.detect(Path::new("Test.java")),
            Some(Language::Java)
        );
        assert_eq!(detector.detect(Path::new("test.unknown")), None);
    }

    #[test]
    fn test_parser_creation() {
        let parser = Parser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_rust_file() {
        let mut parser = Parser::new().expect("Failed to create parser");

        let code = r#"
fn main() {
    println!("Hello");
}

struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
"#;

        let result = parser.parse(Path::new("test.rs"), code);
        assert!(result.is_ok());

        let parsed = result.expect("Parse failed");
        assert_eq!(parsed.language, Language::Rust);

        let functions: Vec<_> = parsed
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Function)
            .collect();
        assert!(!functions.is_empty());

        let structs: Vec<_> = parsed
            .nodes
            .iter()
            .filter(|n| n.node_type == NodeType::Struct)
            .collect();
        assert!(!structs.is_empty());
    }

    #[test]
    fn test_parse_unsupported_file() {
        let mut parser = Parser::new().expect("Failed to create parser");
        let result = parser.parse(Path::new("test.xyz"), "some code");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_supported() {
        let detector = LanguageDetector::new();

        assert!(detector.is_supported(Path::new("test.rs")));
        assert!(detector.is_supported(Path::new("test.py")));
        assert!(!detector.is_supported(Path::new("test.xyz")));
    }
}
