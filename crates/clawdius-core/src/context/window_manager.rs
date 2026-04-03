//! Context window manager for smart context assembly
//!
//! Provides token-budgeted file selection, context formatting, and message
//! assembly. This is a heuristic manager — not a replacement for vector search.

use std::collections::HashSet;
use std::time::SystemTime;

use tiktoken_rs::CoreBPE;

use super::ContextItem;
use crate::error::Result;
use crate::llm::{ChatMessage, ChatRole};

const DEFAULT_MAX_TOKENS: usize = 128_000;
const DEFAULT_RESERVED_FOR_RESPONSE: usize = 4096;
const CHARS_PER_TOKEN_FALLBACK: usize = 4;

/// Metadata about a file available for context selection.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File path
    pub path: String,
    /// File contents
    pub content: String,
    /// Language identifier
    pub language: Option<String>,
    /// Last modification time
    pub last_modified: Option<SystemTime>,
}

impl FileInfo {
    /// Create a new file info.
    #[must_use]
    pub fn new(path: String, content: String) -> Self {
        Self {
            path,
            content,
            language: None,
            last_modified: None,
        }
    }

    /// Set the language.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the last modified time.
    #[must_use]
    pub fn with_last_modified(mut self, ts: SystemTime) -> Self {
        self.last_modified = Some(ts);
        self
    }
}

/// Manages the context window budget for LLM interactions.
///
/// Handles token budgeting across system prompt, context, and user messages,
/// selects the most relevant files for a query, and assembles the final
/// message list.
pub struct ContextWindowManager {
    max_tokens: usize,
    system_prompt_tokens: usize,
    reserved_for_response: usize,
    tokenizer: CoreBPE,
}

impl ContextWindowManager {
    /// Create a new context window manager.
    ///
    /// # Errors
    ///
    /// Returns an error if the tiktoken tokenizer cannot be initialized.
    pub fn new(max_tokens: usize, reserved_for_response: usize) -> Result<Self> {
        let tokenizer = tiktoken_rs::cl100k_base()
            .map_err(|e| crate::Error::Config(format!("Failed to initialize tokenizer: {e}")))?;
        Ok(Self {
            max_tokens,
            system_prompt_tokens: 0,
            reserved_for_response,
            tokenizer,
        })
    }

    /// Create a manager with default limits (128k max, 4096 reserved).
    ///
    /// # Errors
    ///
    /// Returns an error if the tiktoken tokenizer cannot be initialized.
    pub fn with_defaults() -> Result<Self> {
        Self::new(DEFAULT_MAX_TOKENS, DEFAULT_RESERVED_FOR_RESPONSE)
    }

    /// Set the system prompt, measuring its token cost.
    pub fn set_system_prompt(&mut self, system: &str) {
        self.system_prompt_tokens = self.tokenizer.encode_with_special_tokens(system).len();
    }

    /// Tokens available for context items after system prompt and response reservation.
    #[must_use]
    pub fn available_for_context(&self) -> usize {
        self.max_tokens
            .saturating_sub(self.system_prompt_tokens)
            .saturating_sub(self.reserved_for_response)
    }

    /// Count tokens in a string using tiktoken (cl100k_base).
    #[must_use]
    pub fn estimate_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_with_special_tokens(text).len()
    }

    /// Select the most relevant files within a token budget.
    ///
    /// Scoring heuristic (higher is better):
    /// - +100 if the file path appears in the query
    /// - +50 if recently modified (within 24h)
    /// - +20 * (1 / (1 + file_size_kb)) — prefer smaller files
    /// - +1 for each query token found in the file content
    #[must_use]
    pub fn select_files(
        &self,
        query: &str,
        all_files: Vec<FileInfo>,
        budget: usize,
    ) -> Vec<FileInfo> {
        let query_tokens: HashSet<String> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|s| s.len() > 1)
            .map(|s| s.to_lowercase())
            .collect();

        let now = SystemTime::now();
        let one_day = std::time::Duration::from_secs(86_400);

        let mut scored: Vec<(usize, FileInfo)> = all_files
            .into_iter()
            .map(|file| {
                let mut score: usize = 0;
                let query_lower = query.to_lowercase();

                if query_lower.contains(&file.path.to_lowercase()) {
                    score += 100;
                }

                if let Some(modified) = file.last_modified {
                    if now.duration_since(modified).is_ok_and(|d| d < one_day) {
                        score += 50;
                    }
                }

                let size_kb = file.content.len() / 1024;
                score += 20 * 1000 / (size_kb + 1);

                let content_lower = file.content.to_lowercase();
                for token in &query_tokens {
                    if content_lower.contains(token.as_str()) {
                        score += 1;
                    }
                }

                (score, file)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        let mut selected = Vec::new();
        let mut used_tokens: usize = 0;

        for (_, file) in scored {
            let file_text = format!(
                "@file:{}\n```{}\n{}\n```",
                file.path,
                file.language.as_deref().unwrap_or("text"),
                file.content
            );
            let tokens = self.estimate_tokens(&file_text);
            if used_tokens + tokens <= budget {
                used_tokens += tokens;
                selected.push(file);
            }
        }

        selected
    }

    /// Format context items into a single string, truncating if necessary.
    ///
    /// If the formatted output exceeds the available context budget, items are
    /// trimmed from the end until the budget is satisfied.
    pub fn build_context_message(&self, items: &[ContextItem]) -> Result<String> {
        let budget = self.available_for_context();
        let mut parts: Vec<String> = Vec::new();
        let mut used_tokens: usize = 0;

        for item in items {
            let formatted = item.to_formatted_string();
            let tokens = self.estimate_tokens(&formatted);
            if used_tokens + tokens > budget {
                break;
            }
            used_tokens += tokens;
            parts.push(formatted);
        }

        Ok(parts.join("\n\n"))
    }

    /// Assemble the final message list, ensuring total tokens stay within budget.
    ///
    /// If the combined system + context + user tokens would exceed the window,
    /// the context is progressively trimmed from the end.
    pub fn format_messages(&self, system: &str, context: &str, user: &str) -> Vec<ChatMessage> {
        let system_tokens = self.estimate_tokens(system);
        let user_tokens = self.estimate_tokens(user);
        let total_budget = self.max_tokens.saturating_sub(self.reserved_for_response);
        let context_budget = total_budget
            .saturating_sub(system_tokens)
            .saturating_sub(user_tokens);

        let context_tokens = self.estimate_tokens(context);
        let final_context = if context_tokens > context_budget {
            self.truncate_to_tokens(context, context_budget)
        } else {
            context.to_string()
        };

        vec![
            ChatMessage {
                role: ChatRole::System,
                content: system.to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: format!("<context>\n{final_context}\n</context>\n\n{user}"),
            },
        ]
    }

    fn truncate_to_tokens(&self, text: &str, max_tokens: usize) -> String {
        let tokens = self.tokenizer.encode_with_special_tokens(text);
        if tokens.len() <= max_tokens {
            return text.to_string();
        }
        let decoder = tiktoken_rs::cl100k_base();
        let truncated_tokens = &tokens[..max_tokens];
        match decoder.and_then(|d| d.decode(truncated_tokens.to_vec())) {
            Ok(s) => s,
            Err(_) => {
                let char_budget = max_tokens * CHARS_PER_TOKEN_FALLBACK;
                text.chars().take(char_budget).collect()
            },
        }
    }
}

