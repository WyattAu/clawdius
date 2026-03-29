//! WhatsApp Channel Implementation
//!
//! Provides integration with WhatsApp Business API for messaging gateway.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::messaging::types::{MessagingError, Platform, Result};

use super::MessagingChannel;

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    messaging_product: &'a str,
    recipient_type: &'a str,
    to: &'a str,
    #[serde(rename = "type")]
    message_type: &'a str,
    text: TextMessage<'a>,
}

#[derive(Serialize)]
struct EditMessageRequest<'a> {
    messaging_product: &'a str,
    message: EditMessageBody<'a>,
}

#[derive(Serialize)]
struct EditMessageBody<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    message_type: &'a str,
    text: EditTextBody<'a>,
}

#[derive(Serialize)]
struct EditTextBody<'a> {
    body: &'a str,
}

#[derive(Serialize)]
struct TextMessage<'a> {
    preview_url: bool,
    body: &'a str,
}

#[derive(Deserialize)]
struct MessageResponse {
    messages: Vec<MessageId>,
}

#[derive(Deserialize)]
struct MessageId {
    id: String,
}

// Reserved for future error handling implementation
#[allow(dead_code)]
#[derive(Deserialize)]
struct WhatsAppError {
    message: String,
}

/// WhatsApp Business API channel adapter
pub struct WhatsAppChannel {
    phone_number_id: String,
    access_token: String,
    client: Client,
    base_url: Option<String>,
}

impl WhatsAppChannel {
    /// Creates a new WhatsApp channel
    ///
    /// # Arguments
    /// * `phone_number_id` - The WhatsApp Business phone number ID
    /// * `access_token` - The WhatsApp Business API access token
    pub fn new(phone_number_id: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            phone_number_id: phone_number_id.into(),
            access_token: access_token.into(),
            client: Client::new(),
            base_url: None,
        }
    }

    fn api_base(&self) -> String {
        self.base_url
            .as_deref()
            .unwrap_or("https://graph.facebook.com/v18.0")
            .to_string()
    }

    fn api_url(&self) -> String {
        format!("{}/{}/messages", self.api_base(), self.phone_number_id)
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        phone_number_id: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            phone_number_id: phone_number_id.into(),
            access_token: access_token.into(),
            client: Client::new(),
            base_url: Some(base_url.into()),
        }
    }
}

#[async_trait]
impl MessagingChannel for WhatsAppChannel {
    fn platform(&self) -> Platform {
        Platform::WhatsApp
    }

    async fn send_message(&self, recipient: &str, text: &str) -> Result<String> {
        let max_len = Platform::WhatsApp.max_message_length();
        if text.len() > max_len {
            return Err(MessagingError::MessageTooLong {
                length: text.len(),
                max: max_len,
            });
        }

        let response = self
            .client
            .post(self.api_url())
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .json(&SendMessageRequest {
                messaging_product: "whatsapp",
                recipient_type: "individual",
                to: recipient,
                message_type: "text",
                text: TextMessage {
                    preview_url: false,
                    body: text,
                },
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

        let result: MessageResponse =
            serde_json::from_str(&body).map_err(|e| MessagingError::ParseError(e.to_string()))?;

        result
            .messages
            .first()
            .map(|m| m.id.clone())
            .ok_or_else(|| MessagingError::SendFailed("No message ID returned".to_string()))
    }

    async fn edit_message(
        &self,
        _chat_id: &str,
        message_id: &str,
        new_text: &str,
    ) -> Result<String> {
        let url = format!("{}/{}", self.api_base(), message_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .json(&EditMessageRequest {
                messaging_product: "whatsapp",
                message: EditMessageBody {
                    id: message_id,
                    message_type: "text",
                    text: EditTextBody { body: new_text },
                },
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

        Ok(message_id.to_string())
    }

    fn supports_edit(&self) -> bool {
        true
    }

    async fn is_connected(&self) -> bool {
        let response = self
            .client
            .get(format!("{}/{}", self.api_base(), self.phone_number_id))
            .header("Authorization", format!("Bearer {}", self.access_token))
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
        let channel =
            WhatsAppChannel::with_base_url("https://example.com", "123456789", "test-token");
        assert_eq!(channel.platform(), Platform::WhatsApp);
    }

    #[test]
    fn test_api_url() {
        let channel =
            WhatsAppChannel::with_base_url("https://example.com", "123456789", "test-token");
        let url = channel.api_url();
        assert!(url.contains("123456789"));
        assert!(url.contains("example.com"));
    }
}
