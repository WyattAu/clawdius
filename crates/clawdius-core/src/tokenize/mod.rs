//! Token Counting Module
//!
//! Provides accurate token counting for LLM context management.
//!
//! # Features
//!
//! - **Code-aware tokenization**: Handles programming language syntax
//! - **Multiple strategies**: Simple, BPE-like, and language-specific
//! - **No external dependencies**: Self-contained implementation
//!
//! # Example
//!
//! ```rust
//! use clawdius_core::tokenize::count_tokens;
//!
//! let code = r#"fn main() {
//!     println!("Hello, world!");
//! }"#;
//!
//! let tokens = count_tokens(code, TokenizerStrategy::Code);
//! println!("Token count: {}", tokens);
//! ```

mod counter;

pub use counter::{count_tokens, TokenizerStrategy};

/// Token count estimate for different content types
#[derive(Debug, Clone, Copy)]
pub struct TokenEstimate {
    /// Estimated token count
    pub tokens: usize,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,
    /// Strategy used
    pub strategy: TokenizerStrategy,
}

impl TokenEstimate {
    /// Creates a new estimate.
    #[must_use]
    pub fn new(tokens: usize, confidence: f32, strategy: TokenizerStrategy) -> Self {
        Self {
            tokens,
            confidence: confidence.clamp(0.0, 1.0),
            strategy,
        }
    }
}
