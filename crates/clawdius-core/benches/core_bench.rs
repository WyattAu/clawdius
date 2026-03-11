use clawdius_core::{
    context::Mention,
    diff::FileDiff,
    rpc::types::{Id, Request, Response},
    session::{Message, Session, SessionStore},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn bench_session_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_store");

    group.bench_function("create_session", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        b.iter(|| {
            let mut session = Session::new();
            session.title = Some("Test Session".to_string());
            session.meta.provider = Some("anthropic".to_string());
            session.meta.model = Some("claude-3-5-sonnet".to_string());
            black_box(store.create_session(&session))
        });
    });

    group.bench_function("load_session", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let mut session = Session::new();
        session.title = Some("Test Session".to_string());
        store.create_session(&session).unwrap();

        b.iter(|| black_box(store.load_session(&session.id)));
    });

    group.bench_function("save_message", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        b.iter(|| {
            let msg = Message::user("Hello, world!");
            black_box(store.save_message(&session.id, &msg))
        });
    });

    group.bench_function("load_session_full_100_messages", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        for i in 0..100 {
            let msg = Message::user(&format!("Message {}", i));
            store.save_message(&session.id, &msg).unwrap();
        }

        b.iter(|| black_box(store.load_session_full(&session.id)));
    });

    group.finish();
}

fn bench_context_mentions(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_mentions");

    let inputs = vec![
        ("single_file", "Fix the bug in @file:src/main.rs"),
        ("multiple_mentions", "Compare @file:src/a.rs with @file:src/b.rs and check @url:https://example.com"),
        ("git_mentions", "Review @git:diff and @git:log:5"),
        ("complex", "Check @file:src/lib.rs, @folder:tests, @url:https://docs.rs, @git:diff, @search:\"function definition\""),
    ];

    for (name, text) in inputs {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("parse", name), text, |b, text| {
            b.iter(|| black_box(Mention::parse(text)));
        });
    }

    group.finish();
}

fn bench_diff_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_computation");

    let small_old = "line1\nline2\nline3\n";
    let small_new = "line1\nmodified\nline3\n";

    let medium_old: String = (0..100).map(|i| format!("line{}\n", i)).collect();
    let medium_new: String = (0..100)
        .map(|i| {
            if i == 50 {
                format!("modified{}\n", i)
            } else {
                format!("line{}\n", i)
            }
        })
        .collect();

    let large_old: String = (0..1000).map(|i| format!("line{}\n", i)).collect();
    let large_new: String = (0..1000)
        .map(|i| {
            if i % 10 == 0 {
                format!("modified{}\n", i)
            } else {
                format!("line{}\n", i)
            }
        })
        .collect();

    group.throughput(Throughput::Bytes(small_old.len() as u64));
    group.bench_with_input(BenchmarkId::new("compute", "small"), &small_old, |b, _| {
        b.iter(|| {
            black_box(FileDiff::compute(
                PathBuf::from("test.txt"),
                Some(&small_old),
                &small_new,
            ))
        });
    });

    group.throughput(Throughput::Bytes(medium_old.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("compute", "medium"),
        &medium_old,
        |b, _| {
            b.iter(|| {
                black_box(FileDiff::compute(
                    PathBuf::from("test.txt"),
                    Some(&medium_old),
                    &medium_new,
                ))
            });
        },
    );

    group.throughput(Throughput::Bytes(large_old.len() as u64));
    group.bench_with_input(BenchmarkId::new("compute", "large"), &large_old, |b, _| {
        b.iter(|| {
            black_box(FileDiff::compute(
                PathBuf::from("test.txt"),
                Some(&large_old),
                &large_new,
            ))
        });
    });

    group.finish();
}

fn bench_json_rpc_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_rpc_serialization");

    group.bench_function("request_serialize", |b| {
        let req = Request::new(1, "chat/send").with_params(serde_json::json!({
            "message": "Hello, world!",
            "provider": "anthropic",
            "model": "claude-3-5-sonnet"
        }));

        b.iter(|| black_box(serde_json::to_string(&req)));
    });

    group.bench_function("request_deserialize", |b| {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"chat/send","params":{"message":"Hello"}}"#;

        b.iter(|| black_box(serde_json::from_str::<Request>(json)));
    });

    group.bench_function("response_serialize", |b| {
        let res = Response::success(
            Id::Number(1),
            serde_json::json!({
                "status": "ok",
                "data": {"key": "value"}
            }),
        );

        b.iter(|| black_box(serde_json::to_string(&res)));
    });

    group.bench_function("response_deserialize", |b| {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}"#;

        b.iter(|| black_box(serde_json::from_str::<Response>(json)));
    });

    group.finish();
}

fn bench_token_counting(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_counting");

    let small_text = "Hello, world! This is a simple test message.";
    let medium_text: String = (0..50)
        .map(|i| format!("This is line {} of the test message. ", i))
        .collect();
    let large_text: String = (0..500)
        .map(|i| format!("This is line {} of the test message. ", i))
        .collect();

    group.throughput(Throughput::Bytes(small_text.len() as u64));
    group.bench_with_input(BenchmarkId::new("count", "small"), small_text, |b, text| {
        b.iter(|| {
            let tokens = tiktoken_rs::cl100k_base().unwrap();
            black_box(tokens.encode_with_special_tokens(text))
        });
    });

    group.throughput(Throughput::Bytes(medium_text.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("count", "medium"),
        &medium_text,
        |b, text| {
            b.iter(|| {
                let tokens = tiktoken_rs::cl100k_base().unwrap();
                black_box(tokens.encode_with_special_tokens(text))
            });
        },
    );

    group.throughput(Throughput::Bytes(large_text.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("count", "large"),
        &large_text,
        |b, text| {
            b.iter(|| {
                let tokens = tiktoken_rs::cl100k_base().unwrap();
                black_box(tokens.encode_with_special_tokens(text))
            });
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_session_store,
    bench_context_mentions,
    bench_diff_computation,
    bench_json_rpc_serialization,
    bench_token_counting,
);
criterion_main!(benches);
