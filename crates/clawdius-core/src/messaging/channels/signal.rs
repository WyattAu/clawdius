//! Signal Channel Implementation
//!
//! Provides integration with Signal messaging service for messaging gateway.
//! Uses the signal-cli-rest-api for communication.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    recipients: Vec<&'a str>,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    base64_attachments: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SendMessageResponse {
    timestamp: u64,
}

// Reserved for future error handling implementation
#[allow(dead_code)]
#[derive(Deserialize)]
struct SignalError {
    error: String,
}

/// Signal channel adapter using signal-cli-rest-api
pub struct SignalChannel {
    api_endpoint: String,
    number: String,
    client: Client,
}

impl SignalChannel {
    /// Creates a new Signal channel
    ///
    /// # Arguments
    /// * `api_endpoint` - The signal-cli-rest-api endpoint (e.g., "http://localhost:8080")
    /// * `number` - The Signal phone number to use
    pub fn new(api_endpoint: impl Into<String>, number: impl Into<String>) -> Self {
        Self {
            api_endpoint: api_endpoint.into(),
            number: number.into(),
            client: Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/v1/{}", self.api_endpoint, path)
    }
}

#[async_trait]
impl MessagingChannel for SignalChannel {
    fn platform(&self) -> Platform {
        Platform::Signal
    }

    async fn send_message(&self, recipient: &str, text: &str) -> Result<String> {
        let max_len = Platform::Signal.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .post(self.api_url(&format!("send/{}", self.number)))
            .json(&SendMessageRequest {
                recipients: vec![recipient],
                message: text,
                base64_attachments: None,
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

        let result: SendMessageResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        Ok(result.timestamp.to_string())
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        _message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        self.send_message(chat_id, new_text).await
    }

    fn supports_edit(&self) -> bool {
        false
    }

    async fn is_connected(&self) -> bool {
        let response = self.client.get(self.api_url("about")).send().await;

        response.map(|r| r.status().is_success()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform() {
        let channel = SignalChannel::new("http://localhost:8080", "+1234567890");
        assert_eq!(channel.platform(), Platform::Signal);
    }

    #[test]
    fn test_api_url() {
        let channel = SignalChannel::new("http://localhost:8080", "+1234567890");
        let url = channel.api_url("send");
        assert_eq!(url, "http://localhost:8080/v1/send");
    }
}
