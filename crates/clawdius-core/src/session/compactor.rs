//! Context compaction for managing token limits.
//!
//! The compactor monitors session token usage and, when it exceeds a configured
//! threshold, summarizes older messages using either:
//! - **Abstractive** summarization via an LLM (preferred, when available)
//! - **Extractive** summarization via truncation (fallback, when no LLM)
//!
//! The most recent messages are always preserved to maintain conversational flow.

use std::sync::Arc;

use tiktoken_rs::CoreBPE;

use super::{Message, MessageRole, Session};
use crate::config::SessionConfig;
use crate::error::Result;
use crate::llm::providers::LlmClient;

/// Compaction configuration
#[derive(Debug, Clone)]
pub struct CompactConfig {
    /// Trigger at this % of context window
    pub threshold_percent: f32,
    /// Keep this many recent messages
    pub keep_recent: usize,
    /// Minimum messages before compacting
    pub min_messages: usize,
    /// Summarization model (cheaper/smaller) — if None, uses the session's model
    pub summary_model: Option<String>,
    /// Maximum length for extractive (truncation) fallback summary in chars
    pub extractive_max_chars: usize,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 0.85,
            keep_recent: 4,
            min_messages: 10,
            summary_model: None,
            extractive_max_chars: 4000,
        }
    }
}

impl From<SessionConfig> for CompactConfig {
    fn from(config: SessionConfig) -> Self {
        Self {
            threshold_percent: config.compact_threshold,
            keep_recent: config.keep_recent,
            min_messages: config.min_messages,
            summary_model: None,
            extractive_max_chars: 4000,
        }
    }
}

/// Result of compaction
#[derive(Debug, Clone)]
pub struct CompactSummary {
    /// Number of messages summarized
    pub summarized_count: usize,
    /// Tokens before compaction
    pub tokens_before: usize,
    /// Tokens after compaction
    pub tokens_after: usize,
    /// Summary content
    pub summary: String,
    /// Whether LLM-based abstractive summarization was used
    pub used_llm: bool,
}

/// Context compactor with optional LLM-based summarization.
pub struct Compactor {
    config: CompactConfig,
    tokenizer: CoreBPE,
    llm: Option<Arc<dyn LlmClient>>,
}

impl Compactor {
    /// Create a new compactor without LLM (extractive fallback only).
    pub fn new(config: impl Into<CompactConfig>) -> Self {
        Self {
            config: config.into(),
            tokenizer: tiktoken_rs::cl100k_base().expect("failed to load tokenizer"),
            llm: None,
        }
    }

    /// Create a new compactor with LLM client for abstractive summarization.
    pub fn with_llm(config: impl Into<CompactConfig>, llm: Arc<dyn LlmClient>) -> Self {
        Self {
            config: config.into(),
            tokenizer: tiktoken_rs::cl100k_base().expect("failed to load tokenizer"),
            llm: Some(llm),
        }
    }

    /// Check if session needs compaction
    #[must_use]
    pub fn needs_compaction(&self, session: &Session) -> bool {
        if session.messages.len() < self.config.min_messages {
            return false;
        }
        let tokens = self.count_tokens(session);
        let limit = self.get_context_limit(session);
        tokens as f32 / limit as f32 >= self.config.threshold_percent
    }

    /// Perform compaction on session
    pub async fn compact(&self, session: &mut Session) -> Result<CompactSummary> {
        let tokens_before = self.count_tokens(session);
        let keep_from = session.messages.len().saturating_sub(self.config.keep_recent);
        if keep_from == 0 {
            return Ok(CompactSummary {
                summarized_count: 0,
                tokens_before,
                tokens_after: tokens_before,
                summary: "Nothing to compact".to_string(),
                used_llm: false,
            });
        }
        let old_messages: Vec<&Message> = session.messages[..keep_from].iter().collect();
        let summary_result = self.generate_summary(&old_messages).await?;
        let summary_message = Message::system(format!(
            "[Previous context summarized]\n\n{}\n\n[End of summary]",
            summary_result.text
        ));
        let recent_messages: Vec<Message> = session.messages[keep_from..].to_vec();
        session.messages.clear();
        session.messages.push(summary_message);
        session.messages.extend(recent_messages);
        let tokens_after = self.count_tokens(session);
        Ok(CompactSummary {
            summarized_count: keep_from,
            tokens_before,
            tokens_after,
            summary: summary_result.text,
            used_llm: summary_result.used_llm,
        })
    }

    /// Perform compaction only if needed.
    pub async fn compact_if_needed(&self, session: &mut Session) -> Result<Option<CompactSummary>> {
        if self.needs_compaction(session) {
            let summary = self.compact(session).await?;
            tracing::info!(
                summarized = summary.summarized_count,
                tokens_before = summary.tokens_before,
                tokens_after = summary.tokens_after,
                used_llm = summary.used_llm,
                "context compaction performed"
            );
            Ok(Some(summary))
        } else {
            Ok(None)
        }
    }

    fn count_tokens(&self, session: &Session) -> usize {
        session.messages.iter().map(|msg| {
            self.tokenizer.encode_with_special_tokens(msg.as_text().unwrap_or("")).len()
        }).sum()
    }

