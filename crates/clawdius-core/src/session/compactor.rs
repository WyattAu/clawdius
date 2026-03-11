//! Context compaction for managing token limits

use tiktoken_rs::CoreBPE;

use super::{Message, MessageRole, Session};
use crate::config::SessionConfig;
use crate::error::Result;

/// Compaction configuration
#[derive(Debug, Clone)]
pub struct CompactConfig {
    /// Trigger at this % of context window
    pub threshold_percent: f32,
    /// Keep this many recent messages
    pub keep_recent: usize,
    /// Minimum messages before compacting
    pub min_messages: usize,
    /// Summarization model (cheaper/smaller)
    pub summary_model: Option<String>,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 0.85,
            keep_recent: 4,
            min_messages: 10,
            summary_model: None,
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
}

/// Context compactor
pub struct Compactor {
    config: CompactConfig,
    tokenizer: CoreBPE,
}

impl Compactor {
    /// Create a new compactor
    pub fn new(config: impl Into<CompactConfig>) -> Self {
        Self {
            config: config.into(),
            tokenizer: tiktoken_rs::cl100k_base().expect("failed to load tokenizer"),
        }
    }

    /// Check if session needs compaction
    pub fn needs_compaction(&self, session: &Session) -> bool {
        // Not enough messages
        if session.messages.len() < self.config.min_messages {
            return false;
        }

        // Calculate current token usage
        let tokens = self.count_tokens(session);
        let limit = self.get_context_limit(session);

        tokens as f32 / limit as f32 >= self.config.threshold_percent
    }

    /// Perform compaction on session
    pub async fn compact(&self, session: &mut Session) -> Result<CompactSummary> {
        let tokens_before = self.count_tokens(session);

        // Determine messages to summarize (keep recent ones)
        let keep_from = session
            .messages
            .len()
            .saturating_sub(self.config.keep_recent);

        if keep_from == 0 {
            return Ok(CompactSummary {
                summarized_count: 0,
                tokens_before,
                tokens_after: tokens_before,
                summary: "Nothing to compact".to_string(),
            });
        }

        // Generate summary of old messages
        let old_messages: Vec<&Message> = session.messages[..keep_from].iter().collect();
        let summary = self.generate_summary(&old_messages).await?;

        // Create summary message
        let summary_message = Message::system(format!(
            "[Previous context summarized]\n\n{}\n\n[End of summary]",
            summary
        ));

        // Replace old messages with summary
        let recent_messages: Vec<Message> = session.messages[keep_from..].to_vec();
        session.messages.clear();
        session.messages.push(summary_message);
        session.messages.extend(recent_messages);

        let tokens_after = self.count_tokens(session);
        let summarized_count = keep_from;

        Ok(CompactSummary {
            summarized_count,
            tokens_before,
            tokens_after,
            summary,
        })
    }

    /// Count tokens in session
    fn count_tokens(&self, session: &Session) -> usize {
        session
            .messages
            .iter()
            .map(|msg| {
                let text = msg.as_text().unwrap_or("");
                self.tokenizer.encode_with_special_tokens(text).len()
            })
            .sum()
    }

    /// Get context limit for session's model
    fn get_context_limit(&self, session: &Session) -> usize {
        // Default limits by model
        match session.meta.model.as_deref() {
            Some(m) if m.contains("claude-3") => 200_000,
            Some(m) if m.contains("claude-2") => 100_000,
            Some(m) if m.contains("gpt-4") => 128_000,
            Some(m) if m.contains("gpt-3.5") => 16_384,
            Some(m) if m.contains("gemini") => 1_000_000,
            _ => 100_000, // Default
        }
    }

    /// Generate summary of messages
    async fn generate_summary(&self, messages: &[&Message]) -> Result<String> {
        // Format messages for summarization
        let formatted: Vec<String> = messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "User",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                    MessageRole::Tool => "Tool",
                };
                let content = msg.as_text().unwrap_or("[non-text content]");
                format!("{}: {}", role, content)
            })
            .collect();

        let all_content = formatted.join("\n\n");

        // For now, create a simple extractive summary
        // In production, this would call the LLM with a summarization prompt
        let summary = if all_content.len() > 2000 {
            format!(
                "{}...\n\n[Content truncated - {} messages summarized]",
                &all_content[..2000],
                messages.len()
            )
        } else {
            all_content
        };

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_compaction() {
        let compactor = Compactor::new(CompactConfig {
            threshold_percent: 0.5,
            keep_recent: 2,
            min_messages: 3,
            summary_model: None,
        });

        let mut session = Session::new();
        session.meta.model = Some("claude-3-5-sonnet".to_string());

        // Not enough messages
        session.add_message(Message::user("Hello"));
        assert!(!compactor.needs_compaction(&session));

        session.add_message(Message::user("World"));
        assert!(!compactor.needs_compaction(&session));

        // Enough messages but low token count
        session.add_message(Message::user("Test"));
        assert!(!compactor.needs_compaction(&session));
    }
}
