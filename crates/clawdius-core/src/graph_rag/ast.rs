//! AST parsing and indexing

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// AST node type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Variable,
    Constant,
}

/// AST node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstNode {
    /// Node ID
    pub id: String,
    /// Node type
    pub ty: NodeType,
    /// Node name
    pub name: String,
    /// File path
    pub file: PathBuf,
    /// Start position
    pub start: Position,
    /// End position
    pub end: Position,
}

/// Position in file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Line number
    pub line: usize,
    /// Column number
    pub column: usize,
}

/// File information for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// File path
    pub path: String,
    /// Content hash
    pub hash: String,
    /// Programming language
    pub language: Option<String>,
    /// Last modified timestamp
    pub last_modified: Option<i64>,
}

/// Symbol kind
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Trait,
    Module,
    Variable,
    Constant,
    Method,
    Field,
    Interface,
    Type,
    Macro,
    Other(String),
}

impl SymbolKind {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Module => "module",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Method => "method",
            SymbolKind::Field => "field",
            SymbolKind::Interface => "interface",
            SymbolKind::Type => "type",
            SymbolKind::Macro => "macro",
            SymbolKind::Other(s) => s.as_str(),
        }
    }

    #[must_use]
    pub fn parse_kind(s: &str) -> Self {
        match s {
            "function" => SymbolKind::Function,
            "class" => SymbolKind::Class,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "module" => SymbolKind::Module,
            "variable" => SymbolKind::Variable,
            "constant" => SymbolKind::Constant,
            "method" => SymbolKind::Method,
            "field" => SymbolKind::Field,
            "interface" => SymbolKind::Interface,
            "type" => SymbolKind::Type,
            "macro" => SymbolKind::Macro,
            other => SymbolKind::Other(other.to_string()),
        }
    }
}

/// Symbol in code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol ID (database primary key)
    pub id: Option<i64>,
    /// File ID this symbol belongs to
    pub file_id: i64,
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Function/method signature
    pub signature: Option<String>,
    /// Documentation comment
    pub doc_comment: Option<String>,
    /// Start line (1-indexed)
    pub start_line: i32,
    /// End line (1-indexed)
    pub end_line: i32,
    /// Start column (1-indexed)
    pub start_col: i32,
    /// End column (1-indexed)
    pub end_col: i32,
}

/// Reference to a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Reference ID
    pub id: Option<i64>,
    /// Symbol being referenced
    pub symbol_id: i64,
    /// File where reference occurs
    pub file_id: i64,
    /// Line number
    pub line: i32,
    /// Column number
    pub col: i32,
    /// Surrounding code context
    pub context: Option<String>,
}

/// Relationship type between symbols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationshipType {
    Calls,
    Imports,
    Extends,
    Implements,
    Contains,
    References,
    DependsOn,
    Other(String),
}

impl RelationshipType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            RelationshipType::Calls => "calls",
            RelationshipType::Imports => "imports",
            RelationshipType::Extends => "extends",
            RelationshipType::Implements => "implements",
            RelationshipType::Contains => "contains",
            RelationshipType::References => "references",
            RelationshipType::DependsOn => "depends_on",
            RelationshipType::Other(s) => s.as_str(),
        }
    }

    #[must_use]
    pub fn parse_relationship(s: &str) -> Self {
        match s {
            "calls" => RelationshipType::Calls,
            "imports" => RelationshipType::Imports,
            "extends" => RelationshipType::Extends,
            "implements" => RelationshipType::Implements,
            "contains" => RelationshipType::Contains,
            "references" => RelationshipType::References,
            "depends_on" => RelationshipType::DependsOn,
            other => RelationshipType::Other(other.to_string()),
        }
    }
}

/// Relationship between symbols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Relationship ID
    pub id: Option<i64>,
    /// Source symbol
    pub from_symbol: i64,
    /// Target symbol
    pub to_symbol: i64,
    /// Relationship type
    pub relationship_type: RelationshipType,
}
