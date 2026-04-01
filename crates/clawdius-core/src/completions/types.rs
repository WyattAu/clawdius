//! Completion Types
//!
//! Types for inline completion requests and responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Position in a document (re-exported from LSP module for convenience)
pub use crate::lsp::Position;

/// A completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The document content (or relevant portion)
    pub document: String,
    /// Cursor position in the document
    pub position: Position,
    /// Programming language
    pub language: String,
    /// File path (optional, for context)
    #[serde(default)]
    pub file_path: Option<String>,
    /// Additional context
    #[serde(default)]
    pub context: CompletionContext,
    /// Trigger type
    #[serde(default)]
    pub trigger: CompletionTrigger,
}

impl CompletionRequest {
    /// Creates a new completion request.
    #[must_use]
    pub fn new(
        document: impl Into<String>,
        position: Position,
        language: impl Into<String>,
    ) -> Self {
        Self {
            document: document.into(),
            position,
            language: language.into(),
            file_path: None,
            context: CompletionContext::default(),
            trigger: CompletionTrigger::default(),
        }
    }

    /// Sets the file path.
    #[must_use]
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Sets the context.
    #[must_use]
    pub fn with_context(mut self, context: CompletionContext) -> Self {
        self.context = context;
        self
    }

    /// Extracts the prefix (code before cursor).
    #[must_use]
    pub fn prefix(&self) -> &str {
        let line = self.position.line as usize;
        let char = self.position.character as usize;

        let mut current_line = 0;
        let mut current_char = 0;

        for (idx, c) in self.document.char_indices() {
            if current_line == line && current_char == char {
                return &self.document[..idx];
            }
            if c == '\n' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }
        }

        &self.document
    }

    /// Extracts the suffix (code after cursor).
    #[must_use]
    pub fn suffix(&self) -> &str {
        let line = self.position.line as usize;
        let char = self.position.character as usize;

        let mut current_line = 0;
        let mut current_char = 0;

        for (idx, c) in self.document.char_indices() {
            if current_line == line && current_char == char {
                return &self.document[idx..];
            }
            if c == '\n' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }
        }

        ""
    }

    /// Gets the current line content.
    #[must_use]
    pub fn current_line(&self) -> &str {
        let line = self.position.line as usize;
        let lines: Vec<&str> = self.document.lines().collect();
        lines.get(line).unwrap_or(&"")
    }

    /// Gets the current line up to cursor.
    #[must_use]
    pub fn current_line_prefix(&self) -> &str {
        let line_content = self.current_line();
        let char = self.position.character as usize;
        if char >= line_content.len() {
            line_content
        } else {
            let bytes = line_content.as_bytes();
            let char_indices: Vec<usize> = line_content.char_indices().map(|(i, _)| i).collect();
            let end_byte = char_indices.get(char).copied().unwrap_or(bytes.len());
            &line_content[..end_byte]
        }
    }
}

/// Completion context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionContext {
    /// Related files content
    #[serde(default)]
    pub related_files: HashMap<String, String>,
    /// Recent edits
    #[serde(default)]
    pub recent_edits: Vec<String>,
    /// Imported modules
    #[serde(default)]
    pub imports: Vec<String>,
    /// Function/method signatures in scope
    #[serde(default)]
    pub signatures: Vec<String>,
    /// Maximum context tokens
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
}

fn default_max_context_tokens() -> usize {
    2048
}

impl CompletionContext {
    /// Creates empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a related file.
    #[must_use]
    pub fn with_related_file(
        mut self,
        path: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.related_files.insert(path.into(), content.into());
        self
    }

    /// Adds an import.
    #[must_use]
    pub fn with_import(mut self, import: impl Into<String>) -> Self {
        self.imports.push(import.into());
        self
    }
}

/// Completion trigger type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompletionTrigger {
    /// Automatic trigger (typing)
    #[default]
    Automatic,
    /// Manual trigger (keyboard shortcut)
    Manual,
    /// Triggered by special character (e.g., `.`)
    TriggerCharacter(char),
    /// Inline suggestion requested
    Inline,
}

/// A completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The completion text
    pub text: String,
    /// Display text (if different from insert text)
    #[serde(default)]
    pub display_text: Option<String>,
    /// Confidence score (0.0 - 1.0)
    #[serde(default)]
    pub confidence: f32,
    /// Whether this is a complete statement
    #[serde(default)]
    pub is_complete: bool,
    /// Source of the completion
    #[serde(default)]
    pub source: CompletionSource,
    /// Additional completions (alternatives)
    #[serde(default)]
    pub alternatives: Vec<CompletionResponse>,
    /// Cache key (for caching)
    #[serde(default)]
    pub cache_key: Option<String>,
}

