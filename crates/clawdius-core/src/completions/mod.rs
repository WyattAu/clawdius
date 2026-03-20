//! Inline Completions Module
//!
//! Provides AI-powered inline code completions similar to GitHub Copilot.
//!
//! # Features
//!
//! - **Multi-provider support**: Works with all LLM providers (Anthropic, OpenAI, Ollama, etc.)
//! - **Context-aware**: Uses surrounding code for better suggestions
//! - **Streaming**: Real-time streaming completions for faster feedback
//! - **Caching**: Response caching for repeated patterns
//! - **Language detection**: Automatic language detection for syntax-aware completions
//!
//! # Example
//!
//! ```rust,ignore
//! use clawdius_core::completions::{InlineCompletionProvider, LlmCompletionConfig};
//! use clawdius_core::llm::create_provider;
//!
//! let config = LlmCompletionConfig {
//!     max_tokens: 256,
//!     temperature: 0.3,
//!     ..Default::default()
//! };
//!
//! let provider = InlineCompletionProvider::new(llm_provider, config);
//!
//! let request = CompletionRequest {
//!     document: "fn calculate_sum(numbers: &[i32]) -> i32 {\n    ".to_string(),
//!     position: Position::new(1, 4),
//!     language: "rust".to_string(),
//!     ..Default::default()
//! };
//!
//! let completion = provider.complete(&request).await?;
//! println!("Suggestion: {}", completion.text);
//! ```

mod cache;
mod provider;
mod types;

pub use cache::CompletionCache;
pub use provider::{InlineCompletionProvider, LlmCompletionConfig};
pub use types::{
    CompletionContext, CompletionProvider as CompletionProviderTrait, CompletionRequest,
    CompletionResponse, CompletionTrigger, FimTemplate,
};
