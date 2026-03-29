//! Real HTTP integration tests for all 7 messaging channel adapters.
//!
//! Each test spins up an httpmock server, configures it to respond like the
//! real platform API, constructs the channel adapter pointing at the mock,
//! and verifies send_message, edit_message, is_connected, and error handling.

use clawdius_core::messaging::channels::MessagingChannel;
use clawdius_core::messaging::channels::{
    DiscordChannel, MatrixChannel, RocketChatChannel, SignalChannel, SlackChannel, TelegramChannel,
    WhatsAppChannel,
};
use httpmock::MockServer;

// ============================================================
// Telegram Tests
// ============================================================

#[tokio::test]
async fn telegram_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path_contains("/sendMessage");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"result":{"message_id":42}}));
    });

    let channel = TelegramChannel::with_base_url(&base, "TEST-TOKEN");
    let msg_id = channel.send_message("99", "hello").await.unwrap();
    assert_eq!(msg_id, "42");
}

#[tokio::test]
async fn telegram_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path_contains("/editMessageText");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"result":{"message_id":42}}));
    });

    let channel = TelegramChannel::with_base_url(&base, "TEST-TOKEN");
    let msg_id = channel.edit_message("99", "42", "edited").await.unwrap();
    assert_eq!(msg_id, "42");
}

#[tokio::test]
async fn telegram_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET").path_contains("/getMe");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"result":{"id":1,"first_name":"Bot"}}));
    });

    let channel = TelegramChannel::with_base_url(&base, "TEST-TOKEN");
    assert!(channel.is_connected().await);
}

#[tokio::test]
async fn telegram_send_error() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path_contains("/sendMessage");
        then.status(403).json_body(
            serde_json::json!({"ok":false,"description":"Forbidden: bot was blocked by the user"}),
        );
    });

    let channel = TelegramChannel::with_base_url(&base, "TEST-TOKEN");
    let result = channel.send_message("99", "hello").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn telegram_message_too_long() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    let channel = TelegramChannel::with_base_url(&base, "TEST-TOKEN");
    let long_text = "x".repeat(4097);
    let result = channel.send_message("99", &long_text).await;
    assert!(result.is_err());
}

// ============================================================
// Discord Tests
// ============================================================

#[tokio::test]
async fn discord_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/channels/123/messages")
            .header("authorization", "Bot test-token");
        then.status(200)
            .json_body(serde_json::json!({"id":"msg123","channel_id":"123"}));
    });

    let channel = DiscordChannel::with_base_url(&base, "test-token");
    let msg_id = channel.send_message("123", "hello").await.unwrap();
    assert_eq!(msg_id, "msg123");
}

#[tokio::test]
async fn discord_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("PATCH")
            .path("/channels/123/messages/msg123")
            .header("authorization", "Bot test-token");
        then.status(200)
            .json_body(serde_json::json!({"id":"msg123","channel_id":"123"}));
    });

    let channel = DiscordChannel::with_base_url(&base, "test-token");
    let msg_id = channel
        .edit_message("123", "msg123", "edited")
        .await
        .unwrap();
    assert_eq!(msg_id, "msg123");
}

#[tokio::test]
async fn discord_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET")
            .path("/users/@me")
            .header("authorization", "Bot test-token");
        then.status(200)
            .json_body(serde_json::json!({"id":"123","username":"bot"}));
    });

    let channel = DiscordChannel::with_base_url(&base, "test-token");
    assert!(channel.is_connected().await);
}

#[tokio::test]
async fn discord_send_error() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path("/channels/123/messages");
        then.status(403)
            .json_body(serde_json::json!({"message":"Missing Permissions"}));
    });

    let channel = DiscordChannel::with_base_url(&base, "test-token");
    let result = channel.send_message("123", "hello").await;
    assert!(result.is_err());
}

// ============================================================
// Slack Tests
// ============================================================

#[tokio::test]
async fn slack_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/chat.postMessage")
            .header("authorization", "Bearer xoxb-test");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"ts":"1234567890.123456"}));
    });

    let channel = SlackChannel::with_base_url(&base, "xoxb-test");
    let msg_id = channel.send_message("C123", "hello").await.unwrap();
    assert_eq!(msg_id, "1234567890.123456");
}

#[tokio::test]
async fn slack_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/chat.update")
            .header("authorization", "Bearer xoxb-test");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"ts":"1234567890.123456"}));
    });

    let channel = SlackChannel::with_base_url(&base, "xoxb-test");
    let msg_id = channel
        .edit_message("C123", "1234567890.123456", "edited")
        .await
        .unwrap();
    assert_eq!(msg_id, "1234567890.123456");
}

#[tokio::test]
async fn slack_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/auth.test")
            .header("authorization", "Bearer xoxb-test");
        then.status(200)
            .json_body(serde_json::json!({"ok":true,"user":"bot"}));
    });

    let channel = SlackChannel::with_base_url(&base, "xoxb-test");
    assert!(channel.is_connected().await);
}

#[tokio::test]
async fn slack_send_error() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path("/chat.postMessage");
        then.status(200)
            .json_body(serde_json::json!({"ok":false,"error":"channel_not_found"}));
    });

    let channel = SlackChannel::with_base_url(&base, "xoxb-test");
    let result = channel.send_message("C123", "hello").await;
    assert!(result.is_err());
}

// ============================================================
// Matrix Tests
// ============================================================

