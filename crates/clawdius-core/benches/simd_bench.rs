//! SIMD Checksum & Hash Benchmarks
//!
//! Compares SIMD-accelerated vs scalar checksum/hash on various data sizes.

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
                        hash ^= byte as u64;
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

criterion_group!(
    benches,
    bench_checksum,
    bench_hash,
    bench_checksum_vs_scalar
);
criterion_main!(benches);
