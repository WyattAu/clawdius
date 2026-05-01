//! Response formatting and chunking for different platform limits.
//!
//! Each platform has different message length limits and formatting
//! capabilities. [`ResponseFormatter`] splits long responses into
//! chunks that fit within platform constraints.

use crate::adapter::{OutgoingMessage, Platform};

/// Maximum size for a single message chunk.
const CHUNK_SIZE_RESERVE: usize = 50; // Reserve for metadata/footers

/// Formats and chunks responses for platform-specific constraints.
pub struct ResponseFormatter {
    /// Whether to include code block markers.
    pub preserve_code_blocks: bool,
}

impl ResponseFormatter {
    /// Create a new formatter with default settings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preserve_code_blocks: true,
        }
    }

    /// Split a response into chunks that fit within the platform's
    /// message length limit.
    ///
    /// Respects code block boundaries — chunks never split a code
    /// block in half (when `preserve_code_blocks` is true).
    #[must_use]
    pub fn chunk_response(&self, platform: Platform, text: &str) -> Vec<String> {
        let max_len = platform.max_message_length() - CHUNK_SIZE_RESERVE;

        if text.len() <= max_len {
            return vec![text.to_string()];
        }

        if self.preserve_code_blocks {
            self.chunk_preserving_code_blocks(text, max_len)
        } else {
            self.chunk_by_chars(text, max_len)
        }
    }

    /// Create outgoing messages from a response, properly chunked.
    #[must_use]
    pub fn format_response(
        &self,
        platform: Platform,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Vec<OutgoingMessage> {
        let chunks = self.chunk_response(platform, text);

        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let mut msg = OutgoingMessage::new(platform, chat_id, chunk);
                if i == 0 {
                    if let Some(reply) = reply_to {
                        msg = msg.with_reply_to(reply);
                    }
                }
                msg
            })
            .collect()
    }

    /// Create streamed chunk messages.
    #[must_use]
    pub fn format_stream_chunk(
        platform: Platform,
        chat_id: &str,
        text: &str,
        position: u64,
        message_id: Option<&str>,
    ) -> OutgoingMessage {
        let mut msg = OutgoingMessage::chunk(platform, chat_id, text, position);
        if let Some(id) = message_id {
            msg = msg.with_reply_to(id);
        }
        msg
    }

    /// Chunk text while preserving code block boundaries.
    #[allow(clippy::unused_self)]
    fn chunk_preserving_code_blocks(&self, text: &str, max_len: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut in_code_block = false;
        let mut code_block_lang = String::new();

        for line in text.lines() {
            let is_fence = line.trim_start().starts_with("```");

            if is_fence {
                in_code_block = !in_code_block;
                if in_code_block {
                    // Starting a code block
                    code_block_lang = line.trim_start().trim_start_matches('`').trim().to_string();
                }
            }

            // Check if adding this line would exceed the limit
            let proposed_len = if current.is_empty() {
                line.len()
            } else {
                current.len() + 1 + line.len() // +1 for newline
            };

            if proposed_len > max_len && !current.is_empty() {
                // Need to chunk
                if in_code_block {
                    // Close the code block in this chunk
                    current.push_str("\n```");
                    chunks.push(std::mem::take(&mut current));
                    // Reopen in next chunk
                    current = format!("```{code_block_lang}\n{line}");
                } else {
                    chunks.push(std::mem::take(&mut current));
                    current = line.to_string();
                }
            } else if proposed_len > max_len && current.is_empty() {
                // Single line exceeds limit — force chunk at max_len
                for chunk in line.as_bytes().chunks(max_len.max(1)) {
                    let chunk_str = String::from_utf8_lossy(chunk);
                    if !current.is_empty() {
                        chunks.push(std::mem::take(&mut current));
                    }
                    current = chunk_str.to_string();
                }
            } else {
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(line);
            }

            if is_fence && !in_code_block {
                code_block_lang.clear();
            }
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }

    /// Simple character-based chunking (fallback).
    #[allow(clippy::unused_self)]
    fn chunk_by_chars(&self, text: &str, max_len: usize) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < text.len() {
            let end = if start + max_len >= text.len() {
                text.len()
            } else {
                // Try to break at a newline within the limit
                let candidate = &text[start..start + max_len];
                candidate.rfind('\n').map_or_else(|| start + max_len, |newline_pos| start + newline_pos)
            };

            chunks.push(text[start..end].to_string());
            start = end;

            // Skip the newline we broke at
            if start < text.len() && text.as_bytes()[start] == b'\n' {
                start += 1;
            }
        }

        chunks
    }
}

impl Default for ResponseFormatter {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_message_no_chunk() {
        let formatter = ResponseFormatter::new();
        let chunks = formatter.chunk_response(Platform::Discord, "short message");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "short message");
    }

    #[test]
    fn test_long_message_chunks() {
        let formatter = ResponseFormatter::new();
        let long_text = "word ".repeat(1000); // ~5000 chars, exceeds Discord's 2000
        let chunks = formatter.chunk_response(Platform::Discord, &long_text);
        assert!(chunks.len() > 1);

        // Each chunk should be within Discord's limit
        for chunk in &chunks {
            assert!(chunk.len() <= Platform::Discord.max_message_length() - CHUNK_SIZE_RESERVE);
        }
    }

    #[test]
    fn test_code_block_preservation() {
        let formatter = ResponseFormatter::new();
        let text = "intro\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\noutro";
        let chunks = formatter.chunk_response(Platform::Telegram, text);
        assert_eq!(chunks.len(), 1); // Should fit in one chunk

        // Verify code block is intact
        assert!(chunks[0].contains("```rust"));
        assert!(chunks[0].contains("```"));
    }

    #[test]
    fn test_format_response_with_reply() {
        let formatter = ResponseFormatter::new();
        let msgs = formatter.format_response(
            Platform::Telegram,
            "chat123",
            "hello",
            Some("msg456"),
        );
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].reply_to, Some("msg456".to_string()));
    }

    #[test]
    fn test_format_stream_chunk() {
        let msg = ResponseFormatter::format_stream_chunk(
            Platform::Discord,
            "chat789",
            "partial text",
            3,
            Some("orig_msg"),
        );
        assert!(msg.is_chunk);
        assert_eq!(msg.stream_position, Some(3));
        assert_eq!(msg.reply_to, Some("orig_msg".to_string()));
    }

    #[test]
    fn test_slack_allows_longer_messages() {
        let formatter = ResponseFormatter::new();
        // Slack allows 40k chars
        let text = "x".repeat(30_000);
        let chunks = formatter.chunk_response(Platform::Slack, &text);
        assert_eq!(chunks.len(), 1);
    }
}
