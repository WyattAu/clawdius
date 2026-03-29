//! Matrix Channel Implementation
//!
//! Provides integration with Matrix API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct MatrixMessageEvent {
    msgtype: String,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
}

#[derive(Serialize)]
struct MatrixEditEvent<'a> {
    msgtype: &'a str,
    body: String,
    #[serde(rename = "m.new_content")]
    new_content: MatrixNewContent<'a>,
    #[serde(rename = "m.relates_to")]
    relates_to: MatrixRelatesTo<'a>,
}

#[derive(Serialize)]
struct MatrixNewContent<'a> {
    msgtype: &'a str,
    body: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    formatted_body: Option<&'a str>,
}

#[derive(Serialize)]
struct MatrixRelatesTo<'a> {
    rel_type: &'a str,
    event_id: &'a str,
}

#[derive(Deserialize)]
struct MatrixEventResponse {
    event_id: String,
}

pub struct MatrixChannel {
    homeserver: String,
    access_token: String,
    client: Client,
}

impl MatrixChannel {
    pub fn new(homeserver: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            homeserver: homeserver.into(),
            access_token: access_token.into(),
            client: Client::new(),
        }
    }

    fn api_url(&self, room_id: &str) -> String {
        let txn_id = Uuid::new_v4().to_string();
        format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message?access_token={}&txn_id={}",
            self.homeserver, room_id, self.access_token, txn_id
        )
    }
}

#[async_trait]
impl MessagingChannel for MatrixChannel {
    fn platform(&self) -> Platform {
        Platform::Matrix
    }

    async fn send_message(&self, room_id: &str, text: &str) -> Result<String> {
        let max_len = Platform::Matrix.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .put(self.api_url(room_id))
            .json(&MatrixMessageEvent {
                msgtype: "m.text".to_string(),
                body: text.to_string(),
                format: Some("org.matrix.custom.html".to_string()),
            })
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(MessagingError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: MatrixEventResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        Ok(result.event_id)
    }

    async fn edit_message(
        &self,
        room_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let txn_id = Uuid::new_v4().to_string();
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}?access_token={}",
            self.homeserver, room_id, txn_id, self.access_token
        );

        let edit_event = MatrixEditEvent {
            msgtype: "m.text",
            body: format!("* {}", new_text),
            new_content: MatrixNewContent {
                msgtype: "m.text",
                body: new_text,
                format: Some("org.matrix.custom.html"),
                formatted_body: Some(new_text),
            },
            relates_to: MatrixRelatesTo {
                rel_type: "m.replace",
                event_id: message_id,
            },
        };

        let response = self
            .client
            .put(&url)
            .json(&edit_event)
            .send()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(MessagingError::SendFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let result: MatrixEventResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        Ok(result.event_id)
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self
            .client
            .get(format!(
                "{}/_matrix/client/v3/account/whoami?access_token={}",
                self.homeserver, self.access_token
            ))
            .send()
            .await;

        response.map(|r| r.status().is_success()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform() {
        let channel = MatrixChannel::new("https://matrix.org", "test-token");
        assert_eq!(channel.platform(), Platform::Matrix);
    }
}
