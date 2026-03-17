//! LSP Protocol Types
//!
//! Types for the Language Server Protocol v3.17.

use serde::{Deserialize, Serialize};

/// A position in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Line (0-based)
    pub line: u32,
    /// Character (0-based, UTF-16 code units)
    pub character: u32,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    /// Position at the start of the document.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0, 0)
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::zero()
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line
            .cmp(&other.line)
            .then(self.character.cmp(&other.character))
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A range in a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Start position
    pub start: Position,
    /// End position
    pub end: Position,
}

impl Range {
    /// Creates a new range.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Creates a range at a single position.
    #[must_use]
    pub const fn at(line: u32, character: u32) -> Self {
        let pos = Position::new(line, character);
        Self::new(pos, pos)
    }

    /// Creates a range spanning a line.
    #[must_use]
    pub const fn line(line: u32, start_char: u32, end_char: u32) -> Self {
        Self::new(
            Position::new(line, start_char),
            Position::new(line, end_char),
        )
    }
}

impl Default for Range {
    fn default() -> Self {
        Self::new(Position::zero(), Position::zero())
    }
}

/// A location in a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Document URI
    pub uri: String,
    /// Range in the document
    pub range: Range,
}

impl Location {
    /// Creates a new location.
    #[must_use]
    pub fn new(uri: impl Into<String>, range: Range) -> Self {
        Self {
            uri: uri.into(),
            range,
        }
    }
}

/// Text document identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    /// Document URI
    pub uri: String,
}

impl TextDocumentIdentifier {
    /// Creates a new document identifier.
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

/// Text document position parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentPositionParams {
    /// Text document
    pub text_document: TextDocumentIdentifier,
    /// Position
    pub position: Position,
}

impl TextDocumentPositionParams {
    /// Creates new parameters.
    #[must_use]
    pub fn new(uri: impl Into<String>, position: Position) -> Self {
        Self {
            text_document: TextDocumentIdentifier::new(uri),
            position,
        }
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    /// Error
    Error = 1,
    /// Warning
    Warning = 2,
    /// Information
    Information = 3,
    /// Hint
    Hint = 4,
}

#[allow(clippy::derivable_impls)]
impl Default for DiagnosticSeverity {
    fn default() -> Self {
        Self::Error
    }
}

/// A diagnostic (error/warning/etc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Range of the diagnostic
    pub range: Range,
    /// Severity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<DiagnosticSeverity>,
    /// Diagnostic code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<serde_json::Value>,
    /// Source of the diagnostic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Message
    pub message: String,
    /// Related information
    #[serde(default)]
    pub related_information: Vec<DiagnosticRelatedInformation>,
}

impl Diagnostic {
    /// Creates a new diagnostic.
    #[must_use]
    pub fn new(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: Some(DiagnosticSeverity::Error),
            code: None,
            source: None,
            message: message.into(),
            related_information: Vec::new(),
        }
    }

    /// Sets the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Sets the source.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// Related diagnostic information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticRelatedInformation {
    /// Location
    pub location: Location,
    /// Message
    pub message: String,
}

/// Completion item kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

/// A completion item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// Label (shown in the completion list)
    pub label: String,
    /// Kind
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,
    /// Detail (shown after label)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    /// Sort text (for ordering)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
    /// Filter text (for filtering)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
    /// Insert text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
    /// Whether to preselect this item
    #[serde(default)]
    pub preselect: bool,
}

impl CompletionItem {
    /// Creates a new completion item.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: None,
            detail: None,
            documentation: None,
            sort_text: None,
            filter_text: None,
            insert_text: None,
            preselect: false,
        }
    }

    /// Sets the kind.
    #[must_use]
    pub fn with_kind(mut self, kind: CompletionItemKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Sets the detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets the documentation.
    #[must_use]
    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }
}

/// Completion list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionList {
    /// Whether the list is incomplete
    pub is_incomplete: bool,
    /// Completion items
    pub items: Vec<CompletionItem>,
}

impl CompletionList {
    /// Creates an empty list.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            is_incomplete: false,
            items: Vec::new(),
        }
    }

    /// Creates a list from items.
    #[must_use]
    pub fn from_items(items: Vec<CompletionItem>) -> Self {
        Self {
            is_incomplete: false,
            items,
        }
    }
}

impl Default for CompletionList {
    fn default() -> Self {
        Self::empty()
    }
}

/// Hover information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    /// Contents
    pub contents: HoverContents,
    /// Range (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}

/// Hover contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HoverContents {
    /// Plain text
    String(String),
    /// Markup content
    Markup(MarkupContent),
    /// Multiple contents
    Array(Vec<MarkedString>),
}

/// Markup content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    /// Kind (plaintext or markdown)
    pub kind: MarkupKind,
    /// Value
    pub value: String,
}

