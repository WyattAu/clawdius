//! Context compaction for managing token limits in context items
//!
//! This module provides automatic compaction of context items when approaching
//! token limits, with intelligent prioritization and summarization.

use serde::{Deserialize, Serialize};
use tiktoken_rs::CoreBPE;

use super::ContextItem;
use crate::error::Result;

/// Configuration for context compaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompactorConfig {
    /// Threshold percentage (0.0-1.0) at which to trigger compaction
    #[serde(default = "default_threshold")]
    pub threshold: f32,

    /// Number of recent items to always preserve
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,

    /// Maximum tokens allowed in context
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Provider-specific context limits
    #[serde(default)]
    pub provider_limits: ProviderTokenLimits,
}

fn default_threshold() -> f32 {
    0.85
}

fn default_keep_recent() -> usize {
    4
}

fn default_max_tokens() -> usize {
    200_000
}

impl Default for ContextCompactorConfig {
    fn default() -> Self {
        Self {
            threshold: default_threshold(),
            keep_recent: default_keep_recent(),
            max_tokens: default_max_tokens(),
            provider_limits: ProviderTokenLimits::default(),
        }
    }
}

/// Provider-specific token limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTokenLimits {
    /// Claude models (default: 200k)
    #[serde(default = "default_claude_limit")]
    pub claude: usize,

    /// GPT-4 models (default: 128k)
    #[serde(default = "default_gpt4_limit")]
    pub gpt4: usize,

    /// GPT-3.5 models (default: 16k)
    #[serde(default = "default_gpt35_limit")]
    pub gpt35: usize,

    /// Gemini models (default: 1M)
    #[serde(default = "default_gemini_limit")]
    pub gemini: usize,

    /// Default for unknown models
    #[serde(default = "default_model_limit")]
    pub default: usize,
}

impl Default for ProviderTokenLimits {
    fn default() -> Self {
        Self {
            claude: default_claude_limit(),
            gpt4: default_gpt4_limit(),
            gpt35: default_gpt35_limit(),
            gemini: default_gemini_limit(),
            default: default_model_limit(),
        }
    }
}

fn default_claude_limit() -> usize {
    200_000
}

fn default_gpt4_limit() -> usize {
    128_000
}

fn default_gpt35_limit() -> usize {
    16_384
}

fn default_gemini_limit() -> usize {
    1_000_000
}

fn default_model_limit() -> usize {
    100_000
}

/// Result of context compaction
#[derive(Debug, Clone)]
pub struct CompactResult {
    /// Number of items that were compacted
    pub compacted_count: usize,

    /// Token count before compaction
    pub tokens_before: usize,

    /// Token count after compaction
    pub tokens_after: usize,

    /// Items that were preserved
    pub preserved_count: usize,

    /// Summary of compacted items (if generated)
    pub summary: Option<String>,
}

/// Context compactor for managing token limits
pub struct ContextCompactor {
    config: ContextCompactorConfig,
    tokenizer: CoreBPE,
}

impl ContextCompactor {
    /// Create a new context compactor with configuration
    pub fn new(config: ContextCompactorConfig) -> Result<Self> {
        let tokenizer = tiktoken_rs::cl100k_base()
            .map_err(|e| crate::Error::Config(format!("Failed to initialize tokenizer: {e}")))?;

        Ok(Self { config, tokenizer })
    }

    /// Create a compactor with default configuration
    pub fn with_defaults() -> Result<Self> {
        Self::new(ContextCompactorConfig::default())
    }

    /// Create a compactor for a specific provider/model
    pub fn for_model(model: &str) -> Result<Self> {
        let limits = ProviderTokenLimits::default();
        let max_tokens = limits.get_limit(model);

        let config = ContextCompactorConfig {
            max_tokens,
            ..Default::default()
        };

        Self::new(config)
    }

    /// Check if compaction is needed
    #[must_use]
    pub fn should_compact(&self, current_tokens: usize) -> bool {
        let threshold_tokens = (self.config.max_tokens as f32 * self.config.threshold) as usize;
        current_tokens >= threshold_tokens
    }

