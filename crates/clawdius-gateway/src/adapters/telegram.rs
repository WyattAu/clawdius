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
//! - Long-polling with graceful shutdown via `CancellationToken`
//! - Rate limit awareness

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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
    message_callback: Mutex<Option<MessageCallback>>,
    /// Cancellation token for shutting down the polling loop.
    cancel_token: Mutex<CancellationToken>,
    /// Whether the polling loop is active.
    running: AtomicBool,
    /// Count of successfully processed messages.
    messages_processed: AtomicU64,
    /// Count of errors encountered.
    error_count: AtomicU64,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter from a bot token.
    pub fn new(token: impl Into<String>) -> Self {
        let bot = teloxide::Bot::new(token);
        Self {
            bot,
            message_callback: Mutex::new(None),
            cancel_token: Mutex::new(CancellationToken::new()),
            running: AtomicBool::new(false),
            messages_processed: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
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

    /// Run the long-polling loop until cancelled (static, owned args, no unsafe).
    async fn run_polling_with(
        bot: teloxide::Bot,
        callback: Option<MessageCallback>,
        cancel: CancellationToken,
        running: &AtomicBool,
        processed: &AtomicU64,
        errors: &AtomicU64,
    ) {
        let mut last_update_id: i32 = 0;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Telegram polling stopped");
                    break;
                }
                result = bot.get_updates()
                    .offset(last_update_id)
                    .timeout(std::time::Duration::from_secs(35))
                    .allowed_update(teloxide::types::AllowedUpdate::Message)
                    .limit(100)
                => {
                    match result {
                        Ok(updates) => {
                            for update in &updates {
                                last_update_id = update.id.0 + 1;
                                if let Some(incoming) = Self::convert_update(update.clone()).await {
                                    Self::dispatch_with(&callback, incoming, processed).await;
                                }
                            }
                        }
                        Err(teloxide::RequestError::Api(teloxide::ApiError::RetryAfter(dur))) => {
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
        // Store the callback using interior mutability.
        // We spawn a task because set_message_callback is &self (not async).
        let guard = self.message_callback.clone();
        tokio::spawn(async move {
            let mut cb = guard.lock().await;
            *cb = Some(callback);
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

        // Reset cancellation token
        {
            let mut token = self.cancel_token.lock().await;
            *token = CancellationToken::new();
        }

        self.running.store(true, Ordering::Relaxed);

        // Spawn the polling loop as a background task.
        // The loop runs until cancel_token is cancelled (via stop()).
        let running = self.running.clone();
        let _polling_handle = tokio::spawn(async move {
            // We need to get `self` into the spawned task.
            // Instead, we use a different approach: store the polling handle.
        });

        // Actually, since start() is &self, we can't move self into spawn.
        // Instead, we make run_polling take &self and await it directly.
        // But that would block start() from returning.
        //
        // The solution: we need interior mutability for the spawned handle.
        // For simplicity, we'll run polling in the current task and return
        // immediately after spawning it.

        // Wait for the message callback to be set (brief wait)
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Spawn the polling loop
        let bot = self.bot.clone();
        let callback = self.message_callback.lock().await.clone();
        let cancel = self.cancel_token.lock().await.clone();
        let running = &self.running;
        let processed = &self.messages_processed;
        let errors = &self.error_count;
        tokio::spawn(async move {
            Self::run_polling_with(bot, callback, cancel, running, processed, errors).await;
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        tracing::info!("Telegram adapter stopping");
        self.cancel_token.lock().await.cancel();
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
