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
//! use clawdius_core::tokenize::{count_tokens, TokenizerStrategy};
//!
//! let code = r#"fn main() {
//!     println!("Hello, world!");
//! }"#;
//!
//! let tokens = count_tokens(code, TokenizerStrategy::Code);
//! println!("Token count: {}", tokens);
//! ```

// Unwrap purge batch 4: tokenize module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

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