    /// Estimate token count for text
    #[must_use]
    pub fn estimate_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_with_special_tokens(text).len()
    }

    /// Estimate token count for a context item
    #[must_use]
    pub fn estimate_item_tokens(&self, item: &ContextItem) -> usize {
        let text = item.to_formatted_string();
        self.estimate_tokens(&text)
    }

    /// Compact a list of context items
    pub fn compact(&self, items: Vec<ContextItem>) -> Result<(Vec<ContextItem>, CompactResult)> {
        let tokens_before: usize = items
            .iter()
            .map(|item| self.estimate_item_tokens(item))
            .sum();

        if !self.should_compact(tokens_before) {
            let item_count = items.len();
            return Ok((
                items,
                CompactResult {
                    compacted_count: 0,
                    tokens_before,
                    tokens_after: tokens_before,
                    preserved_count: item_count,
                    summary: None,
                },
            ));
        }

        let mut compacted_items = Vec::new();
        let mut preserved_items = Vec::new();
        let mut items_to_compact = Vec::new();

        let keep_from = items.len().saturating_sub(self.config.keep_recent);

        for (idx, item) in items.into_iter().enumerate() {
            if idx < keep_from {
                if self.should_preserve(&item) {
                    preserved_items.push(item);
                } else {
                    items_to_compact.push(item);
                }
            } else {
                preserved_items.push(item);
            }
        }

        if !items_to_compact.is_empty() {
            let summary = self.generate_summary(&items_to_compact)?;
            let summary_item = ContextItem::Symbol {
                name: "[Context Summary]".to_string(),
                location: "compacted".to_string(),
                content: summary.clone(),
            };
            compacted_items.push(summary_item);
        }

        compacted_items.extend(preserved_items);

        let tokens_after: usize = compacted_items
            .iter()
            .map(|item| self.estimate_item_tokens(item))
            .sum();

        let result = CompactResult {
            compacted_count: items_to_compact.len(),
            tokens_before,
            tokens_after,
            preserved_count: compacted_items.len(),
            summary: if items_to_compact.is_empty() {
                None
            } else {
                compacted_items.first().and_then(|item| {
                    if let ContextItem::Symbol { content, .. } = item {
                        Some(content.clone())
                    } else {
                        None
                    }
                })
            },
        };

        Ok((compacted_items, result))
    }

    /// Determine if an item should be preserved (not compacted)
    fn should_preserve(&self, item: &ContextItem) -> bool {
        matches!(
            item,
            ContextItem::GitDiff { .. } | ContextItem::Problems { .. }
        )
    }

    /// Generate a summary of items to be compacted
    fn generate_summary(&self, items: &[ContextItem]) -> Result<String> {
        let mut summary_parts = Vec::new();

        for item in items {
            match item {
                ContextItem::File { path, .. } => {
                    summary_parts.push(format!("- File: {path}"));
                }
                ContextItem::Folder { path, files } => {
                    summary_parts.push(format!("- Folder: {} ({} files)", path, files.len()));
                }
                ContextItem::Url { url, title, .. } => {
                    let title_str = title.as_deref().unwrap_or("Untitled");
                    summary_parts.push(format!("- URL: {url} ({title_str})"));
                }
                ContextItem::Symbol { name, location, .. } => {
                    summary_parts.push(format!("- Symbol: {name} @ {location}"));
                }
                ContextItem::Search { query, results } => {
                    summary_parts.push(format!(
                        "- Search: \"{}\" ({} results)",
                        query,
                        results.len()
                    ));
                }
                ContextItem::GitDiff { staged, .. } => {
                    let label = if *staged { "staged" } else { "unstaged" };
                    summary_parts.push(format!("- Git diff ({label})"));
                }
                ContextItem::GitLog { commits } => {
                    summary_parts.push(format!("- Git log ({} commits)", commits.len()));
                }
                ContextItem::Problems { diagnostics } => {
                    summary_parts.push(format!("- Problems ({} diagnostics)", diagnostics.len()));
                }
                ContextItem::Image {
                    path,
                    mime_type,
                    description,
                    ..
                } => {
                    let desc = description.as_deref().unwrap_or("no description");
                    summary_parts.push(format!("- Image: {path} ({mime_type}, {desc})"));
                }
                ContextItem::Screenshot {
                    source,
                    url,
                    timestamp,
                    ..
                } => {
                    let url_str = url.as_deref().unwrap_or("N/A");
                    summary_parts.push(format!("- Screenshot: {source} ({url_str}, {timestamp})"));
                }
            }
        }

        let summary = format!(
            "[Context Summary - {} items compacted]\n{}\n[End of summary]",
            items.len(),
            summary_parts.join("\n")
        );

        Ok(summary)
    }

    /// Get the maximum token limit
    #[must_use]
    pub fn max_tokens(&self) -> usize {
        self.config.max_tokens
    }

    /// Get the threshold percentage
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.config.threshold
    }

    /// Update the maximum token limit
    pub fn set_max_tokens(&mut self, max_tokens: usize) {
        self.config.max_tokens = max_tokens;
    }
}

