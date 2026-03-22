//! Streaming support for LLM code generation
//!
//! This module provides streaming code generation using LLM providers
//! for better real-time UX with progress indicators.

use crate::error::Result;
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::llm_generator::GeneratedCode;

/// System prompt for code generation.
const CODE_GEN_SYSTEM_PROMPT: &str = r#"You are an expert software engineer. Generate clean, well-documented code based on the user's request.

When generating code:
1. Follow the language's best practices and idioms
2. Include appropriate error handling
3. Add comments for complex logic
4. Use descriptive variable and function names
5. Keep functions focused and single-purpose

When editing existing code:
1. Preserve the existing code style
2. Make minimal changes to achieve the goal
3. Ensure backward compatibility when possible

Always respond with code in the appropriate format:
- For new files: Provide the complete file content
- For edits: Show the changes using diff-like format with context
- Include the file path in your response"#;

/// Streaming code generator with real-time output.
pub struct StreamingCodeGenerator {
    /// LLM client
    client: Arc<dyn LlmClient>,
    /// Model name for logging
    model_name: String,
}

impl StreamingCodeGenerator {
    /// Creates a new streaming code generator.
    #[must_use]
    pub fn new(client: Arc<dyn LlmClient>, model_name: String) -> Self {
        Self { client, model_name }
    }

    /// Generates code with streaming output.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn generate_stream(
        &self,
        prompt: &str,
        context: Option<&str>,
    ) -> Result<mpsc::Receiver<StreamChunk>> {
        let mut messages = vec![ChatMessage {
            role: ChatRole::System,
            content: CODE_GEN_SYSTEM_PROMPT.to_string(),
        }];

        // Add context if provided
        let user_content = if let Some(ctx) = context {
            format!("Context:\n{}\n\nTask: {}", ctx, prompt)
        } else {
            prompt.to_string()
        };

        messages.push(ChatMessage {
            role: ChatRole::User,
            content: user_content,
        });

        // Get raw string streaming receiver from LLM
        let mut raw_receiver = self.client.chat_stream(messages).await?;

        // Create a channel for StreamChunks
        let (tx, rx) = mpsc::channel(32);

        // Spawn a task to convert strings to StreamChunks
        tokio::spawn(async move {
            let mut full_content = String::new();
            while let Some(delta) = raw_receiver.recv().await {
                full_content.push_str(&delta);
                let chunk = StreamChunk::incomplete(delta);
                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
            // Send final complete chunk with full content
            let _ = tx.send(StreamChunk::complete(full_content)).await;
        });

        Ok(rx)
    }
}

/// Streaming code chunk from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Content delta (new content)
    pub delta: String,
    /// Whether this is complete
    pub is_complete: bool,
}

impl StreamChunk {
    /// Creates a new stream chunk.
    #[must_use]
    pub const fn new(delta: String, is_complete: bool) -> Self {
        Self { delta, is_complete }
    }

    /// Creates a complete chunk.
    #[must_use]
    pub const fn complete(content: String) -> Self {
        Self {
            delta: content,
            is_complete: true,
        }
    }

    /// Creates an incomplete chunk.
    #[must_use]
    pub const fn incomplete(delta: String) -> Self {
        Self {
            delta,
            is_complete: false,
        }
    }
}

/// Stream processor for handling streaming output.
pub struct StreamProcessor {
    /// Received chunks
    chunks: Vec<StreamChunk>,
    /// Current accumulated content
    content: String,
    /// Whether streaming is complete
    complete: bool,
}