impl Default for ContextWindowManager {
    fn default() -> Self {
        Self::with_defaults().expect("default tokenizer should always initialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_creation() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        assert_eq!(mgr.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(mgr.reserved_for_response, DEFAULT_RESERVED_FOR_RESPONSE);
    }

    #[test]
    fn test_available_for_context() {
        let mut mgr = ContextWindowManager::new(100_000, 4_096).unwrap();
        assert_eq!(mgr.available_for_context(), 95_904);

        mgr.set_system_prompt("You are a helpful assistant.");
        let sys_tokens = mgr.estimate_tokens("You are a helpful assistant.");
        assert_eq!(mgr.available_for_context(), 95_904 - sys_tokens);
    }

    #[test]
    fn test_estimate_tokens() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        let tokens = mgr.estimate_tokens("Hello, world!");
        assert!(tokens > 0);
        assert!(tokens < 20);
    }

    #[test]
    fn test_select_files_respects_budget() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        let budget = 500;

        let files = vec![
            FileInfo::new("src/main.rs".into(), "x".repeat(5000)),
            FileInfo::new("src/utils.rs".into(), "fn helper() {}".into()),
            FileInfo::new("README.md".into(), "A project description.".into()),
        ];

        let selected = mgr.select_files("helper function utils", files, budget);
        let total: usize = selected
            .iter()
            .map(|f| mgr.estimate_tokens(&f.content))
            .sum();
        assert!(total <= budget + 50);
    }

    #[test]
    fn test_select_files_prioritizes_path_match() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        let budget = 50_000;

        let files = vec![
            FileInfo::new("src/main.rs".into(), "some content".into()),
            FileInfo::new("src/utils.rs".into(), "some content".into()),
            FileInfo::new("src/helper.rs".into(), "some content".into()),
        ];

        let selected = mgr.select_files("src/utils.rs fix bug", files, budget);
        assert_eq!(selected[0].path, "src/utils.rs");
    }

    #[test]
    fn test_build_context_message() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        let items = vec![
            ContextItem::File {
                path: "test.rs".into(),
                content: "fn main() {}".into(),
                language: Some("rust".into()),
            },
            ContextItem::Folder {
                path: "src".into(),
                files: vec!["main.rs".into(), "lib.rs".into()],
            },
        ];

        let msg = mgr.build_context_message(&items).unwrap();
        assert!(msg.contains("@file:test.rs"));
        assert!(msg.contains("@folder:src"));
    }

    #[test]
    fn test_format_messages_within_budget() {
        let mgr = ContextWindowManager::new(100_000, 4_096).unwrap();
        let msgs = mgr.format_messages("Be helpful.", "some context", "do stuff");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, ChatRole::System);
        assert_eq!(msgs[1].role, ChatRole::User);
        assert!(msgs[1].content.contains("<context>"));
        assert!(msgs[1].content.contains("</context>"));
    }

    #[test]
    fn test_format_messages_truncates_context() {
        let mgr = ContextWindowManager::new(1_000, 100).unwrap();
        let long_context = "x".repeat(10_000);
        let msgs = mgr.format_messages("sys", &long_context, "user");
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn test_select_files_empty() {
        let mgr = ContextWindowManager::with_defaults().unwrap();
        let selected = mgr.select_files("query", vec![], 1000);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_file_info_builders() {
        let info = FileInfo::new("test.rs".into(), "content".into())
            .with_language("rust")
            .with_last_modified(SystemTime::now());
        assert_eq!(info.path, "test.rs");
        assert_eq!(info.language.as_deref(), Some("rust"));
        assert!(info.last_modified.is_some());
    }
}