    fn get_context_limit(&self, session: &Session) -> usize {
        match session.meta.model.as_deref() {
            Some(m) if m.contains("claude-3") => 200_000,
            Some(m) if m.contains("claude-2") => 100_000,
            Some(m) if m.contains("gpt-4") => 128_000,
            Some(m) if m.contains("gpt-3.5") => 16_384,
            Some(m) if m.contains("gemini") => 1_000_000,
            Some(m) if m.contains("glm") => 128_000,
            _ => 100_000,
        }
    }

    async fn generate_summary(&self, messages: &[&Message]) -> Result<SummaryResult> {
        let formatted = Self::format_messages_for_summary(messages);
        if let Some(ref llm) = self.llm {
            match self.summarize_with_llm(llm, &formatted).await {
                Ok(summary) => return Ok(SummaryResult { text: summary, used_llm: true }),
                Err(e) => tracing::warn!("LLM summarization failed: {}, falling back to extractive", e),
            }
        }
        Ok(SummaryResult {
            text: Self::extractive_summary(&formatted, self.config.extractive_max_chars),
            used_llm: false,
        })
    }

    fn format_messages_for_summary(messages: &[&Message]) -> Vec<FormattedMessage> {
        messages.iter().filter_map(|msg| {
            let role = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
                MessageRole::Tool => "Tool",
            };
            let content = msg.as_text()?;
            if content.trim().len() < 2 { return None; }
            let truncated = if content.len() > 2000 {
                format!("{}... [truncated at 2000 chars]", &content[..2000])
            } else { content.to_string() };
            Some(FormattedMessage { role: role.to_string(), content: truncated })
        }).collect()
    }

    async fn summarize_with_llm(&self, llm: &Arc<dyn LlmClient>, formatted: &[FormattedMessage]) -> Result<String> {
        let conversation_text = formatted.iter().map(|m| format!("{}: {}", m.role, m.content)).collect::<Vec<_>>().join("\n\n");
        let system_prompt = crate::llm::ChatMessage {
            role: crate::llm::ChatRole::System,
            content: r#"You are a context compaction assistant. Summarize this conversation, preserving ALL critical info: task/objective, files modified, key decisions, progress state, errors and fixes, code patterns. Discard verbose output. Under 2000 words."#.to_string(),
        };
        let user_prompt = crate::llm::ChatMessage {
            role: crate::llm::ChatRole::User,
            content: format!("Summarize:\n\n{conversation_text}"),
        };
        let response = llm.chat(vec![system_prompt, user_prompt]).await?;
        Ok(if response.len() > 6000 { format!("{}... [truncated]", &response[..6000]) } else { response })
    }

    fn extractive_summary(formatted: &[FormattedMessage], max_chars: usize) -> String {
        let mut parts = Vec::new();
        let mut total_len = 0;
        let msg_count = formatted.len();
        for msg in formatted.iter().rev() {
            let entry = format!("{}: {}", msg.role, msg.content);
            if total_len + entry.len() + 2 <= max_chars {
                total_len += entry.len() + 2;
                parts.push(entry);
            }
        }
        parts.reverse();
        if parts.len() < msg_count {
            parts.insert(0, format!("[Oldest {} of {} messages omitted]", msg_count - parts.len(), msg_count));
        }
        let mut result = parts.join("\n\n");
        if result.len() > max_chars { result.truncate(max_chars); result.push_str("\n\n[truncated]"); }
        result
    }
}

#[derive(Debug, Clone)]
struct FormattedMessage { role: String, content: String }
struct SummaryResult { text: String, used_llm: bool }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_compaction() {
        let compactor = Compactor::new(CompactConfig {
            threshold_percent: 0.5, keep_recent: 2, min_messages: 3,
            summary_model: None, extractive_max_chars: 4000,
        });
        let mut session = Session::new();
        session.meta.model = Some("claude-3-5-sonnet".to_string());
        session.add_message(Message::user("Hello"));
        assert!(!compactor.needs_compaction(&session));
        session.add_message(Message::user("World"));
        assert!(!compactor.needs_compaction(&session));
        session.add_message(Message::user("Test"));
        assert!(!compactor.needs_compaction(&session));
    }

    #[test]
    fn test_extractive_summary_basic() {
        let messages = vec![
            FormattedMessage { role: "User".into(), content: "Please create a hello world function".into() },
            FormattedMessage { role: "Assistant".into(), content: "fn hello() { println!(\"Hello\"); }".into() },
        ];
        let summary = Compactor::extractive_summary(&messages, 500);
        assert!(summary.contains("hello world"));
    }

    #[test]
    fn test_format_messages_skips_empty() {
        let mut session = Session::new();
        session.add_message(Message::user("Task: implement feature"));
        session.add_message(Message::user(""));
        session.add_message(Message::assistant("Done"));
        let messages: Vec<&Message> = session.messages.iter().collect();
        let formatted = Compactor::format_messages_for_summary(&messages);
        assert_eq!(formatted.len(), 2);
    }

    #[test]
    fn test_compact_preserves_recent() {
        let compactor = Compactor::new(CompactConfig {
            threshold_percent: 0.01, keep_recent: 2, min_messages: 3,
            summary_model: None, extractive_max_chars: 4000,
        });
        let mut session = Session::new();
        session.meta.model = Some("claude-3-5-sonnet".to_string());
        for i in 0..5 { session.add_message(Message::user(format!("Message {i}"))); }
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(compactor.compact(&mut session)).unwrap();
        assert_eq!(result.summarized_count, 3);
        assert!(!result.used_llm);
        assert_eq!(session.messages.len(), 3);
    }
}
