//! SIMD-accelerated tokenization fallback.
//!
//! Token counting backends live in the [`simd-tokenizer`] crate
//! (<https://docs.rs/simd-tokenizer>), extracted from this module. The types
//! are re-exported here under the historical `clawdius_core::tokenizer`
//! paths so downstream imports keep working.
//!
//! # Architecture
//!
//! | Type | Description |
//! | --- | --- |
//! | [`TokenCounter`] | Trait abstracting token counting |
//! | [`SimdWhitespaceTokenizer`] | SWAR byte-scan for whitespace boundaries |
//! | [`TokenEstimator`] | Enum dispatching to the best available backend |
//! | [`TiktokenCounter`] | Exact cl100k_base counting (feature `tiktoken`) |
//!
//! The `tiktoken` feature (non-WASM) enables the [`TiktokenCounter`]
//! wrapper. Without it, [`TokenEstimator`] falls back to the SWAR estimator
//! only.

#[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
pub use simd_tokenizer::TiktokenCounter;
pub use simd_tokenizer::{
    estimate_from_whitespace_splits, scalar_count_splits, SimdWhitespaceTokenizer, TokenCounter,
    TokenEstimator,
};

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
        use crate::tokenize::{count_tokens, TokenizerStrategy};

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

    #[test]
    fn test_simd_matches_scalar() {
        let t = SimdWhitespaceTokenizer::new();
        let cases: &[&str] = &[
            "",
            "a",
            "hello world",
            "  leading and trailing  ",
            "tabs\there\ttoo",
            "new\nlines\nhere",
        ];
        let long = "The quick brown fox jumps over the lazy dog. ".repeat(10);
        let mut all_cases: Vec<&str> = cases.to_vec();
        all_cases.push(&long);
        for &case in &all_cases {
            let simd_splits = t.count_splits(case.as_bytes());
            let scalar_splits = scalar_count_splits(case.as_bytes());
            assert_eq!(
                simd_splits,
                scalar_splits,
                "mismatch for input of {} bytes",
                case.len()
            );
        }
    }

    #[test]
    fn test_unaligned_lengths_match_scalar() {
        for len in [16usize, 17, 32, 33, 64, 65] {
            let data = vec![b'x'; len];
            let t = SimdWhitespaceTokenizer::new();
            assert_eq!(t.count_splits(&data), scalar_count_splits(&data));
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
    #[test]
    fn test_tiktoken_backend_name() {
        let t = TiktokenCounter::new().expect("tiktoken init");
        assert_eq!(t.backend_name(), "tiktoken(cl100k_base)");
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "tiktoken"))]
    #[test]
    fn test_tiktoken_count() {
        let t = TiktokenCounter::new().expect("tiktoken init");
        let c = t.count("hello world");
        assert!(c >= 2, "expected >= 2, got {c}");
    }
}
