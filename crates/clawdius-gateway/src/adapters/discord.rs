//! Discord platform adapter using serenity.
//!
//! Connects Clawdius to Discord via a bot token. Handles incoming
//! messages, file attachments, and response delivery including
//! streaming edits via message editing.
//!
//! # Setup
//!
//! 1. Create a Discord Application at <https://discord.com/developers/applications>
//! 2. Create a bot user and copy the token
//! 3. Enable the `discord` feature: `cargo build --features discord`
//!
//! # Features
//!
//! - Full message handling (text, replies, embeds)
//! - File/attachment download
//! - Message editing for streaming responses
//! - Markdown formatting (Discord-flavored)
//! - Role-based admin detection

use std::sync::Arc;

use crate::adapter::{
    AdapterHealth, IncomingMessage, MessageCallback, OutgoingMessage, Platform, PlatformAdapter,
    PlatformConfig,
};
use crate::error::GatewayError;

/// Discord adapter implementation.
///
/// Uses serenity's `Client` for gateway connection and message handling.
/// Bot events are dispatched through serenity's framework and forwarded
/// to the gateway's [`MessageHandler`].
pub struct DiscordAdapter {
    /// The bot token used to authenticate with Discord.
    token: String,
    /// Counter of messages successfully processed.
    messages_processed: std::sync::atomic::AtomicU64,
    /// Counter of errors encountered.
    error_count: std::sync::atomic::AtomicU64,
    /// Whether the adapter has been started.
    running: std::sync::atomic::AtomicBool,
    /// HTTP client for attachment downloads and direct REST calls.
    http: std::sync::OnceLock<reqwest::Client>,
    message_callback: Arc<tokio::sync::Mutex<Option<MessageCallback>>>,
}

impl DiscordAdapter {
    /// Create a new Discord adapter from a bot token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            error_count: std::sync::atomic::AtomicU64::new(0),
            running: std::sync::atomic::AtomicBool::new(false),
            http: std::sync::OnceLock::new(),
            message_callback: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create from a PlatformConfig.
    pub fn from_config(config: &PlatformConfig) -> Result<Self, GatewayError> {
        let token = config
            .api_token
            .as_ref()
            .ok_or_else(|| GatewayError::Config("DISCORD_BOT_TOKEN not set".to_string()))?;
        Ok(Self::new(token))
    }

    /// Get or create the shared HTTP client.
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(reqwest::Client::new)
    }

    /// Convert a Discord message ID string to serenity's type.
    fn parse_message_id(id: &str) -> Result<serenity::model::id::MessageId, GatewayError> {
        id.parse::<u64>()
            .map(serenity::model::id::MessageId::new)
            .map_err(|_| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("invalid message_id: {id}"),
                source: None,
            })
    }

    /// Convert a Discord channel ID string to serenity's type.
    fn parse_channel_id(id: &str) -> Result<serenity::model::id::ChannelId, GatewayError> {
        id.parse::<u64>()
            .map(serenity::model::id::ChannelId::new)
            .map_err(|_| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("invalid channel_id: {id}"),
                source: None,
            })
    }
}

#[async_trait::async_trait]
impl PlatformAdapter for DiscordAdapter {
    fn platform(&self) -> Platform {
        Platform::Discord
    }

    fn set_message_callback(&self, callback: MessageCallback) {
        let guard = self.message_callback.clone();
        tokio::spawn(async move {
            let mut cb = guard.lock().await;
            *cb = Some(callback);
        });
    }

    async fn start(&self) -> Result<(), GatewayError> {
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // The actual Discord gateway is managed by the serenity Client
        // which runs in its own task. This adapter focuses on the
        // send/edit/download operations; message reception is handled
        // by the serenity event handler dispatched from main.rs.
        //
        // To run the full Discord bot:
        //   let mut client = Client::builder(&token, GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT)
        //       .event_handler(DiscordHandler)
        //       .await?;
        //   client.start().await?;
        Ok(())
    }

    async fn stop(&self) -> Result<(), GatewayError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn send_message(&self, message: OutgoingMessage) -> Result<(), GatewayError> {
        let channel_id = Self::parse_channel_id(&message.chat_id)?;

        // Use Discord's REST API to send the message directly.
        // This avoids needing the full serenity Client in the adapter.
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id.get()
        );

        let mut body = serde_json::json!({
            "content": message.text,
        });

        // Add message reference for replies
        if let Some(ref reply_to) = message.reply_to {
            body["message_reference"] = serde_json::json!({
                "message_id": reply_to,
            });
        }

        let response = self
            .http()
            .post(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("send failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("send failed with {status}: {text}"),
                source: None,
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn edit_message(&self, message_id: &str, new_text: &str) -> Result<(), GatewayError> {
        // message_id is expected to be "channel_id:message_id"
        let parts: Vec<&str> = message_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!(
                    "invalid message_id format (expected channel_id:message_id): {message_id}"
                ),
                source: None,
            });
        }