#[tokio::test]
async fn matrix_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("PUT")
            .path_contains("/_matrix/client/v3/rooms/!room:example.com/send/m.room.message");
        then.status(200)
            .json_body(serde_json::json!({"event_id":"$event_123"}));
    });

    let channel = MatrixChannel::new(&base, "TEST-TOKEN");
    let msg_id = channel
        .send_message("!room:example.com", "hello")
        .await
        .unwrap();
    assert_eq!(msg_id, "$event_123");
}

#[tokio::test]
async fn matrix_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("PUT")
            .path_contains("/_matrix/client/v3/rooms/!room:example.com/send/m.room.message")
            .body_contains("m.replace");
        then.status(200)
            .json_body(serde_json::json!({"event_id":"$event_456"}));
    });

    let channel = MatrixChannel::new(&base, "TEST-TOKEN");
    let msg_id = channel
        .edit_message("!room:example.com", "$event_123", "edited text")
        .await
        .unwrap();
    assert_eq!(msg_id, "$event_456");
}

#[tokio::test]
async fn matrix_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET")
            .path_contains("/_matrix/client/v3/account/whoami");
        then.status(200)
            .json_body(serde_json::json!({"user_id":"@bot:matrix.org"}));
    });

    let channel = MatrixChannel::new(&base, "TEST-TOKEN");
    assert!(channel.is_connected().await);
}

// ============================================================
// Signal Tests
// ============================================================

#[tokio::test]
async fn signal_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path("/v1/send/+1234567890");
        then.status(200)
            .json_body(serde_json::json!({"timestamp":1700000000}));
    });

    let channel = SignalChannel::new(&base, "+1234567890");
    let msg_id = channel.send_message("+999", "hello").await.unwrap();
    assert_eq!(msg_id, "1700000000");
}

#[tokio::test]
async fn signal_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET").path("/v1/about");
        then.status(200).body("OK");
    });

    let channel = SignalChannel::new(&base, "+1234567890");
    assert!(channel.is_connected().await);
}

#[tokio::test]
async fn signal_send_error() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST").path("/v1/send/+1234567890");
        then.status(500);
    });

    let channel = SignalChannel::new(&base, "+1234567890");
    let result = channel.send_message("+999", "hello").await;
    assert!(result.is_err());
}

// ============================================================
// WhatsApp Tests
// ============================================================

#[tokio::test]
async fn whatsapp_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/12345/messages")
            .header("authorization", "Bearer test-token");
        then.status(200)
            .json_body(serde_json::json!({"messages":[{"id":"wamid_HBgNt"}]}));
    });

    let channel = WhatsAppChannel::with_base_url(&base, "12345", "test-token");
    let msg_id = channel.send_message("12345", "hello").await.unwrap();
    assert_eq!(msg_id, "wamid_HBgNt");
}

#[tokio::test]
async fn whatsapp_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/wamid_HBgNt")
            .header("authorization", "Bearer test-token");
        then.status(200).body("{}");
    });

    let channel = WhatsAppChannel::with_base_url(&base, "12345", "test-token");
    let result = channel.edit_message("12345", "wamid_HBgNt", "edited").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn whatsapp_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET")
            .path("/12345")
            .header("authorization", "Bearer test-token");
        then.status(200)
            .json_body(serde_json::json!({"name":"Phone Number"}));
    });

    let channel = WhatsAppChannel::with_base_url(&base, "12345", "test-token");
    assert!(channel.is_connected().await);
}

// ============================================================
// RocketChat Tests
// ============================================================

#[tokio::test]
async fn rocketchat_send_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/api/v1/chat.postMessage")
            .header("x-user-id", "user123")
            .header("x-auth-token", "token");
        then.status(200).json_body(
            serde_json::json!({"success":true,"result":{"ts":"1700000000","_id":"msg123"}}),
        );
    });

    let channel = RocketChatChannel::new(&base, "user123", "token");
    let msg_id = channel.send_message("general", "hello").await.unwrap();
    assert_eq!(msg_id, "msg123");
}

#[tokio::test]
async fn rocketchat_edit_message() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/api/v1/chat.update")
            .header("x-user-id", "user123")
            .header("x-auth-token", "token");
        then.status(200).json_body(
            serde_json::json!({"success":true,"result":{"ts":"1700000001","_id":"msg123"}}),
        );
    });

    let channel = RocketChatChannel::new(&base, "user123", "token");
    let msg_id = channel
        .edit_message("GENERAL", "msg123", "edited")
        .await
        .unwrap();
    assert_eq!(msg_id, "msg123");
}

#[tokio::test]
async fn rocketchat_is_connected() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("GET")
            .path("/api/v1/me")
            .header("x-user-id", "user123")
            .header("x-auth-token", "token");
        then.status(200)
            .json_body(serde_json::json!({"success":true,"result":{"username":"bot"}}));
    });

    let channel = RocketChatChannel::new(&base, "user123", "token");
    assert!(channel.is_connected().await);
}

#[tokio::test]
async fn rocketchat_send_error() {
    let server = MockServer::start();
    let base = format!("http://127.0.0.1:{}", server.port());

    server.mock(|when, then| {
        when.method("POST")
            .path("/api/v1/chat.postMessage")
            .header("x-user-id", "user123")
            .header("x-auth-token", "token");
        then.status(401);
    });

    let channel = RocketChatChannel::new(&base, "user123", "token");
    let result = channel.send_message("general", "hello").await;
    assert!(result.is_err());
}
