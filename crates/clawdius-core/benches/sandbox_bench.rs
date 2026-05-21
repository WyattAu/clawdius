#![allow(
    missing_docs,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::semicolon_if_nothing_returned
)]

//! Sandbox backend benchmark suite
//!
//! Measures execution overhead for each available sandbox backend.
//! Run with: cargo bench -p clawdius-core --bench sandbox_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::Path;
use std::time::Duration;

use clawdius_core::sandbox::backends::{
    detect_best_backend, DirectBackend, FilteredBackend, SandboxBackend,
};
use clawdius_core::sandbox::tiers::SandboxConfig;
use clawdius_core::sandbox::SandboxTier;

/// Simple echo command that should complete in microseconds
const BENCH_COMMAND: &str = "echo";
const BENCH_ARGS: &[&str] = &["hello"];
const BENCH_CWD: &str = ".";

const fn make_config(tier: SandboxTier) -> SandboxConfig {
    SandboxConfig {
        tier,
        network: false,
        mounts: vec![],
    }
}

fn bench_direct_backend(c: &mut Criterion) {
    let config = make_config(SandboxTier::TrustedAudited);
    let backend = DirectBackend::new(config);
    let cwd = Path::new(BENCH_CWD);

    let mut group = c.benchmark_group("sandbox/backend");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));
    group.bench_function(BenchmarkId::new("echo", "direct"), |b| {
        b.iter(|| {
            let result = backend.execute(
                black_box(BENCH_COMMAND),
                black_box(BENCH_ARGS),
                black_box(cwd),
            );
            assert!(result.is_ok(), "direct backend failed: {:?}", result.err());
        });
    });
    group.finish();
}

fn bench_filtered_backend(c: &mut Criterion) {
    let config = make_config(SandboxTier::Trusted);
    let backend = FilteredBackend::new(config);
    let cwd = Path::new(BENCH_CWD);

    let mut group = c.benchmark_group("sandbox/backend");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));
    group.bench_function(BenchmarkId::new("echo", "filtered"), |b| {
        b.iter(|| {
            let result = backend.execute(
                black_box(BENCH_COMMAND),
                black_box(BENCH_ARGS),
                black_box(cwd),
            );
            assert!(
                result.is_ok(),
                "filtered backend failed: {:?}",
                result.err()
            );
        });
    });
    group.finish();
}

/// Benchmark sandbox executor overhead (tier selection + backend dispatch)
fn bench_executor_overhead(c: &mut Criterion) {
    use clawdius_core::sandbox::executor::SandboxExecutor;

    let cwd = Path::new(BENCH_CWD);

    // TrustedAudited uses DirectBackend (zero overhead baseline)
    let config = make_config(SandboxTier::TrustedAudited);
    if let Ok(executor) = SandboxExecutor::new(SandboxTier::TrustedAudited, config) {
        let mut group = c.benchmark_group("sandbox/executor");
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(2));
        group.bench_function(BenchmarkId::new("execute", "trusted_audited"), |b| {
            b.iter(|| {
                let result = executor.execute(black_box(BENCH_COMMAND), black_box(BENCH_ARGS), cwd);
                assert!(result.is_ok());
            });
        });
        group.finish();
    }

    // Trusted uses FilteredBackend (blocklist check overhead)
    let config = make_config(SandboxTier::Trusted);
    if let Ok(executor) = SandboxExecutor::new(SandboxTier::Trusted, config) {
        let mut group = c.benchmark_group("sandbox/executor");
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(2));
        group.bench_function(BenchmarkId::new("execute", "trusted"), |b| {
            b.iter(|| {
                let result = executor.execute(black_box(BENCH_COMMAND), black_box(BENCH_ARGS), cwd);
                assert!(result.is_ok());
            });
        });
        group.finish();
    }

    // Untrusted uses best available (may be bubblewrap, container, or filtered)
    let config = make_config(SandboxTier::Untrusted);
    let executor = SandboxExecutor::new_with_fallback(SandboxTier::Untrusted, config);
    let mut group = c.benchmark_group("sandbox/executor");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function(
        BenchmarkId::new(
            "execute",
            format!("untrusted ({})", executor.backend_name()),
        ),
        |b| {
            b.iter(|| {
                let result = executor.execute(black_box(BENCH_COMMAND), black_box(BENCH_ARGS), cwd);
                // May succeed or fail depending on sandbox availability
                let _ = black_box(result);
            });
        },
    );
    group.finish();
}

/// Benchmark backend detection cost
fn bench_backend_detection(c: &mut Criterion) {
    c.bench_function("sandbox/detect_best_backend", |b| {
        b.iter(|| {
            let name = detect_best_backend();
            black_box(name);
        });
    });
}

criterion_group!(
    benches,
    bench_direct_backend,
    bench_filtered_backend,
    bench_executor_overhead,
    bench_backend_detection,
);
criterion_main!(benches);
