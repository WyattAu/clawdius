//! Code actions and automated refactoring capabilities.
//!
//! This module provides intelligent code actions that can analyze code and suggest
//! or apply transformations such as refactoring, test generation, and quick fixes.

pub mod refactor;
pub mod tests;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use refactor::*;
pub use tests::*;

pub trait CodeAction: Send + Sync {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn applicability(&self, context: &ActionContext) -> Applicability;

    /// Execute the code action.
    ///
    /// # Errors
    ///
    /// Returns an error if the action cannot be applied or the transformation fails.
    fn execute(&self, context: &ActionContext) -> crate::Result<ActionEdit>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Variable,
    Class,
    Module,
    Trait,
    Struct,
    Enum,
    Method,
    Property,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    pub document: String,
    pub language: String,
    pub position: Position,
    pub selection: Option<String>,
    pub symbol_at_position: Option<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    QuickFix,
    Refactor,
    Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEdit {
    pub edits: Vec<TextEdit>,
    pub title: String,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    Always,
    WhenSelected,
    Never,
}

pub struct ActionRegistry {
    actions: Vec<Arc<dyn CodeAction>>,
}

impl ActionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn register(&mut self, action: Arc<dyn CodeAction>) {
        self.actions.push(action);
    }

    #[must_use]
    pub fn get_applicable_actions(&self, context: &ActionContext) -> Vec<Arc<dyn CodeAction>> {
        self.actions
            .iter()
            .filter(|action| action.applicability(context) != Applicability::Never)
            .cloned()
            .collect()
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(ExtractFunction));
        registry.register(Arc::new(ExtractVariable));
        registry.register(Arc::new(InlineVariable));
        registry.register(Arc::new(RenameSymbol));
        registry.register(Arc::new(MoveToModule));
        registry
    }
}