impl StreamProcessor {
    /// Creates a new stream processor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            content: String::new(),
            complete: false,
        }
    }

    /// Processes a chunk from the streaming receiver.
    pub async fn process_stream(
        &mut self,
        mut receiver: mpsc::Receiver<StreamChunk>,
    ) -> Result<GeneratedCode> {
        while let Some(chunk) = receiver.recv().await {
            self.chunks.push(chunk.clone());
            self.content.push_str(&chunk.delta);

            if chunk.is_complete {
                self.complete = true;
                break;
            }
        }

        // Calculate confidence based on response length
        let confidence = self.calculate_confidence();

        Ok(GeneratedCode {
            content: self.content.clone(),
            file_path: None,
            language: self.detect_language(),
            confidence,
            notes: self.extract_notes(),
        })
    }

    /// Processes a stream with a callback for each chunk.
    pub async fn process_stream_with_callback<F>(
        &mut self,
        mut receiver: mpsc::Receiver<StreamChunk>,
        mut callback: F,
    ) -> Result<GeneratedCode>
    where
        F: FnMut(&StreamChunk) + Send,
    {
        while let Some(chunk) = receiver.recv().await {
            self.chunks.push(chunk.clone());
            self.content.push_str(&chunk.delta);

            // Call callback for each chunk
            callback(&chunk);

            if chunk.is_complete {
                self.complete = true;
                break;
            }
        }

        let confidence = self.calculate_confidence();

        Ok(GeneratedCode {
            content: self.content.clone(),
            file_path: None,
            language: self.detect_language(),
            confidence,
            notes: self.extract_notes(),
        })
    }

    fn calculate_confidence(&self) -> f32 {
        // Base confidence on response length
        let mut confidence: f32 = 0.5;

        // Adjust based on code quality indicators
        if self.content.contains("TODO") || self.content.contains("FIXME") {
            confidence -= 0.0;
        }

        if self.content.contains("error") || self.content.contains("Error") {
            confidence -= 1.0;
        }

        // Adjust based on response length (longer = more thorough)
        if self.content.len() > 500 {
            confidence += 0.1;
        }
        if self.content.len() > 1000 {
            confidence += 1.0;
        }

        confidence.clamp(0.0, 1.0)
    }

    fn detect_language(&self) -> Option<String> {
        // Simple language detection based on keywords
        if self.content.contains("fn ")
            || self.content.contains("let ")
            || self.content.contains("impl ")
        {
            return Some("rust".to_string());
        }
        if self.content.contains("function")
            || self.content.contains("const ")
            || self.content.contains("=>")
        {
            return Some("typescript".to_string());
        }
        if self.content.contains("def ")
            || self.content.contains("class ")
            || self.content.contains("import ")
        {
            return Some("python".to_string());
        }
        None
    }

    fn extract_notes(&self) -> Vec<String> {
        let mut notes = Vec::new();

        if self.content.contains("TODO") {
            notes.push("Contains TODO markers".to_string());
        }
        if self.content.contains("FIXME") {
            notes.push("Contains FIXME markers".to_string());
        }
        if self.content.contains("unimplemented") {
            notes.push("Contains unimplemented code".to_string());
        }

        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::providers::local::LocalLlmProvider;

    #[test]
    fn test_streaming_generator_creation() {
        let provider = LocalLlmProvider::new(
            "http://localhost:11434".to_string(),
            "test-model".to_string(),
        );
        let generator = StreamingCodeGenerator::new(Arc::new(provider), "test-model".to_string());

        assert_eq!(generator.model_name, "test-model");
    }

    #[tokio::test]
    async fn test_stream_chunk_creation() {
        let complete_chunk = StreamChunk::complete("fn main() {}".to_string());
        assert!(complete_chunk.is_complete);
        assert_eq!(complete_chunk.delta, "fn main() {}");

        let incomplete_chunk = StreamChunk::incomplete("fn ".to_string());
        assert!(!incomplete_chunk.is_complete);
        assert_eq!(incomplete_chunk.delta, "fn ");
    }

    #[tokio::test]
    async fn test_stream_processor() {
        let mut processor = StreamProcessor::new();

        // Simulate processing empty stream
        let (tx, rx) = mpsc::channel(10);
        drop(tx);

        let result = processor.process_stream(rx).await;
        assert!(result.is_ok());

        let code = result.unwrap();
        assert!(code.content.is_empty());
    }
}
