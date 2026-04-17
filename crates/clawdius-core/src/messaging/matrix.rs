//! Matrix client for sending messages to Matrix rooms.
//!
//! Thin wrapper over the Matrix Client-Server API using `reqwest`. Supports
//! sending plain text and Markdown-formatted messages via the `m.room.message`
//! event type.
//!
//! No external Matrix crates are used — this keeps the dependency tree lean
//! and avoids version conflicts.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default Matrix homeserver base URL.
const MATRIX_DEFAULT_BASE_URL: &str = "https://matrix.org";

// ---------------------------------------------------------------------------
// Matrix API types
// ---------------------------------------------------------------------------

/// Matrix-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixConfig {
    /// Homeserver base URL (e.g. `https://matrix.org`).
    pub base_url: String,
    /// Access token (user or appservice).
    pub access_token: String,
    /// Room ID to send messages to (e.g. `!roomid:matrix.org`).
    pub room_id: String,
}

/// Payload for a Matrix `m.room.message` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixMessage {
    /// Message type (e.g. `"m.text"`).
    pub msgtype: String,
    /// Plain-text body.
    pub body: String,
    /// HTML-formatted body (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted_body: Option<String>,
    /// Format hint (e.g. `"org.matrix.custom.html"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

// ---------------------------------------------------------------------------
// MatrixClient
// ---------------------------------------------------------------------------

