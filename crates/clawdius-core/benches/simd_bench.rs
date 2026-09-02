//! SIMD Checksum & Hash Benchmarks
//!
//! Compares SIMD-accelerated vs scalar checksum/hash on various data sizes.

#![allow(
    dead_code,
    missing_docs,
    unused_imports,
    unused_variables,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_lazy_continuation,
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::format_collect,
    clippy::from_over_into,
    clippy::ignored_unit_patterns,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::manual_is_multiple_of,
    clippy::match_single_binding,
    clippy::missing_const_for_fn,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::unwrap_used
)]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as std_black_box;

fn make_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

fn bench_checksum(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd/checksum");
    group.sample_size(1_000);

    for &size in &[64, 1024, 65536, 1_048_576] {
        let data = make_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("fast_checksum", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| black_box(clawdius_core::simd::fast_checksum(black_box(data))));
            },
        );
    }

    group.finish();
}

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd/hash");
    group.sample_size(1_000);

    for &size in &[64, 1024, 65536, 1_048_576] {
        let data = make_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("fast_hash", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| black_box(clawdius_core::simd::fast_hash(black_box(data))));
            },
        );
    }

    group.finish();
}

fn bench_checksum_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd/checksum_vs_scalar");
    group.sample_size(1_000);

    for &size in &[1024, 65536, 1_048_576] {
        let data = make_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("simd_checksum", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| black_box(clawdius_core::simd::fast_checksum(black_box(data))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scalar_checksum", format_size(size)),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut hash: u64 = 0xcbf29ce484222325;
                    for &byte in data {
                        hash ^= u64::from(byte);
                        hash = hash.wrapping_mul(0x100000001b3);
                    }
                    std_black_box(hash)
                });
            },
        );
    }

    group.finish();
}

fn format_size(bytes: usize) -> String {
    match bytes {
        64 => "64B".to_string(),
        1024 => "1KB".to_string(),
        65536 => "64KB".to_string(),
        1_048_576 => "1MB".to_string(),
        _ => format!("{bytes}B"),
    }
}

fn bench_xor_simd_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd/xor_vs_scalar");
    group.sample_size(1_000);
    let key = b"aes-256-gcm-key-simd!!";

    for &size in &[64, 1024, 65536, 1_048_576] {
        let data = make_test_data(size);
        group.throughput(Throughput::Bytes(size as u64));

        // SIMD-accelerated XOR (u128 chunks)
        group.bench_with_input(
            BenchmarkId::new("xor_simd_u128", format_size(size)),
            &data,
            |b, data| {
                let mut out = vec![0u8; data.len()];
                b.iter(|| {
                    clawdius_core::encryption::xor_encrypt(
                        black_box(&mut out),
                        black_box(data),
                        key,
                    );
                    black_box(&out);
                });
            },
        );

        // Scalar XOR (byte-by-byte)
        group.bench_with_input(
            BenchmarkId::new("xor_scalar", format_size(size)),
            &data,
            |b, data| {
                let mut out = vec![0u8; data.len()];
                b.iter(|| {
                    for (i, byte) in data.iter().enumerate() {
                        out[i] = byte ^ key[i % key.len()];
                    }
                    black_box(&out);
                });
            },
        );

        // In-place SIMD XOR
        group.bench_with_input(
            BenchmarkId::new("xor_inplace_simd", format_size(size)),
            &data,
            |b, data| {
                b.iter({
                    let data = data.clone();
                    let key = *key;
                    move || {
                        let mut buf = data.clone();
                        clawdius_core::encryption::xor_inplace(black_box(&mut buf), &key);
                        black_box(&buf);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_checksum,
    bench_hash,
    bench_checksum_vs_scalar,
    bench_xor_simd_vs_scalar
);
criterion_main!(benches);
