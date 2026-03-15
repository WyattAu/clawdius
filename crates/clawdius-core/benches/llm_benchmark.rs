use clawdius_core::llm::{ChatMessage, ChatRole};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_chat_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_message_creation");

    group.bench_function("chat_message_create_simple", |b| {
        b.iter(|| ChatMessage {
            role: ChatRole::User,
            content: black_box("Hello, world!").to_string(),
        });
    });

    group.bench_function("chat_message_create_system", |b| {
        b.iter(|| ChatMessage {
            role: ChatRole::System,
            content: black_box("You are a helpful assistant.").to_string(),
        });
    });

    let long_message: String = (0..100)
        .map(|i| format!("Line {i} of the message. "))
        .collect();
    group.throughput(Throughput::Bytes(long_message.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("chat_message_create", "long"),
        &long_message,
        |b, msg| {
            b.iter(|| ChatMessage {
                role: ChatRole::User,
                content: black_box(msg.clone()),
            });
        },
    );

    group.finish();
}

fn bench_chat_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_message_serialization");

    let msg = ChatMessage {
        role: ChatRole::User,
        content: "Hello, world!".to_string(),
    };

    group.bench_function("message_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&msg)));
    });

    let json = serde_json::to_string(&msg).unwrap();
    group.bench_function("message_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<ChatMessage>(&json)));
    });

    group.finish();
}

fn bench_message_collections(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_message_collections");

    group.bench_function("create_message_vec_10", |b| {
        b.iter(|| {
            let mut messages = Vec::with_capacity(10);
            for i in 0..10 {
                messages.push(ChatMessage {
                    role: if i % 2 == 0 {
                        ChatRole::User
                    } else {
                        ChatRole::Assistant
                    },
                    content: black_box(format!("Message {i}")),
                });
            }
            black_box(messages)
        });
    });

    group.bench_function("create_message_vec_100", |b| {
        b.iter(|| {
            let mut messages = Vec::with_capacity(100);
            for i in 0..100 {
                messages.push(ChatMessage {
                    role: if i % 2 == 0 {
                        ChatRole::User
                    } else {
                        ChatRole::Assistant
                    },
                    content: black_box(format!("Message {i}")),
                });
            }
            black_box(messages)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_chat_message_creation,
    bench_chat_message_serialization,
    bench_message_collections,
);
criterion_main!(benches);
