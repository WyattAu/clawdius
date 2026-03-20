//! Inline Completion Provider
//!
//! LLM-powered inline completion provider.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::cache::{CacheConfig, CompletionCache};
use super::types::{
    CompletionContext, CompletionProvider, CompletionRequest, CompletionResponse, FimTemplate,
};
use crate::llm::providers::LlmClient;
use crate::Result;

/// Configuration for LLM-based completions.
#[derive(Debug, Clone)]
pub struct LlmCompletionConfig {
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Temperature for generation
    pub temperature: f32,
    /// Top-p sampling
    pub top_p: f32,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
    /// FIM template to use
    pub fim_template: FimTemplate,
    /// Enable caching
    pub enable_cache: bool,
    /// Cache configuration
    pub cache_config: CacheConfig,
    /// Minimum prefix length for completion
    pub min_prefix_length: usize,
    /// Maximum context length (in characters)
    pub max_context_length: usize,
}

impl Default for LlmCompletionConfig {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.3,
            top_p: 0.95,
            stop_sequences: vec![
                "\n\n".to_string(),
                "```".to_string(),
                "fn ".to_string(),
                "struct ".to_string(),
                "impl ".to_string(),
            ],
            fim_template: FimTemplate::codellama(),
            enable_cache: true,
            cache_config: CacheConfig::default(),
            min_prefix_length: 10,
            max_context_length: 8000,
        }
    }
}

impl LlmCompletionConfig {
    /// Creates a new configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum tokens.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Sets the temperature.
    #[must_use]
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Sets the FIM template.
    #[must_use]
    pub fn with_fim_template(mut self, template: FimTemplate) -> Self {
        self.fim_template = template;
        self
    }

    /// Disables caching.
    #[must_use]
    pub fn without_cache(mut self) -> Self {
        self.enable_cache = false;
        self
    }
}

/// LLM-powered inline completion provider.
pub struct InlineCompletionProvider {
    llm: Arc<dyn LlmClient>,
    config: LlmCompletionConfig,
    cache: CompletionCache,
}

impl InlineCompletionProvider {
    /// Creates a new completion provider.
    #[must_use]
    pub fn new(llm: Arc<dyn LlmClient>, config: LlmCompletionConfig) -> Self {
        let cache = CompletionCache::with_config(config.cache_config.clone());
        Self { llm, config, cache }
    }

    /// Creates a provider with default configuration.
    #[must_use]
    pub fn with_defaults(llm: Arc<dyn LlmClient>) -> Self {
        Self::new(llm, LlmCompletionConfig::default())
    }

    /// Builds the completion prompt.
    fn build_prompt(&self, request: &CompletionRequest) -> String {
        let prefix = request.prefix();
        let suffix = request.suffix();

        // Truncate context if too long
        let prefix = if prefix.len() > self.config.max_context_length {
            let start = prefix.len() - self.config.max_context_length;
            &prefix[start..]
        } else {
            prefix
        };

        // Build language-specific prompt
        let language_prompt = self.get_language_prompt(&request.language);

        // For models that support FIM (Fill-in-the-Middle)
        if self.supports_fim() {
            format!(
                "{}\n{}",
                language_prompt,
                self.config.fim_template.format(prefix, suffix)
            )
        } else {
            // Fallback to instruction-based prompt
            format!(
                "{}\n\nComplete the following {} code. Only output the completion, no explanations.\n\nCode:\n{}\n// CURSOR HERE\n{}",
                language_prompt,
                request.language,
                prefix,
                suffix
            )
        }
    }

    /// Gets language-specific prompting hints.
    fn get_language_prompt(&self, language: &str) -> String {
        match language.to_lowercase().as_str() {
            "rust" => {
                "You are an expert Rust programmer. Write idiomatic, safe, and efficient code."
            }
            "python" => "You are an expert Python programmer. Write clean, PEP-8 compliant code.",
            "javascript" | "typescript" => {
                "You are an expert JavaScript/TypeScript programmer. Write modern, clean code."
            }
            "go" => "You are an expert Go programmer. Write idiomatic Go code.",
            "java" => "You are an expert Java programmer. Write clean, efficient Java code.",
            "c" | "cpp" => "You are an expert C/C++ programmer. Write efficient, safe code.",
            _ => "You are an expert programmer. Write clean, efficient code.",
        }
        .to_string()
    }

    /// Checks if the LLM supports FIM format.
    fn supports_fim(&self) -> bool {
        // Most code-specific models support FIM
        // This could be made configurable per model
        true
    }

    /// Post-processes the completion.
    fn post_process(&self, completion: &str, request: &CompletionRequest) -> String {
        let mut result = completion.to_string();

        // Remove common artifacts
        let artifacts = [
            "```", // Code blocks
            "```rust",
            "```python",
            "```javascript",
            "```typescript",
            "```go",
            "```java",
            "```c",
            "```cpp",
            "<MID>",
            "<｜fim▁end｜>",
            "<fim_middle>",
        ];

        for artifact in artifacts {
            result = result.replace(artifact, "");
        }

        // Trim trailing whitespace from each line but preserve structure
        result = result
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        // Remove leading newline if present
        result = result.trim_start_matches('\n').to_string();

        // If the completion starts to repeat the prefix, remove it
        let prefix_end = request.current_line_prefix();
        if result.starts_with(prefix_end) && !prefix_end.is_empty() {
            result = result[prefix_end.len()..].to_string();
        }

        result
    }

