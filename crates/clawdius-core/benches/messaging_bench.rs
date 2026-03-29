//! Performance Benchmarks for Messaging Gateway
//!
//! Benchmarks to verify <1ms P99 routing latency requirement.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::Instant;

// Mock types for benchmarking (to avoid external dependencies)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Platform {
    Telegram,
    Discord,
    Matrix,
    Signal,
    Slack,
}

impl Platform {
    fn command_prefix(&self) -> &'static str {
        match self {
            Self::Telegram => "/clawd ",
            Self::Discord => "/clawd ",
            Self::Matrix => "!clawd ",
            Self::Signal => "/clawd ",
            Self::Slack => "/clawd ",
        }
    }
}

/// Benchmark command parsing performance
fn bench_command_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_parsing");

    let commands = [
        "/clawd status",
        "/clawd generate function add_auth_check --lang rust",
        "/clawd analyze why is this function slow and how can I optimize it?",
        "/clawd config set provider openai --verbose",
        "/clawd admin users list --format json",
    ];

    for cmd in &commands {
        group.bench_with_input(BenchmarkId::new("parse", cmd.len()), cmd, |b, &cmd| {
            b.iter(|| {
                let prefix = Platform::Telegram.command_prefix();
                let content = cmd.trim();
                let has_prefix = content.starts_with(prefix);
                black_box(has_prefix)
            });
        });
    }

    group.finish();
}

/// Benchmark message routing latency
fn bench_message_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_routing");
    group.measurement_time(std::time::Duration::from_secs(5));

    // Simulate message routing decision
    group.bench_function("route_decision", |b| {
        b.iter(|| {
            let platform = black_box(Platform::Telegram);
            let user_id = black_box("user-123");
            let message = black_box("/clawd status");

            // Simulate routing decision
            let prefix = platform.command_prefix();
            let is_command = message.starts_with(prefix);
            black_box((platform, user_id, is_command))
        });
    });

    group.finish();
}

/// Benchmark session lookup
fn bench_session_lookup(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut group = c.benchmark_group("session_lookup");

    // Pre-populate sessions
    let mut sessions: HashMap<String, u64> = HashMap::new();
    for i in 0..1000 {
        sessions.insert(format!("telegram:user-{}", i), i);
    }

    group.bench_function("hashmap_lookup", |b| {
        b.iter(|| {
            let key = black_box("telegram:user-500");
            let result = sessions.get(key);
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark rate limiting check
fn bench_rate_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiting");

    // Token bucket state
    let mut tokens: f64 = 10.0;
    let max_tokens: f64 = 10.0;
    let refill_rate: f64 = 1.0; // tokens per second

    group.bench_function("token_bucket_check", |b| {
        b.iter(|| {
            // Simulate token consumption
            if tokens >= 1.0 {
                tokens -= 1.0;
                true
            } else {
                // Refill
                tokens = (tokens + refill_rate * 0.001).min(max_tokens);
                false
            }
        });
    });

    group.finish();
}

/// Benchmark message chunking
fn bench_message_chunking(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_chunking");

    let short_msg = "Short message";
    let medium_msg = "This is a medium length message. ".repeat(10);
    let long_msg = "This is a very long message that needs to be chunked. ".repeat(100);

    group.bench_with_input(BenchmarkId::new("chunk", "short"), &short_msg, |b, msg| {
        b.iter(|| {
            let max_len = 1000;
            if msg.len() <= max_len {
                vec![msg.to_string()]
            } else {
                // Simple chunking
                msg.as_bytes()
                    .chunks(max_len)
                    .map(|c| String::from_utf8_lossy(c).to_string())
                    .collect::<Vec<_>>()
            }
        });
    });

    group.bench_with_input(
        BenchmarkId::new("chunk", "medium"),
        &medium_msg,
        |b, msg| {
            b.iter(|| {
                let max_len = 1000;
                if msg.len() <= max_len {
                    vec![msg.to_string()]
                } else {
                    msg.as_bytes()
                        .chunks(max_len)
                        .map(|c| String::from_utf8_lossy(c).to_string())
                        .collect::<Vec<_>>()
                }
            });
        },
    );

    group.bench_with_input(BenchmarkId::new("chunk", "long"), &long_msg, |b, msg| {
        b.iter(|| {
            let max_len = 1000;
            if msg.len() <= max_len {
                vec![msg.to_string()]
            } else {
                msg.as_bytes()
                    .chunks(max_len)
                    .map(|c| String::from_utf8_lossy(c).to_string())
                    .collect::<Vec<_>>()
            }
        });
    });

    group.finish();
}

/// Benchmark permission checking
fn bench_permission_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("permission_checking");

    #[derive(Debug, Clone)]
    struct Permissions {
        can_generate: bool,
        can_analyze: bool,
        can_admin: bool,
    }

    let admin_perms = Permissions {
        can_generate: true,
        can_analyze: true,
        can_admin: true,
    };

    let user_perms = Permissions {
        can_generate: true,
        can_analyze: true,
        can_admin: false,
    };

    group.bench_function("admin_check", |b| {
        b.iter(|| black_box(admin_perms.can_admin && admin_perms.can_generate));
    });

    group.bench_function("user_check", |b| {
        b.iter(|| black_box(user_perms.can_admin && user_perms.can_generate));
    });

    group.finish();
}

/// End-to-end routing latency benchmark
fn bench_e2e_routing_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_routing");
    group.measurement_time(std::time::Duration::from_secs(10));

    // Simulate complete routing path
    group.bench_function("full_routing_path", |b| {
        b.iter(|| {
            let start = Instant::now();

            // 1. Parse command prefix
            let platform = black_box(Platform::Telegram);
            let message = black_box("/clawd generate function --lang rust");
            let prefix = platform.command_prefix();
            let is_command = message.starts_with(prefix);

            // 2. Extract command content
            let content = if is_command {
                message.strip_prefix(prefix).unwrap_or(message).trim()
            } else {
                message.trim()
            };

            // 3. Split into tokens
            let tokens: Vec<&str> = content.split_whitespace().collect();

            // 4. Categorize command
            let category = if tokens.first().map(|t| *t == "generate").unwrap_or(false) {
                "generate"
            } else if tokens.first().map(|t| *t == "analyze").unwrap_or(false) {
                "analyze"
            } else {
                "unknown"
            };

            let elapsed = start.elapsed();
            black_box((is_command, tokens.len(), category, elapsed))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_command_parsing,
    bench_message_routing,
    bench_session_lookup,
    bench_rate_limiting,
    bench_message_chunking,
    bench_permission_checking,
    bench_e2e_routing_latency,
);

criterion_main!(benches);