impl CompletionResponse {
    /// Creates a new completion response.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            display_text: None,
            confidence: 0.8,
            is_complete: false,
            source: CompletionSource::Llm,
            alternatives: Vec::new(),
            cache_key: None,
        }
    }

    /// Sets the display text.
    #[must_use]
    pub fn with_display_text(mut self, text: impl Into<String>) -> Self {
        self.display_text = Some(text.into());
        self
    }

    /// Sets the confidence.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Marks as complete.
    #[must_use]
    pub fn complete(mut self) -> Self {
        self.is_complete = true;
        self
    }

    /// Adds an alternative.
    #[must_use]
    pub fn with_alternative(mut self, alt: CompletionResponse) -> Self {
        self.alternatives.push(alt);
        self
    }
}

impl Default for CompletionResponse {
    fn default() -> Self {
        Self::new("")
    }
}

/// Source of completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CompletionSource {
    /// From LLM
    #[default]
    Llm,
    /// From cache
    Cache,
    /// From template/pattern
    Template,
    /// From LSP
    Lsp,
}

/// Fill-in-the-Middle (FIM) template for different LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FimTemplate {
    /// Prefix marker
    pub prefix: String,
    /// Suffix marker
    pub suffix: String,
    /// Middle marker (where completion goes)
    pub middle: String,
}

impl FimTemplate {
    /// Creates a new FIM template.
    #[must_use]
    pub fn new(
        prefix: impl Into<String>,
        suffix: impl Into<String>,
        middle: impl Into<String>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            suffix: suffix.into(),
            middle: middle.into(),
        }
    }

    /// Codellama/CodeLlama FIM format
    #[must_use]
    pub fn codellama() -> Self {
        Self::new("<PRE>", "<SUF>", "<MID>")
    }

    /// DeepSeek Coder FIM format
    #[must_use]
    pub fn deepseek() -> Self {
        Self::new("<｜fim▁begin｜>", "<｜fim▁hole｜>", "<｜fim▁end｜>")
    }

    /// StarCoder FIM format
    #[must_use]
    pub fn starcoder() -> Self {
        Self::new("<fim_prefix>", "<fim_suffix>", "<fim_middle>")
    }

    /// Simple format (used by many models)
    #[must_use]
    pub fn simple() -> Self {
        Self::new("<PRE>", "<SUF>", "<MID>")
    }

    /// Formats the prompt for FIM completion.
    #[must_use]
    pub fn format(&self, prefix: &str, suffix: &str) -> String {
        format!(
            "{}{}{}{}{}",
            self.prefix, prefix, self.suffix, suffix, self.middle
        )
    }
}

impl Default for FimTemplate {
    fn default() -> Self {
        Self::codellama()
    }
}

/// Trait for completion providers.
#[async_trait::async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Get completions for a request.
    async fn complete(&self, request: &CompletionRequest) -> crate::Result<CompletionResponse>;

    /// Get streaming completions.
    async fn complete_stream(
        &self,
        request: &CompletionRequest,
    ) -> crate::Result<tokio::sync::mpsc::Receiver<CompletionResponse>>;

    /// Check if the provider is ready.
    fn is_ready(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request() {
        let doc = "fn main() {\n    println!\n}";
        let request = CompletionRequest::new(doc, Position::new(1, 12), "rust");

        assert_eq!(request.language, "rust");
        assert!(request.prefix().contains("fn main"));
        assert!(request.suffix().contains("\n}"));
    }

    #[test]
    fn test_current_line() {
        let doc = "line1\nline2\nline3";
        let request = CompletionRequest::new(doc, Position::new(1, 2), "text");

        assert_eq!(request.current_line(), "line2");
    }

    #[test]
    fn test_fim_template() {
        let template = FimTemplate::codellama();
        let prompt = template.format("fn main() {", "}");

        assert!(prompt.contains("<PRE>"));
        assert!(prompt.contains("<SUF>"));
        assert!(prompt.contains("<MID>"));
    }

    #[test]
    fn test_completion_response() {
        let response = CompletionResponse::new("let x = 5;")
            .with_confidence(0.9)
            .complete();

        assert_eq!(response.text, "let x = 5;");
        assert_eq!(response.confidence, 0.9);
        assert!(response.is_complete);
    }
}
