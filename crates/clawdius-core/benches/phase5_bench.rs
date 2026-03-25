//! Phase 5 Agentic Features Benchmarks
//!
//! Benchmarks for:
//! - Rate limiter throughput
//! - Streaming generation latency
//! - Incremental generation speedup
//! - Architecture drift detection
//! - Technical debt analysis

use clawdius_core::{
    analysis::{debt::DebtAnalyzer, drift::DriftDetector},
    llm::rate_limiter::{RateLimiter, RateLimiterConfig},
    agentic::{
        incremental::IncrementalGenerator,
        streaming_generator::{StreamChunk, StreamProcessor, StreamingCodeGenerator},
    },
    timeout::{TimeoutConfig, TimeoutGuard},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// Rate Limiter Benchmarks
// =================================================================

fn bench_rate_limiter(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiter");

    // High throughput configuration
    let high_throughput_config = RateLimiterConfig {
        requests_per_minute: 6000, // 100 per second
        burst_capacity: 50,
        adaptive: true,
    };

    // Conservative configuration
    let conservative_config = RateLimiterConfig {
        requests_per_minute: 600, // 10 per second
        burst_capacity: 5,
        adaptive: true,
    };

    // Benchmark acquire operation - high throughput
    group.bench_function("acquire_high_throughput", |b| {
        let limiter = RateLimiter::new(high_throughput_config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            let limiter = limiter.clone();
            async move {
                let _permit = limiter.acquire().await.unwrap();
                black_box(_permit)
            }
        });
    });

    // Benchmark acquire operation - conservative
    group.bench_function("acquire_conservative", |b| {
        let limiter = RateLimiter::new(conservative_config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            let limiter = limiter.clone();
            async move {
                let _permit = limiter.acquire().await.unwrap();
                black_box(_permit)
            }
        });
    });

    // Benchmark burst handling
    group.bench_function("burst_10_requests", |b| {
        let limiter = RateLimiter::new(high_throughput_config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        b.to_async(&rt).iter(|| {
            let limiter = limiter.clone();
            async move {
                for _ in 0..10 {
                    let _permit = limiter.acquire().await.unwrap();
                    black_box(_permit);
                }
            }
        });
    });

    group.finish();
}

// ============================================================================
// Streaming Benchmarks
// =================================================================

fn bench_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming");

    // Benchmark stream chunk creation
    group.bench_function("chunk_creation", |b| {
        b.iter(|| {
            let chunk = StreamChunk::new("Hello, world!".to_string(), false);
            black_box(chunk)
        });
    });

    // Benchmark stream processor
    group.bench_function("processor_accumulate", |b| {
        let mut processor = StreamProcessor::new();
        let chunks: Vec<StreamChunk> = (0..100)
            .map(|i| StreamChunk::new(format!("chunk{} ", i), i == 99))
            .collect();

        b.iter(|| {
            processor.reset();
            for chunk in &chunks {
                processor.process_chunk(chunk);
            }
            black_box(processor.get_accumulated())
        });
    });

    // Benchmark callback invocation
    group.bench_function("callback_invocation", |b| {
        let mut processor = StreamProcessor::new();
        let callback = |content: &str, is_complete: bool| {
            black_box((content, is_complete));
        };
        processor.set_callback(callback);

        let chunk = StreamChunk::new("test".to_string(), false);

        b.iter(|| {
            processor.process_chunk(&chunk);
        });
    });

    group.finish();
}

// ============================================================================
// Incremental Generation Benchmarks
// =================================================================

fn bench_incremental(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental");

    // Small file content
    let small_content: String = (0..50)
        .map(|i| format!("fn function_{}() {{\n    // implementation\n}}\n\n", i))
        .collect();

    // Medium file content
    let medium_content: String = (0..200)
        .map(|i| format!("fn function_{}() {{\n    // implementation\n}}\n\n", i))
        .collect();

    // Large file content
    let large_content: String = (0..500)
        .map(|i| format!("fn function_{}() {{\n    // implementation\n}}\n\n", i))
        .collect();

    // Benchmark chunk splitting
    group.throughput(Throughput::Bytes(small_content.len() as u64));
    group.bench_with_input(BenchmarkId::new("split_chunks", "small"), &small_content, |b, content| {
        b.iter(|| {
            let generator = IncrementalGenerator::new(content.clone());
            black_box(generator)
        });
    });

    group.throughput(Throughput::Bytes(medium_content.len() as u64));
    group.bench_with_input(BenchmarkId::new("split_chunks", "medium"), &medium_content, |b, content| {
        b.iter(|| {
            let generator = IncrementalGenerator::new(content.clone());
            black_box(generator)
        });
    });

    group.throughput(Throughput::Bytes(large_content.len() as u64));
    group.bench_with_input(BenchmarkId::new("split_chunks", "large"), &large_content, |b, content| {
        b.iter(|| {
            let generator = IncrementalGenerator::new(content.clone());
            black_box(generator)
        });
    });

    group.finish();
}

// ============================================================================
// Timeout Benchmarks
// =================================================================

