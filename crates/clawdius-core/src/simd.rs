//! SIMD-accelerated checksum and hash utilities.
//!
//! Provides platform-optimized implementations using SSE2 (x86_64) or NEON (aarch64)
//! with scalar fallbacks for all other targets. SIMD and scalar variants produce
//! identical results for any given input.
//!
//! `fast_checksum` — FNV-1a 64-bit with SIMD-accelerated byte loading.
//! `fast_hash` — Parallel multiplicative hash with 4-lane accumulation.

#![allow(unsafe_code)]

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

#[cfg(target_arch = "x86_64")]
pub fn fast_checksum(data: &[u8]) -> u64 {
    if is_x86_feature_detected!("sse2") {
        unsafe { fnv1a_sse2(data) }
    } else {
        scalar_checksum(data)
    }
}

#[cfg(target_arch = "x86_64")]
pub fn fast_hash(data: &[u8]) -> u64 {
    if is_x86_feature_detected!("sse2") {
        unsafe { hash_sse2(data) }
    } else {
        scalar_hash(data)
    }
}

#[cfg(target_arch = "aarch64")]
pub fn fast_checksum(data: &[u8]) -> u64 {
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe { fnv1a_neon(data) }
    } else {
        scalar_checksum(data)
    }
}

#[cfg(target_arch = "aarch64")]
pub fn fast_hash(data: &[u8]) -> u64 {
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe { hash_neon(data) }
    } else {
        scalar_hash(data)
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn fast_checksum(data: &[u8]) -> u64 {
    scalar_checksum(data)
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn fast_hash(data: &[u8]) -> u64 {
    scalar_hash(data)
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
    fn test_exact_16_bytes() {
        let data = b"1234567890123456";
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
    fn test_64_bytes() {
        let data: Vec<u8> = (0..64).map(|i| i as u8).collect();
        assert_eq!(fast_checksum(&data), scalar_checksum(&data));
        assert_eq!(fast_hash(&data), scalar_hash(&data));
    }

    #[test]
    fn test_1kb() {
        let data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        assert_eq!(fast_checksum(&data), scalar_checksum(&data));
        assert_eq!(fast_hash(&data), scalar_hash(&data));
    }

    #[test]
    fn test_64kb() {
        let data: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
        assert_eq!(fast_checksum(&data), scalar_checksum(&data));
        assert_eq!(fast_hash(&data), scalar_hash(&data));
    }

    #[test]
    fn test_17_bytes_partial_chunk() {
        let data = b"12345678901234567";
        assert_eq!(fast_checksum(data), scalar_checksum(data));
        assert_eq!(fast_hash(data), scalar_hash(data));
    }

    #[test]
    fn test_33_bytes_partial_chunk() {
        let data = b"123456789012345678901234567890123";
        assert_eq!(fast_checksum(data), scalar_checksum(data));
        assert_eq!(fast_hash(data), scalar_hash(data));
    }

    #[test]
    fn test_deterministic() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let a = fast_checksum(data);
        let b = fast_checksum(data);
        assert_eq!(a, b);

        let c = fast_hash(data);
        let d = fast_hash(data);
        assert_eq!(c, d);
    }

    #[test]
    fn test_different_inputs_different_hashes() {
        let a = fast_checksum(b"input A");
        let b = fast_checksum(b"input B");
        assert_ne!(a, b);

        let c = fast_hash(b"input A");
        let d = fast_hash(b"input B");
        assert_ne!(c, d);
    }
}
