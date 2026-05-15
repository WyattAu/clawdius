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
//! - Markdown v2 formatting
//! - Long-polling with graceful shutdown via `tokio::sync::Notify`
//! - Rate limit awareness

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;
use teloxide::prelude::*;
use teloxide::types::{ChatId, MessageId, ParseMode, ReplyParameters, UpdateKind};

/// Telegram adapter implementation.
///
/// Uses teloxide long-polling to receive updates. Incoming messages
/// are delivered to the gateway via the message callback.
pub struct TelegramAdapter {
    bot: teloxide::Bot,
    /// Callback for delivering incoming messages to the gateway.
    message_callback: Arc<Mutex<Option<MessageCallback>>>,
    /// Notification for shutting down the polling loop.
    cancel_notify: Arc<Notify>,
    /// Whether the polling loop is active.
    running: Arc<AtomicBool>,
    /// Count of successfully processed messages.
    messages_processed: Arc<AtomicU64>,
    /// Count of errors encountered.
    error_count: Arc<AtomicU64>,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter from a bot token.
    pub fn new(token: impl Into<String>) -> Self {
        let bot = teloxide::Bot::new(token);
        Self {
            bot,
            message_callback: Arc::new(Mutex::new(None)),
            cancel_notify: Arc::new(Notify::new()),
            running: Arc::new(AtomicBool::new(false)),
            messages_processed: Arc::new(AtomicU64::new(0)),
            error_count: Arc::new(AtomicU64::new(0)),
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
    async fn convert_update(update: teloxide::types::Update) -> Option<IncomingMessage> {
        let message = match update.kind {
            UpdateKind::Message(msg) => msg,
            _ => return None,
        };
        let user = message.from.as_ref()?;

        let text = message.text().unwrap_or_default().to_string();

        // Skip empty messages (photos, stickers, etc. without caption)
        if text.is_empty() {
            return None;
        }

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
            attachments: Vec::new(),
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

    /// Dispatch an incoming message via the callback.
    async fn dispatch_message(&self, message: IncomingMessage) {
        let callback = {
            let guard = self.message_callback.lock().await;
            guard.clone()
        };

        if let Some(cb) = callback {
            cb(message).await;
            self.messages_processed.fetch_add(1, Ordering::Relaxed);
        } else {
            tracing::warn!("Telegram adapter received message but no callback is set");
        }
    }

    /// Dispatch a message using pre-cloned callback and counters (no &self).
    async fn dispatch_with(
        callback: &Option<MessageCallback>,
        message: IncomingMessage,
        processed: &AtomicU64,
    ) {
        if let Some(cb) = callback {
            cb(message).await;
            processed.fetch_add(1, Ordering::Relaxed);
        } else {
            tracing::warn!("Telegram adapter received message but no callback is set");
        }
    }

    /// Run the long-polling loop until cancelled.
    async fn run_polling_with(
        bot: teloxide::Bot,
        callback: Option<MessageCallback>,
        cancel: Arc<Notify>,
        running: &AtomicBool,
        processed: &AtomicU64,
        errors: &AtomicU64,
    ) {
        let mut last_update_id: i32 = 0;

        loop {
            tokio::select! {
                _ = cancel.notified() => {
                    tracing::info!("Telegram polling stopped");
                    break;
                }
                result = bot.get_updates()
                    .offset(last_update_id)
                    .timeout(35u32)
                    .allowed_updates(vec![teloxide::types::AllowedUpdate::Message])
                    .limit(100u8)
                => {
                    match result {
                        Ok(updates) => {
                            for update in &updates {
                                last_update_id = (update.id.0 + 1) as i32;
                                if let Some(incoming) = Self::convert_update(update.clone()).await {
                                    Self::dispatch_with(&callback, incoming, processed).await;
                                }
                            }
                        }
                        Err(teloxide::RequestError::RetryAfter(secs)) => {
                            let dur = secs.duration();
                            tracing::warn!("Telegram rate limited, retrying after {dur:?}");
                            tokio::time::sleep(dur).await;
                        }
                        Err(teloxide::RequestError::Network(err)) => {
                            tracing::warn!("Telegram network error: {err}, retrying in 2s");
                            errors.fetch_add(1, Ordering::Relaxed);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                        Err(e) => {
                            tracing::error!("Telegram polling error: {e}");
                            errors.fetch_add(1, Ordering::Relaxed);
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        }

        running.store(false, Ordering::Relaxed);
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

    fn set_message_callback(&self, callback: MessageCallback) {
        let cb = Arc::clone(&self.message_callback);
        tokio::spawn(async move {
            let mut guard = cb.lock().await;
            *guard = Some(callback);
        });
    }

    async fn start(&self) -> Result<(), GatewayError> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        // Verify the bot token by calling getMe
        let me = self.bot.get_me().await.map_err(|e| GatewayError::Adapter {
            platform: "telegram".to_string(),
            message: format!("failed to verify bot token: {e}"),
            source: Some(Box::new(e)),
        })?;

        tracing::info!(bot_id = me.id.0, bot_name = %me.first_name, "Telegram adapter starting");

        self.running.store(true, Ordering::Relaxed);

        // Wait for the message callback to be set (brief wait)
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Spawn the polling loop
        let bot = self.bot.clone();
        let callback = self.message_callback.lock().await.clone();
        let cancel = Arc::clone(&self.cancel_notify);
        let running = Arc::clone(&self.running);
        let processed = Arc::clone(&self.messages_processed);
        let errors = Arc::clone(&self.error_count);
        tokio::spawn(async move {
            Self::run_polling_with(bot, callback, cancel, &running, &processed, &errors).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        tracing::info!("Telegram adapter stopping");
        self.cancel_notify.notify_one();
        self.running.store(false, Ordering::Relaxed);
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
            self.error_count.fetch_add(1, Ordering::Relaxed);
            GatewayError::Adapter {
                platform: "telegram".to_string(),
                message: e.to_string(),
                source: Some(Box::new(e)),
            }
        })?;

        self.messages_processed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        let mid: i32 = message_id.parse().map_err(|_| GatewayError::Adapter {
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

        std::fs::write(&path, bytes).map_err(GatewayError::Io)?;
        Ok(path)
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth {
            healthy: self.running.load(Ordering::Relaxed),
            message: if self.running.load(Ordering::Relaxed) {
                "polling".to_string()
            } else {
                "stopped".to_string()
            },
            messages_processed: self.messages_processed.load(Ordering::Relaxed),
            errors: self.error_count.load(Ordering::Relaxed),
            last_message_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn test_parse_chat_id_valid() {
        let chat_id = parse_chat_id("123456");
        assert_eq!(chat_id.0, 123456);
    }

    #[test]
    fn test_parse_chat_id_invalid() {
        let chat_id = parse_chat_id("not_a_number");
        assert_eq!(chat_id.0, 0);
    }

    #[test]
    fn test_parse_chat_id_negative() {
        let chat_id = parse_chat_id("-1001234567890");
        assert_eq!(chat_id.0, -1001234567890);
    }

    #[test]
    fn test_parse_chat_id_empty() {
        let chat_id = parse_chat_id("");
        assert_eq!(chat_id.0, 0);
    }

    #[test]
    fn test_telegram_adapter_platform() {
        let adapter = TelegramAdapter::new("fake-bot-token");
        assert_eq!(adapter.platform(), Platform::Telegram);
    }

    #[test]
    fn test_telegram_adapter_not_running_initially() {
        let adapter = TelegramAdapter::new("fake-bot-token");
        assert!(!adapter.is_running());
    }

    #[test]
    fn test_telegram_adapter_health_stopped() {
        let adapter = TelegramAdapter::new("fake-bot-token");
        let health = adapter.health();
        assert!(!health.healthy);
        assert_eq!(health.message, "stopped");
        assert_eq!(health.messages_processed, 0);
        assert_eq!(health.errors, 0);
    }

    #[test]
    fn test_telegram_from_config_missing_token() {
        let config = PlatformConfig::new(Platform::Telegram);
        let result = TelegramAdapter::from_config(&config);
        let err = result.err().expect("should be err").to_string();
        assert!(err.contains("TELEGRAM_BOT_TOKEN"));
    }

    #[test]
    fn test_telegram_from_config_valid() {
        let mut config = PlatformConfig::new(Platform::Telegram);
        config.api_token = Some("123456:ABC-DEF".to_string());
        let adapter = TelegramAdapter::from_config(&config).unwrap();
        assert_eq!(adapter.platform(), Platform::Telegram);
    }

    #[test]
    fn test_telegram_outgoing_message_json_format() {
        let msg = OutgoingMessage::new(Platform::Telegram, "123456", "hello telegram");
        let body = serde_json::json!({
            "chat_id": msg.chat_id,
            "text": msg.text,
        });
        assert_eq!(body["chat_id"], "123456");
        assert_eq!(body["text"], "hello telegram");
    }

    #[test]
    fn test_telegram_outgoing_message_empty_text() {
        let msg = OutgoingMessage::new(Platform::Telegram, "123456", "");
        assert_eq!(msg.text, "");
    }

    #[test]
    fn test_telegram_outgoing_message_unicode() {
        let msg = OutgoingMessage::new(Platform::Telegram, "123456", "Привет мир 🌍");
        assert_eq!(msg.text, "Привет мир 🌍");
    }
}
