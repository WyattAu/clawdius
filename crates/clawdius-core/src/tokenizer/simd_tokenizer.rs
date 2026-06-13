//! SIMD-accelerated whitespace tokenizer.
//!
//! Scans UTF-8 bytes using SIMD lanes (SSE2 / NEON) to classify whitespace
//! runs and estimate token boundaries.  Falls back to a scalar byte scan when
//! SIMD is unavailable.

#![allow(unsafe_code)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use super::{estimate_from_whitespace_splits, TokenCounter};

/// SIMD-accelerated whitespace-split token estimator.
///
/// Splits on any byte ≤ 0x20 (covers space, tab, newline, CR, and other C0
/// controls) using 16-byte SIMD loads, then blends word count + character-based
/// heuristics for a BPE-approximate token count.
#[derive(Debug, Clone, Default)]
pub struct SimdWhitespaceTokenizer {
    _priv: (), // prevent direct construction — use ::new()
}

impl SimdWhitespaceTokenizer {
    pub fn new() -> Self {
        Self { _priv: () }
    }

    /// Count whitespace-delimited segments using the fastest available path.
    pub fn count_splits(&self, bytes: &[u8]) -> usize {
        if bytes.is_empty() {
            return 0;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse2") {
                return unsafe { count_splits_sse2(bytes) };
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                return unsafe { count_splits_neon(bytes) };
            }
        }

        count_splits_scalar(bytes)
    }
}

impl TokenCounter for SimdWhitespaceTokenizer {
    fn count(&self, text: &str) -> usize {
        let bytes = text.as_bytes();
        let splits = self.count_splits(bytes);
        estimate_from_whitespace_splits(splits, bytes.len())
    }

    fn backend_name(&self) -> &'static str {
        "simd-whitespace"
    }
}

/// Scalar whitespace boundary counter.
fn count_splits_scalar(data: &[u8]) -> usize {
    let mut in_word = false;
    let mut count = 0;
    for &b in data {
        let is_ws = b <= 0x20;
        if is_ws {
            if in_word {
                count += 1;
                in_word = false;
            }
        } else {
            in_word = true;
        }
    }
    if in_word {
        count += 1;
    }
    count
}

/// Build a 16-byte mask where each byte is 0xFF if ≤ 0x20 else 0x00.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn whitespace_mask_sse2(chunk: __m128i) -> __m128i {
    // byte <= 0x20  ⇔  byte < 0x21
    let thresholds = _mm_set1_epi8(0x21);
    _mm_cmplt_epi8(chunk, thresholds)
}

/// SSE2 whitespace-split counter.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn count_splits_sse2(data: &[u8]) -> usize {
    let len = data.len();
    let mut count = 0;
    let mut in_word = false;
    let mut i = 0;

    while i + 16 <= len {
        let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
        let mask = whitespace_mask_sse2(chunk);
        let mask_bits = _mm_movemask_epi8(mask) as u32;

        for bit in 0..16u32 {
            let is_ws = (mask_bits >> bit) & 1 == 1;
            if is_ws {
                if in_word {
                    count += 1;
                    in_word = false;
                }
            } else {
                in_word = true;
            }
        }
        i += 16;
    }

    for &b in &data[i..] {
        if b <= 0x20 {
            if in_word {
                count += 1;
                in_word = false;
            }
        } else {
            in_word = true;
        }
    }
    if in_word {
        count += 1;
    }
    count
}

