use clawdius_core::session::{Message, Session, SessionStore};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::NamedTempFile;

fn bench_session_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_create");

    group.bench_function("session_new", |b| {
        b.iter(|| {
            let mut session = Session::new();
            session.title = Some("Benchmark Session".to_string());
            session.meta.provider = Some("anthropic".to_string());
            session.meta.model = Some("claude-3-5-sonnet".to_string());
            black_box(session)
        });
    });

    group.bench_function("session_store_create", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        b.iter(|| {
            let mut session = Session::new();
            session.title = Some("Test Session".to_string());
            black_box(store.create_session(&session))
        });
    });

    group.finish();
}

fn bench_session_persistence(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_persistence");

    group.bench_function("session_load", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let mut session = Session::new();
        session.title = Some("Test Session".to_string());
        store.create_session(&session).unwrap();

        b.iter(|| black_box(store.load_session(&session.id)));
    });

    group.bench_function("session_load_full_empty", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        b.iter(|| black_box(store.load_session_full(&session.id)));
    });

    group.finish();
}

fn bench_message_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_message_operations");

    group.bench_function("message_create_user", |b| {
        b.iter(|| Message::user(black_box("Hello, world!")));
    });

    group.bench_function("message_create_assistant", |b| {
        b.iter(|| Message::assistant(black_box("Hello! How can I help you?")));
    });

    group.bench_function("message_create_system", |b| {
        b.iter(|| Message::system(black_box("You are a helpful assistant.")));
    });

    let long_message: String = (0..100)
        .map(|i| format!("Line {i} of the message. "))
        .collect();
    group.throughput(Throughput::Bytes(long_message.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("message_create", "long"),
        &long_message,
        |b, msg| b.iter(|| Message::user(black_box(msg.as_str()))),
    );

    group.finish();
}

fn bench_session_with_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_with_messages");

    group.bench_function("session_save_message", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        b.iter(|| {
            let msg = Message::user("Hello, world!");
            black_box(store.save_message(&session.id, &msg))
        });
    });

    group.bench_function("session_load_10_messages", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        for i in 0..10 {
            let msg = Message::user(format!("Message {i}"));
            store.save_message(&session.id, &msg).unwrap();
        }

        b.iter(|| black_box(store.load_session_full(&session.id)));
    });

    group.bench_function("session_load_100_messages", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        for i in 0..100 {
            let msg = Message::user(format!("Message {i}"));
            store.save_message(&session.id, &msg).unwrap();
        }

        b.iter(|| black_box(store.load_session_full(&session.id)));
    });

    group.bench_function("session_load_1000_messages", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        let session = Session::new();
        store.create_session(&session).unwrap();

        for i in 0..1000 {
            let msg = Message::user(format!("Message {i}"));
            store.save_message(&session.id, &msg).unwrap();
        }

        b.iter(|| black_box(store.load_session_full(&session.id)));
    });

    group.finish();
}

fn bench_session_list_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_list_operations");

    group.bench_function("list_sessions_10", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        for i in 0..10 {
            let mut session = Session::new();
            session.title = Some(format!("Session {i}"));
            store.create_session(&session).unwrap();
        }

        b.iter(|| black_box(store.list_sessions()));
    });

    group.bench_function("list_sessions_100", |b| {
        let temp = NamedTempFile::new().unwrap();
        let store = SessionStore::open(temp.path()).unwrap();

        for i in 0..100 {
            let mut session = Session::new();
            session.title = Some(format!("Session {i}"));
            store.create_session(&session).unwrap();
        }

        b.iter(|| black_box(store.list_sessions()));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_session_create,
    bench_session_persistence,
    bench_message_operations,
    bench_session_with_messages,
    bench_session_list_operations,
);
criterion_main!(benches);
