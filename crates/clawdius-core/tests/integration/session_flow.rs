use clawdius_core::session::{Message, Session, SessionStore};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_session_create_chat_save_load() {
    let temp = NamedTempFile::new().expect("Failed to create temp file");
    let store = SessionStore::open(temp.path()).expect("Failed to open store");

    let mut session = Session::new();
    session.title = Some("Integration Test Session".to_string());
    session.meta.provider = Some("anthropic".to_string());
    session.meta.model = Some("claude-3-5-sonnet".to_string());
    session.meta.tags = vec!["test".to_string(), "integration".to_string()];

    let msg1 = Message::user("Hello, this is a test message!");
    session.add_message(msg1);

    let msg2 = Message::assistant("I understand. How can I help you?");
    session.add_message(msg2);

    let msg3 = Message::user("Please help me write some code.");
    session.add_message(msg3);

    store
        .create_session(&session)
        .expect("Failed to create session");

    for msg in &session.messages {
        store
            .save_message(&session.id, msg)
            .expect("Failed to save message");
    }

    let loaded = store
        .load_session_full(&session.id)
        .expect("Failed to load session")
        .expect("Session should exist");

    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.title, Some("Integration Test Session".to_string()));
    assert_eq!(loaded.meta.provider, Some("anthropic".to_string()));
    assert_eq!(loaded.meta.model, Some("claude-3-5-sonnet".to_string()));
    assert_eq!(loaded.messages.len(), 3);

    assert_eq!(
        loaded.messages[0].as_text(),
        Some("Hello, this is a test message!")
    );
    assert_eq!(
        loaded.messages[1].as_text(),
        Some("I understand. How can I help you?")
    );
    assert_eq!(
        loaded.messages[2].as_text(),
        Some("Please help me write some code.")
    );
}

#[tokio::test]
async fn test_session_multiple_sessions_list() {
    let temp = NamedTempFile::new().expect("Failed to create temp file");
    let store = SessionStore::open(temp.path()).expect("Failed to open store");

    let mut session1 = Session::new();
    session1.title = Some("Session 1".to_string());
    session1.add_message(Message::user("Message in session 1"));

    let mut session2 = Session::new();
    session2.title = Some("Session 2".to_string());
    session2.add_message(Message::user("Message in session 2"));

    let mut session3 = Session::new();
    session3.title = Some("Session 3".to_string());
    session3.add_message(Message::user("Message in session 3"));

    store
        .create_session(&session1)
        .expect("Failed to create session 1");
    store
        .create_session(&session2)
        .expect("Failed to create session 2");
    store
        .create_session(&session3)
        .expect("Failed to create session 3");

    let sessions = store.list_sessions().expect("Failed to list sessions");
    assert_eq!(sessions.len(), 3);

    let titles: Vec<_> = sessions.iter().filter_map(|s| s.title.as_ref()).collect();
    assert!(titles.contains(&&"Session 1".to_string()));
    assert!(titles.contains(&&"Session 2".to_string()));
    assert!(titles.contains(&&"Session 3".to_string()));
}

#[tokio::test]
async fn test_session_delete() {
    let temp = NamedTempFile::new().expect("Failed to create temp file");
    let store = SessionStore::open(temp.path()).expect("Failed to open store");

    let session = Session::new();
    store
        .create_session(&session)
        .expect("Failed to create session");

    let loaded = store
        .load_session(&session.id)
        .expect("Failed to load session");
    assert!(loaded.is_some());

    store
        .delete_session(&session.id)
        .expect("Failed to delete session");

    let loaded = store
        .load_session(&session.id)
        .expect("Failed to load session");
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_session_token_tracking() {
    let temp = NamedTempFile::new().expect("Failed to create temp file");
    let store = SessionStore::open(temp.path()).expect("Failed to open store");

    let mut session = Session::new();
    session.token_usage.input = 1000;
    session.token_usage.output = 500;
    session.token_usage.cached = 200;

    store
        .create_session(&session)
        .expect("Failed to create session");

    store
        .update_token_usage(&session.id, &session.token_usage)
        .expect("Failed to update token usage");

    let loaded = store
        .load_session(&session.id)
        .expect("Failed to load session")
        .expect("Session should exist");

    assert_eq!(loaded.token_usage.input, 1000);
    assert_eq!(loaded.token_usage.output, 500);
    assert_eq!(loaded.token_usage.cached, 200);
    assert_eq!(loaded.total_tokens(), 1500);
}

#[tokio::test]
async fn test_session_search_messages() {
    let temp = NamedTempFile::new().expect("Failed to create temp file");
    let store = SessionStore::open(temp.path()).expect("Failed to open store");

    let mut session = Session::new();
    session.title = Some("Search Test".to_string());

    let msg1 = Message::user("I love programming in Rust");
    let msg2 = Message::assistant("Rust is a great language for systems programming");
    let msg3 = Message::user("What about Python?");

    session.add_message(msg1.clone());
    session.add_message(msg2.clone());
    session.add_message(msg3.clone());

    store
        .create_session(&session)
        .expect("Failed to create session");
    store
        .save_message(&session.id, &msg1)
        .expect("Failed to save");
    store
        .save_message(&session.id, &msg2)
        .expect("Failed to save");
    store
        .save_message(&session.id, &msg3)
        .expect("Failed to save");

    let results = store
        .search_messages("Rust")
        .expect("Failed to search messages");
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_session_in_memory() {
    let store = SessionStore::in_memory().expect("Failed to create in-memory store");

    let mut session = Session::new();
    session.title = Some("In-Memory Test".to_string());
    let msg = Message::user("Test message");
    session.add_message(msg.clone());

    store
        .create_session(&session)
        .expect("Failed to create session");
    store
        .save_message(&session.id, &msg)
        .expect("Failed to save message");

    let loaded = store
        .load_session_full(&session.id)
        .expect("Failed to load session")
        .expect("Session should exist");

    assert_eq!(loaded.messages.len(), 1);
}