/// NEON whitespace-split counter.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn count_splits_neon(data: &[u8]) -> usize {
    let len = data.len();
    let mut count = 0;
    let mut in_word = false;
    let mut i = 0;

    let threshold = vdupq_n_u8(0x20);

    while i + 16 <= len {
        let chunk = vld1q_u8(data.as_ptr().add(i));
        let cmp = vcleq_u8(chunk, threshold);

        // Store SIMD result to local array for per-lane inspection.
        // (vgetq_lane_u8 requires a const lane index in Rust's aarch64 intrinsics)
        let mut buf = [0u8; 16];
        vst1q_u8(buf.as_mut_ptr(), cmp);
        for &lane in &buf {
            if lane != 0 {
                if in_word {
                    count += 1;
                    in_word = false;
                }
            } else {
                in_word = true;
            }
        }
        i += 16;
    }

    for &b in &data[i..] {
        if b <= 0x20 {
            if in_word {
                count += 1;
                in_word = false;
            }
        } else {
            in_word = true;
        }
    }
    if in_word {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_empty() {
        assert_eq!(count_splits_scalar(b""), 0);
    }

    #[test]
    fn test_scalar_single_word() {
        assert_eq!(count_splits_scalar(b"hello"), 1);
    }

    #[test]
    fn test_scalar_two_words() {
        assert_eq!(count_splits_scalar(b"hello world"), 2);
    }

    #[test]
    fn test_scalar_whitespace_only() {
        assert_eq!(count_splits_scalar(b"   \t\n"), 0);
    }

    #[test]
    fn test_scalar_mixed_whitespace() {
        assert_eq!(count_splits_scalar(b"a b\tc\nd"), 4);
    }

    #[test]
    fn test_scalar_leading_trailing_ws() {
        assert_eq!(count_splits_scalar(b"  hello world  "), 2);
    }

    #[test]
    fn test_simd_tokenizer_empty() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.count(""), 0);
    }

    #[test]
    fn test_simd_tokenizer_single() {
        let t = SimdWhitespaceTokenizer::new();
        assert!(t.count("hello") >= 1);
    }

    #[test]
    fn test_simd_tokenizer_sentence() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("The quick brown fox jumps over the lazy dog.");
        assert!(c >= 5);
    }

    #[test]
    fn test_simd_tokenizer_code() {
        let t = SimdWhitespaceTokenizer::new();
        let code = "fn main() { let x = 42; }";
        let c = t.count(code);
        assert!(c >= 4);
    }

    #[test]
    fn test_simd_tokenizer_unicode() {
        let t = SimdWhitespaceTokenizer::new();
        let c = t.count("こんにちは 世界 hello");
        assert!(c >= 2);
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
            let scalar_splits = count_splits_scalar(case.as_bytes());
            assert_eq!(
                simd_splits,
                scalar_splits,
                "mismatch for input of {} bytes",
                case.len()
            );
        }
    }

    #[test]
    fn test_17_byte_alignment() {
        let data = b"12345678901234567";
        let t = SimdWhitespaceTokenizer::new();
        let simd_splits = t.count_splits(data);
        let scalar_splits = count_splits_scalar(data);
        assert_eq!(simd_splits, scalar_splits);
    }

    #[test]
    fn test_33_byte_alignment() {
        let data = b"123456789012345678901234567890123";
        let t = SimdWhitespaceTokenizer::new();
        let simd_splits = t.count_splits(data);
        let scalar_splits = count_splits_scalar(data);
        assert_eq!(simd_splits, scalar_splits);
    }

    #[test]
    fn test_exact_16_bytes() {
        let data = b"1234567890123456";
        let t = SimdWhitespaceTokenizer::new();
        let simd_splits = t.count_splits(data);
        let scalar_splits = count_splits_scalar(data);
        assert_eq!(simd_splits, scalar_splits);
    }

    #[test]
    fn test_exact_32_bytes() {
        let data = b"12345678901234567890123456789012";
        let t = SimdWhitespaceTokenizer::new();
        let simd_splits = t.count_splits(data);
        let scalar_splits = count_splits_scalar(data);
        assert_eq!(simd_splits, scalar_splits);
    }

    #[test]
    fn test_count_splits_scalar_c0_controls() {
        assert_eq!(count_splits_scalar(b"a\x00b\x01c"), 3);
    }

    #[test]
    fn test_backend_name() {
        let t = SimdWhitespaceTokenizer::new();
        assert_eq!(t.backend_name(), "simd-whitespace");
    }
}
