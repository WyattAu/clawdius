use crate::components::message::{Message, MessageRole};

#[test]
fn test_message_role_as_str() {
    assert_eq!(MessageRole::User.as_str(), "user");
    assert_eq!(MessageRole::Assistant.as_str(), "assistant");
    assert_eq!(MessageRole::System.as_str(), "system");
}

#[test]
fn test_message_serialization() {
    let msg = Message {
        id: "test-id".to_string(),
        role: MessageRole::User,
        content: "Hello, world!".to_string(),
        timestamp: "12:00:00".to_string(),
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("test-id"));
    assert!(json.contains("Hello, world!"));
    assert!(json.contains("User"));

    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test-id");
    assert_eq!(deserialized.role, MessageRole::User);
    assert_eq!(deserialized.content, "Hello, world!");
}

#[test]
fn test_message_role_serialization() {
    let role = MessageRole::Assistant;
    let json = serde_json::to_string(&role).unwrap();
    assert!(json.contains("Assistant"));

    let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, MessageRole::Assistant);
}

#[test]
fn test_message_equality() {
    let msg1 = Message {
        id: "id-1".to_string(),
        role: MessageRole::User,
        content: "test".to_string(),
        timestamp: "10:00:00".to_string(),
    };

    let msg2 = Message {
        id: "id-1".to_string(),
        role: MessageRole::User,
        content: "test".to_string(),
        timestamp: "10:00:00".to_string(),
    };

    let msg3 = Message {
        id: "id-2".to_string(),
        role: MessageRole::User,
        content: "test".to_string(),
        timestamp: "10:00:00".to_string(),
    };

    assert_eq!(msg1, msg2);
    assert_ne!(msg1, msg3);
}
