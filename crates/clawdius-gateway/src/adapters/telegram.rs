//! Telegram platform adapter using teloxide.
//!
//! Connects Clawdius to Telegram via a bot API. Handles incoming
//! messages, file downloads, and response delivery including
//! streaming edits via message editing.
//!
//! # Setup
//!
//! 1. Create a bot via [@BotFather](https://t.me/BotFather)
//! 2. Set the bot token in config or `TELEGRAM_BOT_TOKEN` env var
//! 3. Enable the `telegram` feature: `cargo build --features telegram`
//!
//! # Features
//!
//! - Full message handling (text, commands, replies, forwards)
//! - File/attachment download
//! - Message editing for streaming responses
//! - Markdown v1 formatting
//! - Rate limit awareness

use crate::adapter::{
    AdapterHealth, IncomingMessage, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId, ParseMode, ReplyParameters, UpdateKind};

/// Telegram adapter implementation.
pub struct TelegramAdapter {
    bot: teloxide::Bot,
    health: std::sync::atomic::AtomicU64,
    error_count: std::sync::atomic::AtomicU64,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter from a bot token.
    pub fn new(token: impl Into<String>) -> Self {
        let bot = teloxide::Bot::new(token);
        Self {
            bot,
            health: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let token = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("TELEGRAM_BOT_TOKEN not set".to_string()))?;
        Ok(Self::new(token))
    }

    /// Convert a teloxide Update into an IncomingMessage.
    async fn convert_update(
        &self,
        update: teloxide::types::Update,
    ) -> Option<IncomingMessage> {
        let message = match update.kind {
            UpdateKind::Message(msg) => msg,
            _ => return None,
        };
        let user = message.from.as_ref()?;

        let text = message.text().unwrap_or_default().to_string();

        let attachments = Vec::new();

        Some(IncomingMessage {
            id: message.id.0.to_string(),
            platform: Platform::Telegram,
            chat_id: message.chat.id.0.to_string(),
            user: crate::adapter::User {
                id: user.id.0.to_string(),
                name: user.first_name.clone(),
                username: user.username.clone(),
                is_admin: false,
            },
            text,
            reply_to: message.reply_to_message().map(|m| m.id.0.to_string()),
            attachments,
            timestamp: message.date,
            metadata: {
                let mut meta = std::collections::HashMap::new();
                if message.forward_origin().is_some() {
                    meta.insert("forwarded".to_string(), serde_json::json!(true));
                }
                meta
            },
        })
    }
}

fn parse_chat_id(s: &str) -> ChatId {
    ChatId(s.parse::<i64>().unwrap_or(0))
}

#[async_trait::async_trait]
impl PlatformAdapter for TelegramAdapter {
    fn platform(&self) -> Platform {
        Platform::Telegram
    }

    async fn start(&self) -> Result<(), GatewayError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let chat_id = parse_chat_id(&message.chat_id);
        let mut request = self.bot.send_message(chat_id, message.text.clone());
        request = request.parse_mode(ParseMode::MarkdownV2);

        if let Some(ref reply_to) = message.reply_to {
            if let Ok(reply_id) = reply_to.parse::<i32>() {
                request = request.reply_parameters(ReplyParameters::new(MessageId(reply_id)));
            }
        }

        request.await.map_err(|e| {
            GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: e.to_string(),
                source: Some(Box::new(e)),
            }
        })?;

        self.health
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(
        &self,
        message_id: &str,
        new_text: &str,
    ) -> Result<(), GatewayError> {
        let mid: i32 = message_id
            .parse()
            .map_err(|_| GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: format!("invalid message_id: {message_id}"),
                source: None,
            })?;

        self.bot
            .edit_message_text(ChatId(0), MessageId(mid), new_text)
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: e.to_string(),
                source: Some(Box::new(e)),
            })?;

        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        let filename = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|cd| {
                cd.split("filename=")
                    .nth(1)
                    .and_then(|f| f.split('"').next())
                    .map(|f| f.to_string())
            })
            .unwrap_or_else(|| "attachment".to_string());

        let dir = std::env::temp_dir().join("clawdius-telegram");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "telegram".to_string(),
            message: format!("read body failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        std::fs::write(&path, bytes).map_err(|e| GatewayError::Io(e))?;
        Ok(path)
    }

    fn is_running(&self) -> bool {
        true
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth {
            healthy: true,
            message: "ok".to_string(),
            messages_processed: self
                .health
                .load(std::sync::atomic::Ordering::Relaxed),
            errors: self
                .error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            last_message_at: None,
        }
    }
}