fn bench_timeout(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeout");

    // Benchmark guard creation
    group.bench_function("guard_creation", |b| {
        b.iter(|| {
                let guard = TimeoutGuard::with_label(Duration::from_secs(30), "test");
                black_box(guard)
            });
    });

    // Benchmark remaining time check
    group.bench_function("remaining_time_check", |b| {
        let guard = TimeoutGuard::with_label(Duration::from_secs(30), "test");
        b.iter(|| {
            black_box(guard.remaining())
        });
    });

    // Benchmark is_expired check
    group.bench_function("is_expired_check", |b| {
        let guard = TimeoutGuard::with_label(Duration::from_secs(30), "test");
        b.iter(|| {
            black_box(guard.is_expired())
        });
    });

    // Benchmark config creation
    group.bench_function("config_creation", |b| {
        b.iter(|| {
            let config = TimeoutConfig::default();
                black_box(config)
        });
    });

    group.finish();
}

// ============================================================================
// Drift Detection Benchmarks
// =================================================================

fn bench_drift_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("drift_detection");

    let detector = DriftDetector::new();

    // Clean code (no drifts)
    let clean_code = r#"
fn clean_function(x: i32) -> i32 {
    x + 1
}

fn another_function() -> String {
    "hello".to_string()
}
"#;

    // Code with drifts (TODO, unwrap, etc.)
    let drift_code = r#"
fn problematic_function(x: Option<i32>) -> i32 {
    // TODO: handle none case
    x.unwrap()
}

fn another_problematic() -> String {
    // FIXME: this is bad
    unsafe { std::ptr::null_mut() }
}

fn magic_numbers() -> i32 {
    42 // magic number
}
"#;

    // Large file with many drifts
    let large_drift_code: String = (0..100)
        .map(|i| format!(
            "fn func_{}() {{\n    // TODO: implement\n    let x = Some({}).unwrap();\n}}\n\n",
            i, i
        ))
        .collect();

    // Benchmark clean code analysis
    group.throughput(Throughput::Bytes(clean_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "clean"), &clean_code, |b, code| {
        b.iter(|| {
            black_box(detector.analyze_file(PathBuf::from("clean.rs"), code))
        });
    });

    // Benchmark drift code analysis
    group.throughput(Throughput::Bytes(drift_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "drifts"), &drift_code, |b, code| {
        b.iter(|| {
            black_box(detector.analyze_file(PathBuf::from("drift.rs"), code))
        });
    });

    // Benchmark large file analysis
    group.throughput(Throughput::Bytes(large_drift_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "large"), &large_drift_code, |b, code| {
        b.iter(|| {
            black_box(detector.analyze_file(PathBuf::from("large.rs"), code))
        });
    });

    group.finish();
}

// ============================================================================
// Debt Analysis Benchmarks
// =================================================================

fn bench_debt_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("debt_analysis");

    let analyzer = DebtAnalyzer::new();

    // Clean code (no debt)
    let clean_code = r#"
/// A well-documented function
fn clean_function(x: i32) -> i32 {
    x + 1
}

/// Another well-documented function
fn another_function() -> String {
    "hello".to_string()
}
"#;

    // Code with debt (complexity, duplication, etc.)
    let debt_code = r#"
fn complex_function(x: i32, y: i32, z: i32, w: i32) -> i32 {
    if x > 0 {
        if y > 0 {
            if z > 0 {
                if w > 0 {
                    x + y + z + w
                } else {
                    x + y + z
                }
            } else {
                x + y
            }
        } else {
            x
        }
    } else {
        0
    }
}

fn duplicated_logic_a() -> i32 {
    1 + 2 + 3
}

fn duplicated_logic_b() -> i32 {
    1 + 2 + 3
}
"#;

    // Large file with various debt
    let large_debt_code: String = (0..50)
        .map(|i| format!(
            "fn func_{}() {{\n    // undocumented\n    let x = {} + {};\n}}\n\n",
            i, i, i + 1
        ))
        .collect();

    // Benchmark clean code analysis
    group.throughput(Throughput::Bytes(clean_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "clean"), &clean_code, |b, code| {
        b.iter(|| {
            black_box(analyzer.analyze_file(PathBuf::from("clean.rs"), code))
        });
    });

    // Benchmark debt code analysis
    group.throughput(Throughput::Bytes(debt_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "debt"), &debt_code, |b, code| {
        b.iter(|| {
            black_box(analyzer.analyze_file(PathBuf::from("debt.rs"), code))
        });
    });

    // Benchmark large file analysis
    group.throughput(Throughput::Bytes(large_debt_code.len() as u64));
    group.bench_with_input(BenchmarkId::new("analyze", "large"), &large_debt_code, |b, code| {
        b.iter(|| {
            black_box(analyzer.analyze_file(PathBuf::from("large.rs"), code))
        });
    });

    group.finish();
}

// ============================================================================
// Combined Integration Benchmarks
// =================================================================

fn bench_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase5_integration");

    // Benchmark drift + debt combined analysis
    group.bench_function("drift_debt_combined", |b| {
        let drift_detector = DriftDetector::new();
        let debt_analyzer = DebtAnalyzer::new();
        let code = r#"
fn problematic() {
    // TODO: fix this
    let x = Some(1).unwrap();
}
"#;

        b.iter(|| {
            let drifts = drift_detector.analyze_file(PathBuf::from("test.rs"), code);
            let debts = debt_analyzer.analyze_file(PathBuf::from("test.rs"), code);
            black_box((drifts, debts))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_rate_limiter,
    bench_streaming,
    bench_incremental,
    bench_timeout,
    bench_drift_detection,
    bench_debt_analysis,
    bench_integration,
);

criterion_main!(benches);
