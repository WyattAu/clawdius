//! SIMD-accelerated primitives for Clawdius.
//!
//! This crate intentionally contains `unsafe` code for SIMD intrinsics.
//! All unsafe blocks are guarded by runtime feature detection
//! (`is_x86_feature_detected!` / `is_aarch64_feature_detected!`)
//! with scalar fallbacks on unsupported platforms.
//!
//! Other Clawdius crates use `#![deny(unsafe_code)]` and call into this
//! crate's safe public API instead of inlining SIMD intrinsics.

#![allow(unsafe_code)]

// === FNV-1a + multiplicative hash (from simd.rs) ===

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

#[inline]
fn scalar_checksum(data: &[u8]) -> u64 {
    let mut hash: u64 = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[inline]
fn scalar_hash(data: &[u8]) -> u64 {
    let k1: u64 = 0x9e37_79b9_7f4a_7c15;
    let k2: u64 = 0xff51_afd7_ed55_8ccd;
    let k3: u64 = 0x87c3_7b91_1142_53d5;

    let mut h1: u64 = k1;
    let mut h2: u64 = k1;
    let mut h3: u64 = k1;
    let mut h4: u64 = k1;

    let chunks = data.chunks_exact(32);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let w0 = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let w1 = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
        let w2 = u64::from_le_bytes(chunk[16..24].try_into().unwrap());
        let w3 = u64::from_le_bytes(chunk[24..32].try_into().unwrap());

        h1 = h1.wrapping_add(w0);
        h2 = h2.wrapping_add(w1);
        h3 = h3.wrapping_add(w2);
        h4 = h4.wrapping_add(w3);

        h1 = h1.wrapping_mul(k2);
        h2 = h2.wrapping_mul(k2);
        h3 = h3.wrapping_mul(k2);
        h4 = h4.wrapping_mul(k2);

        h1 = rotate64(h1, 27);
        h2 = rotate64(h2, 27);
        h3 = rotate64(h3, 27);
        h4 = rotate64(h4, 27);

        h1 = h1.wrapping_add(h2);
        h2 = h2.wrapping_add(h3);
        h3 = h3.wrapping_add(h4);
        h4 = h4.wrapping_add(h1);
    }

    h1 = h1.wrapping_mul(k3);
    h2 = h2.wrapping_mul(k3);
    h3 = h3.wrapping_mul(k3);
    h4 = h4.wrapping_mul(k3);

    let mut combined = h1 ^ h2 ^ h3 ^ h4;

    for &b in remainder {
        combined = combined.wrapping_mul(31).wrapping_add(b as u64);
    }

    combined
}

#[inline]
const fn rotate64(x: u64, n: u32) -> u64 {
    (x << n) | (x >> (64 - n))
}