impl ProviderTokenLimits {
    /// Get the token limit for a specific model
    #[must_use]
    pub fn get_limit(&self, model: &str) -> usize {
        let model_lower = model.to_lowercase();

        if model_lower.contains("claude-3") || model_lower.contains("claude-2") {
            self.claude
        } else if model_lower.contains("gpt-4") {
            self.gpt4
        } else if model_lower.contains("gpt-3.5") || model_lower.contains("gpt-35") {
            self.gpt35
        } else if model_lower.contains("gemini") {
            self.gemini
        } else {
            self.default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compactor_creation() {
        let compactor = ContextCompactor::with_defaults().unwrap();
        assert_eq!(compactor.max_tokens(), 200_000);
        assert_eq!(compactor.threshold(), 0.85);
    }

    #[test]
    fn test_model_specific_compactor() {
        let claude_compactor = ContextCompactor::for_model("claude-3-5-sonnet").unwrap();
        assert_eq!(claude_compactor.max_tokens(), 200_000);

        let gpt4_compactor = ContextCompactor::for_model("gpt-4-turbo").unwrap();
        assert_eq!(gpt4_compactor.max_tokens(), 128_000);
    }

    #[test]
    fn test_token_estimation() {
        let compactor = ContextCompactor::with_defaults().unwrap();

        let text = "Hello, world!";
        let tokens = compactor.estimate_tokens(text);
        assert!(tokens > 0);
        assert!(tokens < 20);
    }

    #[test]
    fn test_should_compact() {
        let compactor = ContextCompactor::with_defaults().unwrap();

        assert!(!compactor.should_compact(100_000));
        assert!(compactor.should_compact(180_000));
    }

    #[test]
    fn test_compact_empty_items() {
        let compactor = ContextCompactor::with_defaults().unwrap();
        let items = vec![];

        let (compacted, result) = compactor.compact(items).unwrap();

        assert_eq!(compacted.len(), 0);
        assert_eq!(result.compacted_count, 0);
        assert_eq!(result.preserved_count, 0);
    }

    #[test]
    fn test_compact_preserves_recent() {
        let config = ContextCompactorConfig {
            keep_recent: 2,
            max_tokens: 50,
            threshold: 0.5,
            ..Default::default()
        };
        let compactor = ContextCompactor::new(config).unwrap();

        let long_content =
            "This is a long piece of content that will generate many tokens. ".repeat(10);

        let items = vec![
            ContextItem::File {
                path: "file1.rs".to_string(),
                content: long_content.clone(),
                language: Some("rust".to_string()),
            },
            ContextItem::File {
                path: "file2.rs".to_string(),
                content: long_content.clone(),
                language: Some("rust".to_string()),
            },
            ContextItem::File {
                path: "file3.rs".to_string(),
                content: long_content.clone(),
                language: Some("rust".to_string()),
            },
            ContextItem::File {
                path: "file4.rs".to_string(),
                content: long_content.clone(),
                language: Some("rust".to_string()),
            },
        ];

        let (compacted, result) = compactor.compact(items).unwrap();

        assert!(
            compacted.len() <= 3,
            "Expected at most 3 items, got {}",
            compacted.len()
        );
        assert!(
            result.preserved_count >= 2,
            "Expected at least 2 preserved items, got {}",
            result.preserved_count
        );
    }

    #[test]
    fn test_provider_limits() {
        let limits = ProviderTokenLimits::default();

        assert_eq!(limits.get_limit("claude-3-5-sonnet"), 200_000);
        assert_eq!(limits.get_limit("gpt-4-turbo"), 128_000);
        assert_eq!(limits.get_limit("gpt-3.5-turbo"), 16_384);
        assert_eq!(limits.get_limit("gemini-pro"), 1_000_000);
        assert_eq!(limits.get_limit("unknown-model"), 100_000);
    }

    #[test]
    fn test_preserve_important_items() {
        let compactor = ContextCompactor::with_defaults().unwrap();

        let git_diff = ContextItem::GitDiff {
            diff: "diff content".to_string(),
            staged: false,
        };

        assert!(compactor.should_preserve(&git_diff));

        let file = ContextItem::File {
            path: "test.rs".to_string(),
            content: "content".to_string(),
            language: Some("rust".to_string()),
        };

        assert!(!compactor.should_preserve(&file));
    }
}
