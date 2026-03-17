//! LLM-based Code Generation
//!
//! This module provides real code generation using LLM providers.

use crate::error::Result;
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Response from LLM code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCode {
    /// The generated code content
    pub content: String,
    /// Target file path (if specified)
    pub file_path: Option<String>,
    /// Language detected
    pub language: Option<String>,
    /// Confidence level (0.0-1.0)
    pub confidence: f32,
    /// Any warnings or notes
    pub notes: Vec<String>,
}

/// LLM-based code generator.
pub struct LlmCodeGenerator {
    /// LLM client
    client: Arc<dyn LlmClient>,
    /// Model name for logging
    model_name: String,
}

impl LlmCodeGenerator {
    /// Creates a new code generator with the given LLM client.
    #[must_use]
    pub fn new(client: Arc<dyn LlmClient>, model_name: String) -> Self {
        Self { client, model_name }
    }

    /// Generates code based on a prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn generate(&self, prompt: &str, context: Option<&str>) -> Result<GeneratedCode> {
        let mut messages = vec![ChatMessage {
            role: ChatRole::System,
            content: CODE_GEN_SYSTEM_PROMPT.to_string(),
        }];

        // Add context if provided
        let user_content = if let Some(ctx) = context {
            format!("Context:\n{}\n\nTask:\n{}", ctx, prompt)
        } else {
            prompt.to_string()
        };

        messages.push(ChatMessage {
            role: ChatRole::User,
            content: user_content,
        });

        // Call LLM
        let response = self.client.chat(messages).await?;

        // Parse response
        Ok(self.parse_response(&response))
    }

    /// Generates code for a specific file.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn generate_for_file(
        &self,
        prompt: &str,
        file_path: &str,
        existing_content: Option<&str>,
    ) -> Result<GeneratedCode> {
        let context = existing_content.map(|c| format!("Existing file content:\n```\n{}\n```", c));

        let file_prompt = format!("Target file: {}\n\n{}", file_path, prompt);

        let mut result = self.generate(&file_prompt, context.as_deref()).await?;
        result.file_path = Some(file_path.to_string());
        Ok(result)
    }

    /// Generates a code edit based on a diff description.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails.
    pub async fn generate_edit(
        &self,
        file_path: &str,
        existing_content: &str,
        edit_description: &str,
    ) -> Result<GeneratedCode> {
        let prompt = format!(
            "Edit the following file according to the description.\n\n\
             File: {}\n\n\
             Current content:\n```\n{}\n```\n\n\
             Edit description: {}",
            file_path, existing_content, edit_description
        );

        let mut result = self.generate(&prompt, None).await?;
        result.file_path = Some(file_path.to_string());
        Ok(result)
    }

    /// Parses the LLM response into structured code.
    fn parse_response(&self, response: &str) -> GeneratedCode {
        // Try to extract code from markdown code blocks
        let (content, language) = self.extract_code_block(response);

        // Calculate a simple confidence based on response length and structure
        let confidence = self.calculate_confidence(&content, response);

        // Extract any notes (text outside code blocks)
        let notes = self.extract_notes(response);

        GeneratedCode {
            content,
            file_path: None,
            language,
            confidence,
            notes,
        }
    }

    /// Extracts code from markdown code blocks.
    fn extract_code_block(&self, response: &str) -> (String, Option<String>) {
        // Look for ```language\ncode\n``` pattern
        let code_block_regex = regex::Regex::new(r"```(\w*)\n([\s\S]*?)```").unwrap();

        if let Some(caps) = code_block_regex.captures(response) {
            let language = caps.get(1).map(|m| m.as_str().to_string());
            let code = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            return (code.trim().to_string(), language);
        }

        // No code block found, return the whole response
        (response.to_string(), None)
    }

    /// Calculates confidence score based on response quality indicators.
    fn calculate_confidence(&self, code: &str, full_response: &str) -> f32 {
        let mut score: f32 = 0.5; // Base score

        // Check for code block presence
        if full_response.contains("```") {
            score += 0.2;
        }

        // Check for reasonable code length
        let lines = code.lines().count();
        if lines > 0 && lines < 1000 {
            score += 0.1;
        }

        // Check for common code patterns
        if code.contains("fn ") || code.contains("function ") || code.contains("def ") {
            score += 0.1;
        }

        // Check for error handling
        if code.contains("Result<") || code.contains("try ") || code.contains("catch ") {
            score += 0.05;
        }

        // Check for documentation
        if code.contains("///") || code.contains("/**") || code.contains("# ") {
            score += 0.05;
        }

        score.min(1.0)
    }

    /// Extracts notes from the response (text outside code blocks).
    fn extract_notes(&self, response: &str) -> Vec<String> {
        let mut notes = Vec::new();

        // Split by code blocks and collect non-code text
        let parts: Vec<&str> = response.split("```").collect();
        for (i, part) in parts.iter().enumerate() {
            // Odd indices are inside code blocks, even are outside
            if i % 2 == 0 {
                let text = part.trim();
                if !text.is_empty() && text.len() > 10 {
                    notes.push(text.to_string());
                }
            }
        }

        notes
    }
}

impl std::fmt::Debug for LlmCodeGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmCodeGenerator")
            .field("model_name", &self.model_name)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    /// Mock LLM client for testing
    struct MockLlmClient;

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Ok(r#"Here's the generated code:

```rust
fn hello_world() {
    println!("Hello, world!");
}
```

This function prints a greeting message."#
                .to_string())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send("test".to_string()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    #[tokio::test]
    async fn test_generate_code() {
        let client = Arc::new(MockLlmClient);
        let generator = LlmCodeGenerator::new(client, "test-model".to_string());

        let result = generator
            .generate("Write a hello world function", None)
            .await;

        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(code.content.contains("fn hello_world"));
        assert_eq!(code.language, Some("rust".to_string()));
        assert!(!code.notes.is_empty());
    }

    #[test]
    fn test_extract_code_block() {
        let client = Arc::new(MockLlmClient);
        let generator = LlmCodeGenerator::new(client, "test-model".to_string());

        let response = r#"Here's the code:

```python
def hello():
    print("Hello")
```

That's it!"#;

        let (code, lang) = generator.extract_code_block(response);
        assert_eq!(lang, Some("python".to_string()));
        assert!(code.contains("def hello"));
    }

    #[test]
    fn test_calculate_confidence() {
        let client = Arc::new(MockLlmClient);
        let generator = LlmCodeGenerator::new(client, "test-model".to_string());

        // Good code response
        let good_code = r#"```rust
/// Documentation
fn example() -> Result<()> {
    Ok(())
}
```"#;
        let confidence =
            generator.calculate_confidence("fn example() -> Result<()> { Ok(()) }", good_code);
        assert!(confidence > 0.7);

        // Poor response (no code block, no patterns)
        let poor_response = "I don't know";
        let confidence = generator.calculate_confidence(poor_response, poor_response);
        // Poor responses still get base score of 0.5 since we can't fully determine quality
        assert!(confidence >= 0.5 && confidence < 0.7);
    }
}