    /// Gets context from related files.
    #[allow(dead_code)]
    fn build_context(&self, context: &CompletionContext) -> Option<String> {
        if context.related_files.is_empty() {
            return None;
        }

        let mut ctx = String::from("Related files:\n\n");

        for (path, content) in context.related_files.iter().take(3) {
            ctx.push_str(&format!("--- {} ---\n", path));

            // Include only the first N lines
            let lines: Vec<&str> = content.lines().take(50).collect();
            ctx.push_str(&lines.join("\n"));
            ctx.push_str("\n\n");
        }

        Some(ctx)
    }
}

#[async_trait::async_trait]
impl CompletionProvider for InlineCompletionProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        // Check minimum prefix length
        if request.prefix().len() < self.config.min_prefix_length {
            return Ok(CompletionResponse::default());
        }

        // Check cache
        if self.config.enable_cache {
            if let Some(cached) = self.cache.get(request).await {
                return Ok(cached);
            }
        }

        // Build prompt
        let prompt = self.build_prompt(request);

        // Create chat message
        let messages = vec![crate::llm::ChatMessage {
            role: crate::llm::ChatRole::User,
            content: prompt,
        }];

        // Call LLM
        let raw_completion = self.llm.chat(messages).await?;

        // Post-process
        let processed = self.post_process(&raw_completion, request);

        // Build response
        let response = CompletionResponse::new(&processed)
            .with_confidence(0.8)
            .complete();

        // Cache result
        if self.config.enable_cache {
            self.cache.put(request, response.clone()).await;
        }

        Ok(response)
    }

    async fn complete_stream(
        &self,
        request: &CompletionRequest,
    ) -> Result<mpsc::Receiver<CompletionResponse>> {
        // Check minimum prefix length
        if request.prefix().len() < self.config.min_prefix_length {
            let (_, rx) = mpsc::channel(1);
            return Ok(rx);
        }

        let (tx, rx) = mpsc::channel(16);

        // Build prompt
        let prompt = self.build_prompt(request);

        // Create chat message
        let messages = vec![crate::llm::ChatMessage {
            role: crate::llm::ChatRole::User,
            content: prompt,
        }];

        // Get streaming response
        let mut stream_rx = self.llm.chat_stream(messages).await?;

        // Spawn task to process stream
        let config = self.config.clone();
        let request = request.clone();

        tokio::spawn(async move {
            let mut accumulated = String::new();

            while let Some(chunk) = stream_rx.recv().await {
                if chunk.starts_with("[Error:") {
                    let _ = tx.send(CompletionResponse::new(&chunk)).await;
                    break;
                }

                accumulated.push_str(&chunk);

                // Send partial completion
                let processed = Self::post_process_static(&accumulated, &request, &config);
                let response = CompletionResponse::new(&processed).with_confidence(0.7);

                if tx.send(response).await.is_err() {
                    break;
                }
            }

            // Send final processed completion
            let processed = Self::post_process_static(&accumulated, &request, &config);
            let final_response = CompletionResponse::new(&processed)
                .with_confidence(0.8)
                .complete();

            let _ = tx.send(final_response).await;
        });

        Ok(rx)
    }

    fn is_ready(&self) -> bool {
        true
    }
}

impl InlineCompletionProvider {
    /// Static post-processing for use in spawned tasks.
    fn post_process_static(
        completion: &str,
        request: &CompletionRequest,
        config: &LlmCompletionConfig,
    ) -> String {
        let mut result = completion.to_string();

        // Remove common artifacts
        let artifacts = [
            "```",
            "```rust",
            "```python",
            "```javascript",
            "```typescript",
            "```go",
            "```java",
            "```c",
            "```cpp",
            "<MID>",
            "<｜fim▁end｜>",
            "<fim_middle>",
        ];

        for artifact in artifacts {
            result = result.replace(artifact, "");
        }

        result = result
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        result = result.trim_start_matches('\n').to_string();

        // If the completion starts to repeat the prefix, remove it
        let prefix_end = request.current_line_prefix();
        if result.starts_with(prefix_end) && !prefix_end.is_empty() {
            result = result[prefix_end.len()..].to_string();
        }

        // Apply stop sequences
        for stop in &config.stop_sequences {
            if let Some(pos) = result.find(stop) {
                result = result[..pos].to_string();
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::providers::local::LocalLlmProvider;
    use crate::lsp::Position;

    #[test]
    fn test_build_prompt() {
        let llm = Arc::new(LocalLlmProvider::new(
            "http://localhost:11434".to_string(),
            "test".to_string(),
        ));
        let provider = InlineCompletionProvider::with_defaults(llm);

        let request = CompletionRequest::new("fn main() {\n    ", Position::new(1, 4), "rust");

        let prompt = provider.build_prompt(&request);

        assert!(prompt.contains("Rust programmer"));
        assert!(prompt.contains("fn main()"));
    }

    #[test]
    fn test_post_process() {
        let llm = Arc::new(LocalLlmProvider::new(
            "http://localhost:11434".to_string(),
            "test".to_string(),
        ));
        let provider = InlineCompletionProvider::with_defaults(llm);

        let request =
            CompletionRequest::new("fn test() {\n    let x = ", Position::new(1, 12), "rust");

        // Test code block removal
        let completion = "```rust\nlet y = 5;\n```";
        let processed = provider.post_process(completion, &request);
        assert!(!processed.contains("```"));

        // Test prefix removal
        let completion = "let x = 5;";
        let processed = provider.post_process(completion, &request);
        // Should not repeat "let x = "
    }

    #[test]
    fn test_config() {
        let config = LlmCompletionConfig::new()
            .with_max_tokens(512)
            .with_temperature(0.5)
            .without_cache();

        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.temperature, 0.5);
        assert!(!config.enable_cache);
    }
}
