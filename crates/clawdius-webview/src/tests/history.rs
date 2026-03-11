use crate::components::history::{DateFilter, SessionData, SessionInfo, SessionMessage};

#[test]
fn test_session_info_serialization() {
    let session = SessionInfo {
        id: "session-123".to_string(),
        title: Some("Test Session".to_string()),
        provider: Some("anthropic".to_string()),
        model: Some("claude-3".to_string()),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-02T00:00:00Z".to_string(),
        message_count: 5,
        preview: Some("Hello...".to_string()),
    };

    let json = serde_json::to_string(&session).unwrap();
    assert!(json.contains("session-123"));
    assert!(json.contains("Test Session"));
    assert!(json.contains("anthropic"));

    let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "session-123");
    assert_eq!(deserialized.title, Some("Test Session".to_string()));
    assert_eq!(deserialized.message_count, 5);
}

#[test]
fn test_session_data_serialization() {
    let session_data = SessionData {
        session: SessionInfo {
            id: "session-456".to_string(),
            title: Some("Chat Session".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            message_count: 2,
            preview: None,
        },
        messages: vec![
            SessionMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                content: "Hello".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionMessage {
                id: "msg-2".to_string(),
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
                timestamp: "2024-01-01T00:00:01Z".to_string(),
            },
        ],
    };

    let json = serde_json::to_string(&session_data).unwrap();
    assert!(json.contains("session-456"));
    assert!(json.contains("msg-1"));
    assert!(json.contains("Hello"));

    let deserialized: SessionData = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.messages.len(), 2);
    assert_eq!(deserialized.messages[0].role, "user");
    assert_eq!(deserialized.messages[1].content, "Hi there!");
}

#[test]
fn test_date_filter_values() {
    assert_eq!(DateFilter::All, DateFilter::All);
    assert_ne!(DateFilter::Today, DateFilter::ThisWeek);
    assert_ne!(DateFilter::ThisWeek, DateFilter::ThisMonth);
}

#[test]
fn test_session_info_with_none_values() {
    let session = SessionInfo {
        id: "session-789".to_string(),
        title: None,
        provider: None,
        model: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
        message_count: 0,
        preview: None,
    };

    let json = serde_json::to_string(&session).unwrap();
    let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.title, None);
    assert_eq!(deserialized.provider, None);
    assert_eq!(deserialized.model, None);
    assert_eq!(deserialized.message_count, 0);
}