/// Matrix Client-Server API client (send-only).
///
/// Sends messages to Matrix rooms via the `PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}`
/// endpoint. Receiving messages would require a full application service or
/// webhook bridge, which is outside the scope of this module.
#[derive(Debug, Clone)]
pub struct MatrixClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl MatrixClient {
    /// Create a new Matrix client.
    #[must_use]
    pub fn new(base_url: &str, access_token: &str) -> Self {
        let base_url = base_url.trim_end_matches('/');
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            base_url: base_url.to_string(),
            access_token: access_token.to_string(),
        }
    }

    /// Create a client using the default Matrix homeserver.
    #[must_use]
    pub fn with_default_homeserver(access_token: &str) -> Self {
        Self::new(MATRIX_DEFAULT_BASE_URL, access_token)
    }

    /// Send a plain text message to the specified room.
    pub async fn send_message(&self, room_id: &str, text: &str) -> crate::Result<()> {
        let url = self.send_url(room_id);
        let body = MatrixMessage {
            msgtype: "m.text".to_string(),
            body: text.to_string(),
            formatted_body: None,
            format: None,
        };

        self.put_json(&url, &body).await
    }

    /// Send a Markdown-formatted message to the specified room.
    ///
    /// The `body` field contains the raw Markdown as fallback plain text, and
    /// `formatted_body` contains the same text marked as HTML so that Matrix
    /// clients can render it richly.
    pub async fn send_markdown(&self, room_id: &str, markdown: &str) -> crate::Result<()> {
        let url = self.send_url(room_id);
        let body = MatrixMessage {
            msgtype: "m.text".to_string(),
            body: markdown.to_string(),
            formatted_body: Some(markdown.to_string()),
            format: Some("org.matrix.custom.html".to_string()),
        };

        self.put_json(&url, &body).await
    }

    /// Return the configured base URL (for diagnostics).
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // -- internal helpers --------------------------------------------------

    fn txn_id() -> String {
        format!("clawdius_{}", uuid::Uuid::new_v4().simple())
    }

    fn send_url(&self, room_id: &str) -> String {
        format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.base_url,
            room_id,
            Self::txn_id()
        )
    }

    async fn put_json(&self, url: &str, body: &MatrixMessage) -> crate::Result<()> {
        let resp = self
            .http
            .put(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(body)
            .send()
            .await
            .map_err(|e| crate::Error::Llm(format!("Matrix request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(crate::Error::Llm(format!(
                "Matrix API returned {status}: {body_text}"
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_client_new() {
        let client = MatrixClient::new("https://matrix.org", "syt_token");
        assert_eq!(client.base_url(), "https://matrix.org");
    }

    #[test]
    fn test_matrix_client_new_trims_trailing_slash() {
        let client = MatrixClient::new("https://matrix.org/", "tok");
        assert_eq!(client.base_url(), "https://matrix.org");
    }

    #[test]
    fn test_matrix_client_with_default_homeserver() {
        let client = MatrixClient::with_default_homeserver("tok");
        assert_eq!(client.base_url(), "https://matrix.org");
    }

    #[test]
    fn test_matrix_client_clone() {
        let client = MatrixClient::new("https://matrix.org", "tok");
        let _client2 = client.clone();
    }

    #[test]
    fn test_matrix_message_serialize_plain() {
        let msg = MatrixMessage {
            msgtype: "m.text".to_string(),
            body: "hello".to_string(),
            formatted_body: None,
            format: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""msgtype":"m.text""#));
        assert!(json.contains(r#""body":"hello""#));
        assert!(!json.contains("formatted_body"));
        assert!(!json.contains("format"));
    }

    #[test]
    fn test_matrix_message_serialize_markdown() {
        let msg = MatrixMessage {
            msgtype: "m.text".to_string(),
            body: "**bold**".to_string(),
            formatted_body: Some("<b>bold</b>".to_string()),
            format: Some("org.matrix.custom.html".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""formatted_body":"<b>bold</b>""#));
        assert!(json.contains(r#""format":"org.matrix.custom.html""#));
    }

    #[test]
    fn test_matrix_message_deserialize() {
        let json = r#"{"msgtype":"m.text","body":"hi"}"#;
        let msg: MatrixMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msgtype, "m.text");
        assert_eq!(msg.body, "hi");
        assert!(msg.formatted_body.is_none());
        assert!(msg.format.is_none());
    }

    #[test]
    fn test_send_url_contains_room_and_txn() {
        let client = MatrixClient::new("https://matrix.org", "tok");
        let url = client.send_url("!abc:matrix.org");
        assert!(url.starts_with("https://matrix.org/_matrix/client/v3/rooms/"));
        assert!(url.contains("!abc:matrix.org"));
        assert!(url.contains("/send/m.room.message/clawdius_"));
    }

    #[test]
    fn test_matrix_config_roundtrip() {
        let cfg = MatrixConfig {
            base_url: "https://example.com".to_string(),
            access_token: "tok123".to_string(),
            room_id: "!room:example.com".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: MatrixConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg2.base_url, "https://example.com");
        assert_eq!(cfg2.access_token, "tok123");
        assert_eq!(cfg2.room_id, "!room:example.com");
    }

    #[tokio::test]
    async fn test_send_message_mock() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let client = MatrixClient::new(&server.url(""), "tok");

        let room_id = "!test:matrix.org";
        server.mock(|when, then| {
            when.method("PUT")
                .path_matches(format!(
                    r#"/_matrix/client/v3/rooms/{}/send/m\.room\.message/clawdius_"#,
                    regex::escape(room_id)
                ))
                .header("Authorization", "Bearer tok");
            then.status(200)
                .json_body(serde_json::json!({"event_id": "$abc"}));
        });

        let result = client.send_message(room_id, "hello").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_markdown_has_formatted_body() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let client = MatrixClient::new(&server.url(""), "tok");

        let room_id = "!test:matrix.org";
        server.mock(|when, then| {
            when.method("PUT")
                .path_matches(format!(
                    r#"/_matrix/client/v3/rooms/{}/send/m\.room\.message/clawdius_"#,
                    regex::escape(room_id)
                ))
                .header("Authorization", "Bearer tok");
            then.status(200)
                .json_body(serde_json::json!({"event_id": "$abc"}));
        });

        let result = client.send_markdown(room_id, "**bold**").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_message_error_response() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let client = MatrixClient::new(&server.url(""), "bad_tok");

        let room_id = "!test:matrix.org";
        server.mock(|when, then| {
            when.method("PUT")
                .path_matches(format!(
                    r#"/_matrix/client/v3/rooms/{}/send/m\.room\.message/clawdius_"#,
                    regex::escape(room_id)
                ));
            then.status(401).json_body(serde_json::json!({"errcode":"M_UNKNOWN_TOKEN","error":"Unrecognized access token"}));
        });

        let result = client.send_message(room_id, "hello").await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("401"));
    }
}
