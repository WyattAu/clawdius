//! SIMD-accelerated tokenization fallback.
//!
//! Provides a pure-Rust, SIMD-boosted token estimator for environments where
//! `tiktoken-rs` is unavailable (WASM builds, minimal builds, or when the C/C++
//! BPE tokenizer cannot be linked).
//!
//! # Architecture
//!
//! | Type | Description |
//! | --- | --- |
//! | [`TokenCounter`] | Trait abstracting token counting |
//! | [`SimdWhitespaceTokenizer`] | SIMD byte-scan for whitespace boundaries |
//! | [`TokenEstimator`] | Enum dispatching to the best available backend |
//!
//! The `tiktoken` feature (default on, non-WASM) gates the [`TiktokenCounter`]
//! wrapper. Without it, [`TokenEstimator`] falls back to the SIMD estimator only.

#![allow(unsafe_code)]

#[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
mod tiktoken_backend;

mod simd_tokenizer;

pub use simd_tokenizer::SimdWhitespaceTokenizer;
#[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
pub use tiktoken_backend::TiktokenCounter;

use crate::tokenize::{count_tokens, TokenizerStrategy};

/// Abstraction over token counting backends.
pub trait TokenCounter: Send + Sync {
    /// Return an estimated (or exact) token count for `text`.
    fn count(&self, text: &str) -> usize;

    /// Return a human-readable name for this backend.
    fn backend_name(&self) -> &'static str;
}

/// Enum that dispatches to the best available token counter.
///
/// With the `tiktoken` feature enabled on non-WASM targets this wraps a
/// [`TiktokenCounter`]; otherwise it uses [`SimdWhitespaceTokenizer`].
#[derive(Debug)]
pub enum TokenEstimator {
    /// Exact counting via tiktoken-rs (cl100k_base).
    #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
    Tiktoken(TiktokenCounter),
    /// Fast SIMD-accelerated approximation.
    Simd(SimdWhitespaceTokenizer),
}

impl TokenEstimator {
    /// Create the best estimator available for the current target.
    pub fn new() -> Self {
        #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
        {
            Self::Tiktoken(
                TiktokenCounter::new().expect("failed to initialise cl100k_base tokenizer"),
            )
        }
        #[cfg(not(all(not(target_arch = "wasm32"), feature = "tiktoken")))]
        {
            Self::Simd(SimdWhitespaceTokenizer::new())
        }
    }
}

impl Default for TokenEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter for TokenEstimator {
    fn count(&self, text: &str) -> usize {
        match self {
            #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
            Self::Tiktoken(t) => t.count(text),
            Self::Simd(s) => s.count(text),
        }
    }

    fn backend_name(&self) -> &'static str {
        match self {
            #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
            Self::Tiktoken(_) => "tiktoken(cl100k_base)",
            Self::Simd(_) => "simd-whitespace",
        }
    }
}

/// Legacy-count wrapper used internally by [`SimdWhitespaceTokenizer`].
///
/// This is deliberately kept simple: split on whitespace, add a small
/// overhead for punctuation that would normally become separate tokens in BPE,
/// and apply a ~4 chars/token heuristic for whitespace-dense text.
pub(crate) fn estimate_from_whitespace_splits(splits: usize, byte_len: usize) -> usize {
    if splits == 0 {
        return 0;
    }
    let word_tokens = splits;
    let punct_overhead = (byte_len / 40).max(1);
    let char_based = ((byte_len as f64) / 4.0).ceil() as usize;
    // Weighted blend: trust word count when there are clear word boundaries,
    // fall back to char-based when text is dense.
    let blended = (word_tokens + punct_overhead + char_based) / 2;
    blended.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_string() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.count(""), 0);
    }

    #[test]
    fn test_single_word() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("hello");
        assert!(c >= 1, "single word should yield >= 1 token, got {c}");
    }

    #[test]
    fn test_simple_sentence() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("The quick brown fox jumps over the lazy dog.");
        assert!(c >= 5, "sentence should yield >= 5 tokens, got {c}");
    }

    #[test]
    fn test_long_text() {
        let t = SimdWhitespaceTokenizer::new();
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let c = t.count(&text);
        assert!(c >= 100, "long text should yield >= 100 tokens, got {c}");
    }

    #[test]
    fn test_unicode_text() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("こんにちは世界 Hello 世界 this is unicode 🌍");
        assert!(c >= 3, "unicode text should yield >= 3 tokens, got {c}");
    }

    #[test]
    fn test_code_with_symbols() {
        let t = SimdWhitespaceTokenizer::new();
        let code = r#"fn main() {
    let x = 42;
    println!("Hello, world!");
    if x > 0 {
        println!("positive");
    }
}"#;
        let c = t.count(code);
        assert!(c >= 8, "code should yield >= 8 tokens, got {c}");
    }

    #[test]
    fn test_whitespace_only() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.count("   \t\n\r  "), 0);
    }

    #[test]
    fn test_single_character() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.count("a"), 1);
    }

    #[test]
    fn test_repeated_spaces() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("hello   world");
        assert_eq!(t.count("hello world"), c);
    }

    #[test]
    fn test_newlines_and_tabs() {
        let t = SimdWhitespaceTokenizer::new();
        let c1 = t.count("hello\nworld");
        let c2 = t.count("hello\tworld");
        let c3 = t.count("hello world");
        assert_eq!(c1, c2);
        assert_eq!(c2, c3);
    }

    #[test]
    fn test_backend_name() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.backend_name(), "simd-whitespace");
    }

    #[test]
    fn test_estimator_default() {
        let e = TokenEstimator::default();
        let c = e.count("hello world");
        assert!(c >= 1);
    }

    #[test]
    fn test_estimate_from_splits_empty() {
        assert_eq!(estimate_from_whitespace_splits(0, 0), 0);
    }

    #[test]
    fn test_estimate_from_splits_single() {
        let est = estimate_from_whitespace_splits(1, 5);
        assert!(est >= 1);
    }

    #[test]
    fn test_simd_matches_bpe_approximation_within_bounds() {
        let t = SimdWhitespaceTokenizer::new();
        let sentences = [
            "Hello, world!",
            "The quick brown fox jumps over the lazy dog.",
            "fn main() { println!(\"Hello\"); }",
            "This is a longer sentence with many words and punctuation marks.",
            "123 + 456 = 579",
        ];
        for s in &sentences {
            let simd_count = t.count(s);
            let bpe_count = count_tokens(s, TokenizerStrategy::BpeApproximation);
            let ratio = simd_count as f64 / bpe_count as f64;
            assert!(
                ratio >= 0.5 && ratio <= 2.0,
                "simd={simd_count}, bpe={bpe_count}, ratio={ratio:.2} for: {s}"
            );
        }
    }

    #[test]
    fn test_simd_consistent_with_fallback() {
        let t = SimdWhitespaceTokenizer::new();
        let texts = [
            "",
            "a",
            "hello world",
            "line1\nline2\nline3",
            "spaces   and    tabs\t\there",
        ];
        for text in &texts {
            let c1 = t.count(text);
            let c2 = t.count(text);
            assert_eq!(c1, c2, "count must be deterministic for: {text:?}");
        }
    }

    #[test]
    fn test_large_input_no_panic() {
        let t = SimdWhitespaceTokenizer::new();
        let text = "word ".repeat(100_000);
        let _ = t.count(&text);
    }

    #[test]
    fn test_emoji_heavy() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("😀😂🤣😃😄😁😆😅🤪😊😎");
        assert!(c >= 1);
    }
}