impl MarkupContent {
    /// Creates markdown content.
    #[must_use]
    pub fn markdown(value: impl Into<String>) -> Self {
        Self {
            kind: MarkupKind::Markdown,
            value: value.into(),
        }
    }

    /// Creates plaintext content.
    #[must_use]
    pub fn plaintext(value: impl Into<String>) -> Self {
        Self {
            kind: MarkupKind::PlainText,
            value: value.into(),
        }
    }
}

/// Markup kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkupKind {
    #[serde(rename = "plaintext")]
    PlainText,
    #[serde(rename = "markdown")]
    Markdown,
}

/// Marked string (for hover).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarkedString {
    /// Plain string
    String(String),
    /// Language-specific
    LanguageString { language: String, value: String },
}

/// Symbol kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

/// Document symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    /// Name
    pub name: String,
    /// Detail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Kind
    pub kind: SymbolKind,
    /// Whether deprecated
    #[serde(default)]
    pub deprecated: bool,
    /// Full range
    pub range: Range,
    /// Selection range
    pub selection_range: Range,
    /// Children
    #[serde(default)]
    pub children: Vec<DocumentSymbol>,
}

impl DocumentSymbol {
    /// Creates a new document symbol.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: SymbolKind,
        range: Range,
        selection_range: Range,
    ) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind,
            deprecated: false,
            range,
            selection_range,
            children: Vec::new(),
        }
    }
}

/// Symbol information (for workspace symbols).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    /// Name
    pub name: String,
    /// Kind
    pub kind: SymbolKind,
    /// Whether deprecated
    #[serde(default)]
    pub deprecated: bool,
    /// Location
    pub location: Location,
    /// Container name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

/// Text edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    /// Range to edit
    pub range: Range,
    /// New text
    pub new_text: String,
}

impl TextEdit {
    /// Creates a new text edit.
    #[must_use]
    pub fn new(range: Range, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }
}

/// Code action context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeActionContext {
    /// Diagnostics to provide actions for
    pub diagnostics: Vec<Diagnostic>,
    /// Requested action kinds
    #[serde(default)]
    pub only: Vec<String>,
}

/// Code action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAction {
    /// Title
    pub title: String,
    /// Kind (e.g., "quickfix", "refactor")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Diagnostics this action fixes
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    /// Whether this is preferred
    #[serde(default)]
    pub is_preferred: bool,
    /// Edits to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<WorkspaceEdit>,
}

impl CodeAction {
    /// Creates a new code action.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: None,
            diagnostics: Vec::new(),
            is_preferred: false,
            edit: None,
        }
    }

    /// Sets the kind.
    #[must_use]
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

/// Workspace edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEdit {
    /// Document changes
    #[serde(default)]
    pub changes: std::collections::HashMap<String, Vec<TextEdit>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position() {
        let pos = Position::new(10, 5);
        assert_eq!(pos.line, 10);
        assert_eq!(pos.character, 5);
    }

    #[test]
    fn test_position_ordering() {
        let p1 = Position::new(1, 5);
        let p2 = Position::new(1, 10);
        let p3 = Position::new(2, 0);

        assert!(p1 < p2);
        assert!(p2 < p3);
    }

    #[test]
    fn test_range() {
        let range = Range::line(5, 0, 10);
        assert_eq!(range.start.line, 5);
        assert_eq!(range.end.character, 10);
    }

    #[test]
    fn test_location() {
        let loc = Location::new("file:///test.rs", Range::at(10, 5));
        assert_eq!(loc.uri, "file:///test.rs");
    }

    #[test]
    fn test_diagnostic() {
        let diag = Diagnostic::new(Range::at(1, 1), "test error")
            .with_severity(DiagnosticSeverity::Warning)
            .with_source("test");

        assert_eq!(diag.message, "test error");
        assert_eq!(diag.severity, Some(DiagnosticSeverity::Warning));
    }

    #[test]
    fn test_completion_item() {
        let item = CompletionItem::new("foo")
            .with_kind(CompletionItemKind::Function)
            .with_detail("fn foo()");

        assert_eq!(item.label, "foo");
        assert_eq!(item.kind, Some(CompletionItemKind::Function));
    }

    #[test]
    fn test_document_symbol() {
        let symbol = DocumentSymbol::new(
            "MyStruct",
            SymbolKind::Struct,
            Range::line(1, 0, 10),
            Range::line(1, 0, 10),
        );

        assert_eq!(symbol.name, "MyStruct");
        assert_eq!(symbol.kind, SymbolKind::Struct);
    }

    #[test]
    fn test_serialization() {
        let pos = Position::new(10, 5);
        let json = serde_json::to_string(&pos).unwrap();
        let parsed: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, parsed);
    }
}