// --- SSE2 implementations ---

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fnv1a_sse2(data: &[u8]) -> u64 {
    let mut hash: u64 = FNV_OFFSET_BASIS;
    let mut i = 0;
    let len = data.len();

    while i + 8 <= len {
        let val = (data.as_ptr().add(i) as *const u64).read_unaligned();
        for shift in 0..8 {
            let byte = ((val >> (shift * 8)) & 0xFF) as u8;
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        i += 8;
    }

    while i < len {
        hash ^= data[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }

    hash
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn hash_sse2(data: &[u8]) -> u64 {
    let len = data.len();
    if len < 32 {
        return scalar_hash(data);
    }

    let k2: u64 = 0xff51_afd7_ed55_8ccd;
    let k3: u64 = 0x87c3_7b91_1142_53d5;
    let k1: u64 = 0x9e37_79b9_7f4a_7c15;

    let mut h1: u64 = k1;
    let mut h2: u64 = k1;
    let mut h3: u64 = k1;
    let mut h4: u64 = k1;

    let chunks = data.chunks_exact(32);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let v1 = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
        let v2 = _mm_loadu_si128(chunk.as_ptr().add(16) as *const __m128i);

        let w0 = _mm_cvtsi128_si64(v1) as u64;
        let w1 = _mm_extract_epi64(v1, 1) as u64;
        let w2 = _mm_cvtsi128_si64(v2) as u64;
        let w3 = _mm_extract_epi64(v2, 1) as u64;

        h1 = h1.wrapping_add(w0);
        h2 = h2.wrapping_add(w1);
        h3 = h3.wrapping_add(w2);
        h4 = h4.wrapping_add(w3);

        h1 = h1.wrapping_mul(k2);
        h2 = h2.wrapping_mul(k2);
        h3 = h3.wrapping_mul(k2);
        h4 = h4.wrapping_mul(k2);

        h1 = rotate64(h1, 27);
        h2 = rotate64(h2, 27);
        h3 = rotate64(h3, 27);
        h4 = rotate64(h4, 27);

        h1 = h1.wrapping_add(h2);
        h2 = h2.wrapping_add(h3);
        h3 = h3.wrapping_add(h4);
        h4 = h4.wrapping_add(h1);
    }

    h1 = h1.wrapping_mul(k3);
    h2 = h2.wrapping_mul(k3);
    h3 = h3.wrapping_mul(k3);
    h4 = h4.wrapping_mul(k3);

    let mut combined = h1 ^ h2 ^ h3 ^ h4;

    for &b in remainder {
        combined = combined.wrapping_mul(31).wrapping_add(b as u64);
    }

    combined
}

// --- NEON implementations ---

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn fnv1a_neon(data: &[u8]) -> u64 {
    let mut hash: u64 = FNV_OFFSET_BASIS;
    let mut i = 0;
    let len = data.len();

    while i + 8 <= len {
        let val = (data.as_ptr().add(i) as *const u64).read_unaligned();
        for shift in 0..8 {
            let byte = ((val >> (shift * 8)) & 0xFF) as u8;
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        i += 8;
    }

    while i < len {
        hash ^= data[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }

    hash
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn hash_neon(data: &[u8]) -> u64 {
    let len = data.len();
    if len < 32 {
        return scalar_hash(data);
    }

    let k1: u64 = 0x9e3779b97f4a7c15;
    let k2: u64 = 0xff51afd7ed558ccd;
    let k3: u64 = 0x87c37b91114253d5;

    let mut h1: u64 = k1;
    let mut h2: u64 = k1;
    let mut h3: u64 = k1;
    let mut h4: u64 = k1;

    let chunks = data.chunks_exact(32);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let v1 = vld1q_u8(chunk.as_ptr());
        let v2 = vld1q_u8(chunk.as_ptr().add(16));

        let w0 = vgetq_lane_u64(vreinterpretq_u64_u8(v1), 0);
        let w1 = vgetq_lane_u64(vreinterpretq_u64_u8(v1), 1);
        let w2 = vgetq_lane_u64(vreinterpretq_u64_u8(v2), 0);
        let w3 = vgetq_lane_u64(vreinterpretq_u64_u8(v2), 1);

        h1 = h1.wrapping_add(w0);
        h2 = h2.wrapping_add(w1);
        h3 = h3.wrapping_add(w2);
        h4 = h4.wrapping_add(w3);

        h1 = h1.wrapping_mul(k2);
        h2 = h2.wrapping_mul(k2);
        h3 = h3.wrapping_mul(k2);
        h4 = h4.wrapping_mul(k2);

        h1 = rotate64(h1, 27);
        h2 = rotate64(h2, 27);
        h3 = rotate64(h3, 27);
        h4 = rotate64(h4, 27);

        h1 = h1.wrapping_add(h2);
        h2 = h2.wrapping_add(h3);
        h3 = h3.wrapping_add(h4);
        h4 = h4.wrapping_add(h1);
    }

    h1 = h1.wrapping_mul(k3);
    h2 = h2.wrapping_mul(k3);
    h3 = h3.wrapping_mul(k3);
    h4 = h4.wrapping_mul(k3);

    let mut combined = h1 ^ h2 ^ h3 ^ h4;

    for &b in remainder {
        combined = combined.wrapping_mul(31).wrapping_add(b as u64);
    }

    combined
}

// === Public API: Hash + Checksum ===

/// FNV-1a 64-bit checksum with SIMD acceleration.
///
/// Returns identical results on all platforms for the same input.
#[cfg(target_arch = "x86_64")]
pub fn fast_checksum(data: &[u8]) -> u64 {
    if is_x86_feature_detected!("sse2") {
        unsafe { fnv1a_sse2(data) }
    } else {
        scalar_checksum(data)
    }
}

/// FNV-1a 64-bit checksum with SIMD acceleration.
#[cfg(target_arch = "aarch64")]
pub fn fast_checksum(data: &[u8]) -> u64 {
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe { fnv1a_neon(data) }
    } else {
        scalar_checksum(data)
    }
}

/// FNV-1a 64-bit checksum (scalar fallback).
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn fast_checksum(data: &[u8]) -> u64 {
    scalar_checksum(data)
}

/// Parallel 4-lane multiplicative hash with SIMD acceleration.
#[cfg(target_arch = "x86_64")]
pub fn fast_hash(data: &[u8]) -> u64 {
    if is_x86_feature_detected!("sse2") {
        unsafe { hash_sse2(data) }
    } else {
        scalar_hash(data)
    }
}

/// Parallel 4-lane multiplicative hash with SIMD acceleration.
#[cfg(target_arch = "aarch64")]
pub fn fast_hash(data: &[u8]) -> u64 {
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe { hash_neon(data) }
    } else {
        scalar_hash(data)
    }
}

/// Parallel 4-lane multiplicative hash (scalar fallback).
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn fast_hash(data: &[u8]) -> u64 {
    scalar_hash(data)
}

// === Public API: SIMD whitespace split counting ===

/// Scalar whitespace boundary counter.
///
/// Counts the number of whitespace-delimited segments in `data`.
/// Whitespace is any byte <= 0x20.
pub fn count_splits_scalar(data: &[u8]) -> usize {
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

/// Count whitespace-delimited segments using the fastest available path.
///
/// Uses SSE2 on x86_64, NEON on aarch64, scalar fallback otherwise.
pub fn count_splits(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse2") {
            return unsafe { count_splits_sse2(data) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { count_splits_neon(data) };
        }
    }

    count_splits_scalar(data)
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn whitespace_mask_sse2(chunk: __m128i) -> __m128i {
    let thresholds = _mm_set1_epi8(0x21);
    _mm_cmplt_epi8(chunk, thresholds)
}

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
    fn test_empty() {
        assert_eq!(fast_checksum(b""), scalar_checksum(b""));
        assert_eq!(fast_hash(b""), scalar_hash(b""));
    }

    #[test]
    fn test_single_byte() {
        assert_eq!(fast_checksum(b"A"), scalar_checksum(b"A"));
        assert_eq!(fast_hash(b"A"), scalar_hash(b"A"));
    }

    #[test]
    fn test_short_data() {
        let data = b"hello world";
        assert_eq!(fast_checksum(data), scalar_checksum(data));
        assert_eq!(fast_hash(data), scalar_hash(data));
    }

    #[test]
    fn test_exact_32_bytes() {
        let data = b"12345678901234567890123456789012";
        assert_eq!(fast_checksum(data), scalar_checksum(data));
        assert_eq!(fast_hash(data), scalar_hash(data));
    }

    #[test]
    fn test_1kb() {
        let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        assert_eq!(fast_checksum(&data), scalar_checksum(&data));
        assert_eq!(fast_hash(&data), scalar_hash(&data));
    }

    #[test]
    fn test_deterministic() {
        let data = b"The quick brown fox jumps over the lazy dog";
        assert_eq!(fast_checksum(data), fast_checksum(data));
        assert_eq!(fast_hash(data), fast_hash(data));
    }

    #[test]
    fn test_different_inputs_different_hashes() {
        assert_ne!(fast_checksum(b"input A"), fast_checksum(b"input B"));
        assert_ne!(fast_hash(b"input A"), fast_hash(b"input B"));
    }

    #[test]
    fn test_count_splits_empty() {
        assert_eq!(count_splits(b""), 0);
    }

    #[test]
    fn test_count_splits_single_word() {
        assert_eq!(count_splits(b"hello"), 1);
    }

    #[test]
    fn test_count_splits_two_words() {
        assert_eq!(count_splits(b"hello world"), 2);
    }

    #[test]
    fn test_count_splits_whitespace_only() {
        assert_eq!(count_splits(b"   \t\n"), 0);
    }

    #[test]
    fn test_count_splits_mixed() {
        assert_eq!(count_splits(b"a b\tc\nd"), 4);
    }

    #[test]
    fn test_count_splits_matches_scalar() {
        let cases: &[&[u8]] = &[
            b"",
            b"a",
            b"hello world",
            b"  leading and trailing  ",
            b"tabs\there\ttoo",
            b"new\nlines\nhere",
        ];
        for &case in cases {
            assert_eq!(
                count_splits(case),
                count_splits_scalar(case),
                "mismatch for {:?}",
                case
            );
        }
    }
}
