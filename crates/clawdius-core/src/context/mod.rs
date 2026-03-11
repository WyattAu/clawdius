//! Context management for LLM interactions
//!
//! Provides context building, mention resolution, caching, and compaction.

#[cfg(feature = "vector-db")]
mod aggregator;
mod builder;
mod cache;
mod compactor;
mod mentions;

#[cfg(feature = "vector-db")]
pub use aggregator::{AggregatedContext, ContextAggregator, FileContext, SymbolContext};
pub use builder::{ContextBuilder, ContextContent};
pub use cache::{CacheStats, CachedContext, ContextCache};
pub use compactor::{CompactResult, ContextCompactor, ContextCompactorConfig, ProviderTokenLimits};
pub use mentions::{Mention, MentionParser, MentionResolver};

use serde::{Deserialize, Serialize};

/// A context item that can be included in prompts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextItem {
    /// File contents
    File {
        /// File path
        path: String,
        /// File contents
        content: String,
        /// Language identifier
        language: Option<String>,
    },
    /// Folder listing
    Folder {
        /// Folder path
        path: String,
        /// File names in folder
        files: Vec<String>,
    },
    /// URL content
    Url {
        /// URL
        url: String,
        /// Fetched content
        content: String,
        /// Page title
        title: Option<String>,
    },
    /// Workspace problems
    Problems {
        /// Diagnostics
        diagnostics: Vec<Diagnostic>,
    },
    /// Git diff
    GitDiff {
        /// Diff content
        diff: String,
        /// Whether staged
        staged: bool,
    },
    /// Git log
    GitLog {
        /// Commits
        commits: Vec<CommitInfo>,
    },
    /// Symbol definition
    Symbol {
        /// Symbol name
        name: String,
        /// Location
        location: String,
        /// Definition content
        content: String,
    },
    /// Search results
    Search {
        /// Query
        query: String,
        /// Results
        results: Vec<SearchResult>,
    },
}

impl ContextItem {
    /// Format the context item as a string
    #[must_use]
    pub fn to_formatted_string(&self) -> String {
        match self {
            ContextItem::File {
                path,
                content,
                language,
            } => {
                let lang = language.as_deref().unwrap_or("text");
                format!("@file:{path}\n```{lang}\n{content}\n```")
            }
            ContextItem::Folder { path, files } => {
                format!("@folder:{}\n{}", path, files.join("\n"))
            }
            ContextItem::Url {
                url,
                content,
                title,
            } => {
                let title_str = title.as_deref().unwrap_or("Untitled");
                format!("@url:{url}\n# {title_str}\n{content}")
            }
            ContextItem::Problems { diagnostics } => {
                let items: Vec<String> = diagnostics
                    .iter()
                    .map(|d| format!("  {}:{}: {}: {}", d.file, d.line, d.severity, d.message))
                    .collect();
                format!("@problems\n{}", items.join("\n"))
            }
            ContextItem::GitDiff { diff, staged } => {
                let label = if *staged { "staged" } else { "unstaged" };
                format!("@git:diff ({label})\n{diff}")
            }
            ContextItem::GitLog { commits } => {
                let items: Vec<String> = commits
                    .iter()
                    .map(|c| format!("  {} {} - {}", &c.hash[..7], c.author, c.message))
                    .collect();
                format!("@git:log\n{}", items.join("\n"))
            }
            ContextItem::Symbol {
                name,
                location,
                content,
            } => {
                format!("@symbol:{name} @ {location}\n{content}")
            }
            ContextItem::Search { query, results } => {
                let items: Vec<String> = results
                    .iter()
                    .map(|r| format!("  {}:{} - {}", r.file, r.line, r.content))
                    .collect();
                format!("@search:\"{}\"\n{}", query, items.join("\n"))
            }
        }
    }
}

/// A diagnostic from the workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// File path
    pub file: String,
    /// Line number
    pub line: usize,
    /// Column number
    pub column: usize,
    /// Severity
    pub severity: String,
    /// Message
    pub message: String,
    /// Source
    pub source: Option<String>,
}

/// Commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Commit hash
    pub hash: String,
    /// Author
    pub author: String,
    /// Commit message
    pub message: String,
    /// Timestamp
    pub timestamp: String,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// File path
    pub file: String,
    /// Line number
    pub line: usize,
    /// Content snippet
    pub content: String,
    /// Relevance score
    pub score: f32,
}

/// Context container with token budgeting
#[derive(Debug, Clone)]
pub struct Context {
    items: Vec<ContextItem>,
    max_tokens: usize,
    current_tokens: usize,
}

impl Context {
    /// Create a new context with a token budget
    #[must_use]
    pub fn new(max_tokens: usize) -> Self {
        Self {
            items: Vec::new(),
            max_tokens,
            current_tokens: 0,
        }
    }

    /// Add an item to the context
    pub fn add(&mut self, item: ContextItem) -> bool {
        let tokens = Self::estimate_tokens(&item);
        if self.current_tokens + tokens > self.max_tokens {
            return false;
        }
        self.current_tokens += tokens;
        self.items.push(item);
        true
    }

    /// Get remaining token budget
    #[must_use]
    pub fn remaining_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.current_tokens)
    }

    /// Get all items
    #[must_use]
    pub fn items(&self) -> &[ContextItem] {
        &self.items
    }

    /// Estimate tokens for an item
    fn estimate_tokens(item: &ContextItem) -> usize {
        item.to_formatted_string().len() / 4
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(100_000)
    }
}
