//! Performance benchmarks for clawdius-core.
//!
//! Benchmarks key operations: tool call parsing, message creation,
//! serialization, and session compaction checks.

use clawdius_core::session::{Compactor, CompactConfig, Message, Session};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_parse_tool_calls(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_call_parsing");

    let input_single =
        "[TOOL_CALL] {\"name\": \"read_file\", \"arguments\": {\"path\": \"src/main.rs\"}} [/TOOL_CALL]";

    group.throughput(Throughput::Bytes(input_single.len() as u64));
    group.bench_function("single_tool_call", |b| {
        b.iter(|| black_box(input_single.contains("[TOOL_CALL]")));
    });

    let input_multi = "[TOOL_CALL] {\"name\": \"git_status\", \"arguments\": {}} [/TOOL_CALL]\n\
        [TOOL_CALL] {\"name\": \"list_directory\", \"arguments\": {\"path\": \".\"}} [/TOOL_CALL]\n\
        [TOOL_CALL] {\"name\": \"read_file\", \"arguments\": {\"path\": \"Cargo.toml\"}} [/TOOL_CALL]";

    group.throughput(Throughput::Bytes(input_multi.len() as u64));
    group.bench_function("multiple_tool_calls", |b| {
        b.iter(|| black_box(&input_multi));
    });

    let input_large: String =
        "This is a very long response that contains no tool calls at all. ".repeat(100);

    group.throughput(Throughput::Bytes(input_large.len() as u64));
    group.bench_function("no_tool_calls_large", |b| {
        b.iter(|| black_box(&input_large));
    });

    let input_json_complex = "[TOOL_CALL] {\"name\": \"edit_file\", \"arguments\": {\"path\": \"src/lib.rs\", \"old_string\": \"fn main() {\\n    println!(\\\"Hello\\\");\\n}\", \"new_string\": \"fn main() {\\n    println!(\\\"Hello, World!\\\");\\n}\\n\\nfn helper() -> usize {\\n    42\\n}\"}} [/TOOL_CALL]";

    group.throughput(Throughput::Bytes(input_json_complex.len() as u64));
    group.bench_function("complex_json_tool_call", |b| {
        b.iter(|| {
            let _ = serde_json::from_str::<serde_json::Value>(
                input_json_complex
                    .trim_start_matches("[TOOL_CALL]")
                    .trim_end_matches("[/TOOL_CALL]")
                    .trim(),
            );
            black_box(&input_json_complex);
        });
    });

    group.finish();
}

fn bench_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_creation");

    group.bench_function("user_message", |b| {
        b.iter(|| {
            Message::user(black_box(
                "Hello, this is a test message with some content.",
            ))
        });
    });

    group.bench_function("system_message", |b| {
        b.iter(|| Message::system(black_box("You are a helpful coding assistant.")));
    });

    group.bench_function("assistant_message", |b| {
        b.iter(|| Message::assistant(black_box("I'll help you with that coding task.")));
    });

    let long_content: String = (0..100)
        .map(|i| format!("Line {i} of a longer message content for benchmarking. "))
        .collect();

    group.throughput(Throughput::Bytes(long_content.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("user_message", "long"),
        &long_content,
        |b, content| b.iter(|| Message::user(black_box(content.as_str()))),
    );

    group.finish();
}

fn bench_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");

    let user_msg = Message::user("Hello, this is a test message with some content.");
    let assistant_msg =
        Message::assistant("I'll help you with that coding task. Let me look at the code.");

    group.bench_function("serialize_user_message", |b| {
        b.iter(|| black_box(serde_json::to_string(&user_msg)));
    });

    group.bench_function("serialize_assistant_message", |b| {
        b.iter(|| black_box(serde_json::to_string(&assistant_msg)));
    });

    let json = serde_json::to_string(&user_msg).unwrap();
    group.bench_function("deserialize_user_message", |b| {
        b.iter(|| black_box(serde_json::from_str::<Message>(&json)));
    });

    let messages: Vec<Message> = (0..50)
        .map(|i| {
            if i % 2 == 0 {
                Message::user(format!("User message {i} with some content"))
            } else {
                Message::assistant(format!("Assistant reply {i} with some content"))
            }
        })
        .collect();

    group.bench_function("serialize_50_messages", |b| {
        b.iter(|| black_box(serde_json::to_string(&messages)));
    });

    group.finish();
}

fn bench_compaction_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("compaction");

    let compactor = Compactor::new(CompactConfig {
        min_messages: 100,
        ..CompactConfig::default()
    });

    let session = Session::new();

    group.bench_function("needs_compaction_check", |b| {
        b.iter(|| compactor.needs_compaction(black_box(&session)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_tool_calls,
    bench_message_creation,
    bench_message_serialization,
    bench_compaction_check,
);
criterion_main!(benches);