        let channel_id = parts[0];
        let msg_id = parts[1];

        let url = format!("https://discord.com/api/v10/channels/{channel_id}/messages/{msg_id}");

        let body = serde_json::json!({
            "content": new_text,
        });

        let response = self
            .http()
            .patch(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("edit failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            self.error_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Err(GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("edit failed with {status}: {text}"),
                source: None,
            });
        }

        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn download_attachment(&self, url: &str) -> Result<std::path::PathBuf, GatewayError> {
        let response = self
            .http()
            .get(url)
            .send()
            .await
            .map_err(|e| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("download failed: {e}"),
                source: Some(Box::new(e)),
            })?
            .error_for_status()
            .map_err(|e| GatewayError::Adapter {
                platform: "discord".to_string(),
                message: format!("download error: {e}"),
                source: Some(Box::new(e)),
            })?;

        // Extract filename from URL (Discord attachment URLs end with the filename)
        let filename = url
            .rsplit('/')
            .next()
            .and_then(|f| f.split('?').next())
            .unwrap_or("attachment");

        let dir = std::env::temp_dir().join("clawdius-discord");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(filename);

        let bytes = response.bytes().await.map_err(|e| GatewayError::Adapter {
            platform: "discord".to_string(),
            message: format!("read body failed: {e}"),
            source: Some(Box::new(e)),
        })?;

        std::fs::write(&path, bytes).map_err(GatewayError::Io)?;
        Ok(path)
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth {
            healthy: self.is_running(),
            message: if self.is_running() {
                "connected".to_string()
            } else {
                "stopped".to_string()
            },
            messages_processed: self
                .messages_processed
                .load(std::sync::atomic::Ordering::Relaxed),
            errors: self.error_count.load(std::sync::atomic::Ordering::Relaxed),
            last_message_at: None,
        }
    }
}

/// Serenity event handler that converts Discord events to IncomingMessages
/// and dispatches them to the gateway's MessageHandler.
///
/// This is intended to be used from the main binary when spawning the
/// serenity Client, not directly from the adapter.
#[cfg(feature = "discord")]
pub struct DiscordEventHandler {
    /// Callback invoked when a new message is received.
    pub on_message: std::sync::Arc<
        dyn Fn(IncomingMessage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            + Send
            + Sync,
    >,
}

#[cfg(feature = "discord")]
#[serenity::async_trait]
impl serenity::client::EventHandler for DiscordEventHandler {
    async fn message(
        &self,
        ctx: serenity::client::Context,
        new_message: serenity::model::channel::Message,
    ) {
        // Ignore bot messages
        if new_message.author.bot {
            return;
        }

        // Extract attachments
        let attachments: Vec<crate::adapter::Attachment> = new_message
            .attachments
            .iter()
            .map(|a| crate::adapter::Attachment {
                filename: a.filename.clone(),
                mime_type: a
                    .content_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                url: a.url.clone(),
                size: a.size as usize,
            })
            .collect();

        // Check if user is a guild admin (simplified: check MANAGE_MESSAGES permission)
        let is_admin = if let Some(guild_id) = new_message.guild_id {
            let member = match guild_id.member(&ctx.http, new_message.author.id).await {
                Ok(m) => m,
                Err(_) => return,
            };
            member
                .permissions
                .unwrap_or(serenity::model::permissions::Permissions::empty())
                .contains(serenity::model::permissions::Permissions::MANAGE_MESSAGES)
        } else {
            false
        };

        let incoming = IncomingMessage {
            id: new_message.id.get().to_string(),
            platform: Platform::Discord,
            chat_id: new_message.channel_id.get().to_string(),
            user: crate::adapter::User {
                id: new_message.author.id.get().to_string(),
                name: new_message.author.name.clone(),
                username: match new_message.author.discriminator {
                    Some(d) => Some(format!("{}#{}", new_message.author.name, d)),
                    None => Some(new_message.author.name.clone()),
                },
                is_admin,
            },
            text: new_message.content.clone(),
            reply_to: new_message
                .referenced_message
                .as_ref()
                .map(|rm| rm.id.get().to_string()),
            attachments,
            timestamp: chrono::DateTime::from_timestamp(
                new_message.id.created_at().unix_timestamp(),
                0,
            )
            .unwrap_or_else(chrono::Utc::now),
            metadata: {
                let mut meta = std::collections::HashMap::new();
                if let Some(guild_id) = new_message.guild_id {
                    meta.insert("guild_id".to_string(), serde_json::json!(guild_id.get()));
                }
                meta
            },
        };

        (self.on_message)(incoming).await;
    }
}
