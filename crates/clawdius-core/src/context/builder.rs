//! Context builder for assembling context items

use super::{Context, ContextItem};

/// Context content with metadata
#[derive(Debug, Clone)]
pub struct ContextContent {
    /// Formatted content string
    pub content: String,
    /// Token count estimate
    pub tokens: usize,
}

impl ContextContent {
    /// Create new context content
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let tokens = content.len() / 4; // Rough estimate
        Self { content, tokens }
    }
}

/// Context builder
pub struct ContextBuilder {
    items: Vec<ContextItem>,
    max_tokens: usize,
}

impl ContextBuilder {
    /// Create a new context builder
    #[must_use]
    pub fn new(max_tokens: usize) -> Self {
        Self {
            items: Vec::new(),
            max_tokens,
        }
    }

    /// Add a context item
    #[must_use]
    pub fn add(mut self, item: ContextItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add multiple items
    #[must_use]
    pub fn add_all(mut self, items: Vec<ContextItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Build the context
    pub fn build(self) -> Context {
        let mut context = Context::new(self.max_tokens);

        for item in self.items {
            if !context.add(item) {
                tracing::warn!(
                    remaining = context.remaining_tokens(),
                    "Context full, skipping item"
                );
                break;
            }
        }

        context
    }

    /// Build formatted content string
    #[must_use]
    pub fn build_content(&self) -> ContextContent {
        let parts: Vec<String> = self
            .items
            .iter()
            .map(super::ContextItem::to_formatted_string)
            .collect();

        let content = parts.join("\n---\n");
        let tokens = content.len() / 4;

        ContextContent { content, tokens }
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new(100_000)
    }
}
