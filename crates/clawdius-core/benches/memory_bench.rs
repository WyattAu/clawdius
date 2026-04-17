//! Memory profiling benchmark.
//!
//! Measures per-session memory overhead by creating N sessions with M messages
//! each and tracking RSS growth and SQLite database file size.
//!
//! Run with:
//!   cargo bench -p clawdius-core --bench memory_bench -- --verbose
//!
//! Key metrics:
//!   - `rss_per_session_kb`: Resident set size increase per session
//!   - `db_bytes_per_session`: SQLite file growth per session
//!   - `db_bytes_per_message`: SQLite file growth per message

use std::path::Path;

use clawdius_core::session::{Message, Session, SessionStore};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Read current process RSS in KB from `/proc/self/status` (Linux only).
fn current_rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    if let Some(kb_str) = rest.strip_suffix(" kB") {
                        if let Ok(kb) = kb_str.trim().parse::<u64>() {
                            return kb;
                        }
                    }
                }
            }
        }
    }
    0
}

/// Get file size in bytes.
fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn bench_memory_per_session(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_per_session");

    // Benchmark: 100 sessions with 10 messages each.
    group.bench_function("100_sessions_10_msgs", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("test.db");
            let store = SessionStore::open(&db_path).unwrap();

            let base_rss = current_rss_kb();
            let base_db_size = file_size(&db_path);

            for i in 0..100u64 {
                let mut session = Session::new();
                session.title = Some(format!("Session {i}"));
                session.meta.provider = Some("anthropic".to_string());
                session.meta.model = Some("claude-3-5-sonnet".to_string());

                // System prompt.
                let sys_msg = Message::system("You are a helpful assistant.");
                session.add_message(sys_msg);
                store
                    .save_message(
                        &session.id,
                        &Message::system("You are a helpful assistant."),
                    )
                    .unwrap();

                // 10 user+assistant message pairs.
                for j in 0..10u64 {
                    let user_text = format!(
                        "This is user message {j} in session {i}. \
                         It contains enough text to simulate a realistic message. \
                         Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                         Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua."
                    );
                    let user_msg = Message::user(&user_text);
                    store.save_message(&session.id, &user_msg).unwrap();
                    session.add_message(user_msg);

                    let assistant_text = format!(
                        "This is assistant response {j} in session {i}. \
                         It contains enough text to simulate a realistic response. \
                         The quick brown fox jumps over the lazy dog. \
                         Pack my box with five dozen liquor jugs."
                    );
                    let assistant_msg = Message::assistant(&assistant_text);
                    store.save_message(&session.id, &assistant_msg).unwrap();
                    session.add_message(assistant_msg);
                }

                store.create_session(&session).unwrap();
            }

            let final_rss = current_rss_kb();
            let final_db_size = file_size(&db_path);

            let rss_delta_kb = final_rss.saturating_sub(base_rss);
            let db_delta = final_db_size.saturating_sub(base_db_size);

            black_box((
                rss_delta_kb,
                db_delta,
                rss_delta_kb / 100, // per-session RSS
                db_delta / 1000,    // per-message DB
            ));
        });
    });

    // Benchmark: 1000 sessions with 5 messages each.
    group.bench_function("1000_sessions_5_msgs", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("test.db");
            let store = SessionStore::open(&db_path).unwrap();

            let base_rss = current_rss_kb();
            let base_db_size = file_size(&db_path);

            for i in 0..1000u64 {
                let mut session = Session::new();
                session.title = Some(format!("Session {i}"));
                session.meta.provider = Some("ollama".to_string());
                session.meta.model = Some("llama3".to_string());

                let sys_msg = Message::system("You are a helpful assistant.");
                session.add_message(sys_msg);
                store
                    .save_message(
                        &session.id,
                        &Message::system("You are a helpful assistant."),
                    )
                    .unwrap();

                for j in 0..5u64 {
                    let user_text = format!(
                        "User message {j} in session {i}. \
                         Lorem ipsum dolor sit amet, consectetur adipiscing elit."
                    );
                    let user_msg = Message::user(&user_text);
                    store.save_message(&session.id, &user_msg).unwrap();
                    session.add_message(user_msg);

                    let assistant_text = format!(
                        "Response {j} in session {i}. \
                         The quick brown fox jumps over the lazy dog."
                    );
                    let assistant_msg = Message::assistant(&assistant_text);
                    store.save_message(&session.id, &assistant_msg).unwrap();
                    session.add_message(assistant_msg);
                }

                store.create_session(&session).unwrap();
            }

            let final_rss = current_rss_kb();
            let final_db_size = file_size(&db_path);

            let rss_delta_kb = final_rss.saturating_sub(base_rss);
            let db_delta = final_db_size.saturating_sub(base_db_size);

            black_box((
                rss_delta_kb,
                db_delta,
                rss_delta_kb / 1000, // per-session RSS
                db_delta / 5000,     // per-message DB
            ));
        });
    });

    // Benchmark: 10000 sessions with 3 messages each (simulates 10K users).
    group.bench_function("10000_sessions_3_msgs", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("test.db");
            let store = SessionStore::open(&db_path).unwrap();

            let base_rss = current_rss_kb();
            let base_db_size = file_size(&db_path);

            for i in 0..10_000u64 {
                let mut session = Session::new();
                session.title = Some(format!("Session {i}"));
                session.meta.provider = Some("ollama".to_string());
                session.meta.model = Some("llama3".to_string());

                let sys_msg = Message::system("You are a helpful assistant.");
                session.add_message(sys_msg);
                store
                    .save_message(
                        &session.id,
                        &Message::system("You are a helpful assistant."),
                    )
                    .unwrap();

                for j in 0..3u64 {
                    let user_text = format!("User message {j} in session {i}.");
                    let user_msg = Message::user(&user_text);
                    store.save_message(&session.id, &user_msg).unwrap();
                    session.add_message(user_msg);

                    let assistant_text = format!("Response {j} in session {i}.");
                    let assistant_msg = Message::assistant(&assistant_text);
                    store.save_message(&session.id, &assistant_msg).unwrap();
                    session.add_message(assistant_msg);
                }

                store.create_session(&session).unwrap();
            }

            let final_rss = current_rss_kb();
            let final_db_size = file_size(&db_path);

            let rss_delta_kb = final_rss.saturating_sub(base_rss);
            let db_delta = final_db_size.saturating_sub(base_db_size);

            black_box((
                rss_delta_kb,
                db_delta,
                rss_delta_kb / 10_000, // per-session RSS
                db_delta / 30_000,     // per-message DB
            ));
        });
    });

    group.finish();
}

fn bench_session_store_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_store_overhead");

    // Measure: just opening a SessionStore and its base memory.
    group.bench_function("open_empty_store", |b| {
        b.iter_with_setup(
            || {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("bench.db");
                path
            },
            |path| {
                let store = SessionStore::open(&path).unwrap();
                black_box(&store);
            },
        );
    });

    // Measure: creating 1000 sessions (no messages) to isolate session overhead.
    group.bench_function("create_1000_empty_sessions", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("bench.db");
            let store = SessionStore::open(&db_path).unwrap();

            for i in 0..1000u64 {
                let mut session = Session::new();
                session.title = Some(format!("Session {i}"));
                session.meta.provider = Some("anthropic".to_string());
                session.meta.model = Some("claude-3-5-sonnet".to_string());
                store.create_session(&session).unwrap();
            }

            let db_size = file_size(&db_path);
            black_box(db_size);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_per_session,
    bench_session_store_overhead,
);
criterion_main!(benches);
