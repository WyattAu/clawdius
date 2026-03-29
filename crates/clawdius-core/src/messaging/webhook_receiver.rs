//! Webhook Receiver
//!
//! Framework-agnostic webhook receiver for multi-platform messaging gateways.
//! Defines platform-specific webhook configurations, signature verification,
//! and request parsing logic that can be wired into any HTTP server.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::protocol::{AuthenticatedUser, NormalizedMessage, PlatformMetadata};
use super::types::{ChannelConfig, MessagingError, Platform, Result};

// ---------------------------------------------------------------------------
// Platform-specific webhook configurations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramWebhookConfig {
    pub secret_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordWebhookConfig {
    pub public_key_pem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixWebhookConfig {
    pub access_token: String,
    pub homeserver_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackWebhookConfig {
    pub signing_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocketChatWebhookConfig {
    pub token: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalWebhookConfig {
    pub verification_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppWebhookConfig {
    pub verify_token: String,
    pub app_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
pub enum WebhookConfig {
    Telegram(TelegramWebhookConfig),
    Discord(DiscordWebhookConfig),
    Matrix(MatrixWebhookConfig),
    Slack(SlackWebhookConfig),
    RocketChat(RocketChatWebhookConfig),
    Signal(SignalWebhookConfig),
    WhatsApp(WhatsAppWebhookConfig),
}

impl WebhookConfig {
    pub fn platform(&self) -> Platform {
        match self {
            Self::Telegram(_) => Platform::Telegram,
            Self::Discord(_) => Platform::Discord,
            Self::Matrix(_) => Platform::Matrix,
            Self::Slack(_) => Platform::Slack,
            Self::RocketChat(_) => Platform::RocketChat,
            Self::Signal(_) => Platform::Signal,
            Self::WhatsApp(_) => Platform::WhatsApp,
        }
    }
}

// ---------------------------------------------------------------------------
// Framework-agnostic request representation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct WebhookHeaders {
    pub headers: HashMap<String, String>,
}

impl WebhookHeaders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(String::as_str)
    }

    pub fn get_case_insensitive(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(key.into(), value.into());
    }
}

#[derive(Debug, Clone)]
pub struct WebhookRequest {
    pub platform: Platform,
    pub body: Vec<u8>,
    pub headers: WebhookHeaders,
    pub query_params: HashMap<String, String>,
    pub source_ip: String,
    pub received_at: DateTime<Utc>,
}

impl WebhookRequest {
    pub fn new(platform: Platform, body: Vec<u8>) -> Self {
        Self {
            platform,
            body,
            headers: WebhookHeaders::new(),
            query_params: HashMap::new(),
            source_ip: String::new(),
            received_at: Utc::now(),
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key, value);
        self
    }

    pub fn with_query_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(key.into(), value.into());
        self
    }

    pub fn with_source_ip(mut self, ip: impl Into<String>) -> Self {
        self.source_ip = ip.into();
        self
    }

    pub fn body_str(&self) -> &str {
        std::str::from_utf8(&self.body).unwrap_or("")
    }
}

// ---------------------------------------------------------------------------
// Verification result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationResult {
    Verified,
    InvalidSignature,
    MissingCredentials,
    UnsupportedPlatform,
}

// ---------------------------------------------------------------------------
// WebhookReceiver
// ---------------------------------------------------------------------------

pub struct WebhookReceiver {
    configs: HashMap<Platform, WebhookConfig>,
    channel_configs: HashMap<Platform, ChannelConfig>,
}

impl WebhookReceiver {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            channel_configs: HashMap::new(),
        }
    }

    pub fn register_platform(
        &mut self,
        config: WebhookConfig,
        channel_config: Option<ChannelConfig>,
    ) {
        let platform = config.platform();
        self.configs.insert(platform, config);
        if let Some(cc) = channel_config {
            self.channel_configs.insert(platform, cc);
        }
    }

    pub fn unregister_platform(&mut self, platform: Platform) {
        self.configs.remove(&platform);
        self.channel_configs.remove(&platform);
    }

    pub fn is_registered(&self, platform: Platform) -> bool {
        self.configs.contains_key(&platform)
    }

    pub fn registered_platforms(&self) -> Vec<Platform> {
        self.configs.keys().copied().collect()
    }

    pub fn get_webhook_config(&self, platform: Platform) -> Option<&WebhookConfig> {
        self.configs.get(&platform)
    }

    pub fn get_channel_config(&self, platform: Platform) -> Option<&ChannelConfig> {
        self.channel_configs.get(&platform)
    }

    pub fn verify_signature(&self, request: &WebhookRequest) -> VerificationResult {
        let config = match self.configs.get(&request.platform) {
            Some(c) => c,
            None => return VerificationResult::UnsupportedPlatform,
        };

        match config {
            WebhookConfig::Telegram(cfg) => verify_telegram(request, cfg),
            WebhookConfig::Discord(cfg) => verify_discord(request, cfg),
            WebhookConfig::Matrix(cfg) => verify_matrix(request, cfg),
            WebhookConfig::Slack(cfg) => verify_slack(request, cfg),
            WebhookConfig::RocketChat(cfg) => verify_rocketchat(request, cfg),
            WebhookConfig::Signal(cfg) => verify_signal(request, cfg),
            WebhookConfig::WhatsApp(cfg) => verify_whatsapp(request, cfg),
        }
    }

    pub fn parse_webhook_body(&self, request: &WebhookRequest) -> Result<NormalizedMessage> {
        match self.verify_signature(request) {
            VerificationResult::Verified => {}
            VerificationResult::InvalidSignature => {
                return Err(MessagingError::AuthenticationFailed(
                    "Invalid webhook signature".into(),
                ));
            }
            VerificationResult::MissingCredentials => {
                return Err(MessagingError::AuthenticationFailed(
                    "Missing webhook credentials".into(),
                ));
            }
            VerificationResult::UnsupportedPlatform => {
                return Err(MessagingError::ChannelNotSupported(
                    request.platform.to_string(),
                ));
            }
        }

        match request.platform {
            Platform::Telegram => parse_telegram_body(request),
            Platform::Discord => parse_discord_body(request),
            Platform::Matrix => parse_matrix_body(request),
            Platform::Slack => parse_slack_body(request),
            Platform::RocketChat => parse_rocketchat_body(request),
            Platform::Signal => parse_signal_body(request),
            Platform::WhatsApp => parse_whatsapp_body(request),
            Platform::Webhook => Err(MessagingError::ChannelNotSupported(
                "Generic webhook parsing not supported".into(),
            )),
        }
    }
}

impl Default for WebhookReceiver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Platform-specific verification
// ---------------------------------------------------------------------------

fn verify_telegram(request: &WebhookRequest, cfg: &TelegramWebhookConfig) -> VerificationResult {
    let token = request
        .query_params
        .get("secret_token")
        .map(String::as_str)
        .or_else(|| request.headers.get("X-Telegram-Bot-Api-Secret-Token"));
    match token {
        Some(t) if t == cfg.secret_token => VerificationResult::Verified,
        Some(_) => VerificationResult::InvalidSignature,
        None => VerificationResult::MissingCredentials,
    }
}

fn verify_discord(request: &WebhookRequest, _cfg: &DiscordWebhookConfig) -> VerificationResult {
    let sig = request.headers.get("X-Signature-Ed25519");
    let ts = request.headers.get("X-Signature-Timestamp");
    match (sig, ts) {
        (Some(signature), Some(timestamp)) => {
            if signature.is_empty() || timestamp.is_empty() {
                return VerificationResult::MissingCredentials;
            }
            VerificationResult::InvalidSignature
        }
        _ => VerificationResult::MissingCredentials,
    }
}

fn verify_matrix(request: &WebhookRequest, cfg: &MatrixWebhookConfig) -> VerificationResult {
    let token = request.headers.get("Authorization");
    match token {
        Some(t) if t == format!("Bearer {}", cfg.access_token) => VerificationResult::Verified,
        Some(t) if t == cfg.access_token => VerificationResult::Verified,
        Some(_) => VerificationResult::InvalidSignature,
        None => VerificationResult::MissingCredentials,
    }
}

fn verify_slack(request: &WebhookRequest, cfg: &SlackWebhookConfig) -> VerificationResult {
    let ts = request.headers.get("X-Slack-Request-Timestamp");
    let sig = request.headers.get("X-Slack-Signature");
    match (ts, sig) {
        (Some(timestamp), Some(signature)) => {
            let base_string = format!(
                "v0:{}:{}",
                timestamp,
                std::str::from_utf8(&request.body).unwrap_or("")
            );
            let mut key = [0u8; 32];
            let secret_bytes = cfg.signing_secret.as_bytes();
            let copy_len = secret_bytes.len().min(32);
            key[..copy_len].copy_from_slice(&secret_bytes[..copy_len]);
            let computed = blake3::keyed_hash(&key, base_string.as_bytes());
            let expected = format!("v0={}", computed.to_hex());
            if signature == &expected {
                VerificationResult::Verified
            } else {
                VerificationResult::InvalidSignature
            }
        }
        _ => VerificationResult::MissingCredentials,
    }
}

fn verify_rocketchat(
    request: &WebhookRequest,
    cfg: &RocketChatWebhookConfig,
) -> VerificationResult {
    let token = request
        .headers
        .get("X-Rocket-Chat-Token")
        .or_else(|| request.query_params.get("token").map(String::as_str));
    match token {
        Some(t) if t == cfg.token => VerificationResult::Verified,
        Some(_) => VerificationResult::InvalidSignature,
        None => VerificationResult::MissingCredentials,
    }
}

fn verify_signal(request: &WebhookRequest, cfg: &SignalWebhookConfig) -> VerificationResult {
    let token = request.headers.get("X-Signal-Token");
    match token {
        Some(t) if t == cfg.verification_token => VerificationResult::Verified,
        Some(_) => VerificationResult::InvalidSignature,
        None => VerificationResult::MissingCredentials,
    }
}

fn verify_whatsapp(request: &WebhookRequest, cfg: &WhatsAppWebhookConfig) -> VerificationResult {
    let mode = request.query_params.get("hub.mode").map(String::as_str);
    let token = request
        .query_params
        .get("hub.verify_token")
        .map(String::as_str);
    match (mode, token) {
        (Some(m), Some(t)) if m == "subscribe" && t == cfg.verify_token => {
            VerificationResult::Verified
        }
        (Some(_), Some(_)) => VerificationResult::InvalidSignature,
        _ => VerificationResult::MissingCredentials,
    }
}

fn parse_telegram_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    struct TelegramUpdate {
        update_id: i64,
        message: Option<TelegramMessage>,
    }

    #[derive(Deserialize)]
    struct TelegramMessage {
        message_id: i64,
        from: Option<TelegramUser>,
        chat: TelegramChat,
        text: Option<String>,
        date: i64,
    }

    #[derive(Deserialize)]
    struct TelegramUser {
        id: i64,
        is_bot: bool,
        first_name: String,
        #[serde(default)]
        last_name: Option<String>,
        #[serde(default)]
        username: Option<String>,
    }

    #[derive(Deserialize)]
    struct TelegramChat {
        id: i64,
    }

    let update: TelegramUpdate = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Telegram parse error: {e}")))?;

    let msg = update
        .message
        .ok_or_else(|| MessagingError::ParseError("No message in Telegram update".into()))?;

    let user = msg
        .from
        .ok_or_else(|| MessagingError::ParseError("No sender in Telegram message".into()))?;

    let display_name = match &user.last_name {
        Some(last) => format!("{} {}", user.first_name, last),
        None => user.first_name.clone(),
    };

    let mut auth_user = AuthenticatedUser::new(user.id.to_string())
        .with_display_name(display_name)
        .with_username(user.username.unwrap_or_default());
    if user.is_bot {
        auth_user = auth_user.bot();
    }

    let timestamp = DateTime::from_timestamp(msg.date, 0).unwrap_or(request.received_at);

    let content = msg.text.unwrap_or_default();

    let metadata = PlatformMetadata::Telegram {
        chat_id: msg.chat.id,
        message_id: msg.message_id,
        reply_to_message_id: None,
    };

    Ok(NormalizedMessage::new(
        format!("tg_{}_{}", update.update_id, msg.message_id),
        Platform::Telegram,
        auth_user,
        content,
        timestamp,
        metadata,
    ))
}

fn parse_discord_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct DiscordInteraction {
        id: String,
        r#type: u8,
        data: Option<DiscordInteractionData>,
        member: Option<DiscordMember>,
        user: Option<DiscordUser>,
        guild_id: Option<String>,
        channel_id: String,
    }

    #[derive(Deserialize)]
    struct DiscordInteractionData {
        name: String,
        options: Option<Vec<serde_json::Value>>,
    }

    #[derive(Deserialize)]
    struct DiscordMember {
        user: DiscordUser,
        nick: Option<String>,
    }

    #[derive(Deserialize)]
    struct DiscordUser {
        id: String,
        username: String,
        #[serde(default)]
        bot: bool,
    }

    let interaction: DiscordInteraction = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Discord parse error: {e}")))?;

    let (user_data, display_name) = match (&interaction.member, &interaction.user) {
        (Some(member), _) => (
            &member.user,
            member
                .nick
                .clone()
                .unwrap_or_else(|| member.user.username.clone()),
        ),
        (_, Some(user)) => (user, user.username.clone()),
        _ => {
            return Err(MessagingError::ParseError(
                "No user in Discord interaction".into(),
            ));
        }
    };

    let mut auth_user = AuthenticatedUser::new(&user_data.id)
        .with_display_name(display_name)
        .with_username(&user_data.username);
    if user_data.bot {
        auth_user = auth_user.bot();
    }

    let content = interaction
        .data
        .as_ref()
        .map(|d| {
            let options = d
                .options
                .as_ref()
                .map(|opts| {
                    opts.iter()
                        .filter_map(|o| o.get("value").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            format!("/{} {}", d.name, options)
        })
        .unwrap_or_default();

    let guild_id = interaction
        .guild_id
        .and_then(|g| g.parse::<u64>().ok())
        .unwrap_or(0);
    let channel_id = interaction
        .channel_id
        .parse::<u64>()
        .map_err(|e| MessagingError::ParseError(format!("Invalid Discord channel_id: {e}")))?;

    let metadata = PlatformMetadata::Discord {
        guild_id,
        channel_id,
        message_id: 0,
        referenced_message_id: None,
    };

    Ok(NormalizedMessage::new(
        interaction.id,
        Platform::Discord,
        auth_user,
        content,
        request.received_at,
        metadata,
    ))
}

fn parse_matrix_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    struct MatrixEvent {
        event_id: String,
        r#type: String,
        room_id: String,
        sender: String,
        content: MatrixContent,
        origin_server_ts: Option<u64>,
    }

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct MatrixContent {
        msgtype: Option<String>,
        body: Option<String>,
    }

    let event: MatrixEvent = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Matrix parse error: {e}")))?;

    if event.r#type != "m.room.message" {
        return Err(MessagingError::ParseError(format!(
            "Unsupported Matrix event type: {}",
            event.r#type
        )));
    }

    let user_id = event.sender.strip_prefix('@').unwrap_or(&event.sender);
    let auth_user = AuthenticatedUser::new(user_id.to_string());

    let timestamp = event
        .origin_server_ts
        .and_then(|ts| DateTime::from_timestamp_millis(ts as i64))
        .unwrap_or(request.received_at);

    let content = event.content.body.unwrap_or_default();

    let metadata = PlatformMetadata::Matrix {
        room_id: event.room_id,
        event_id: event.event_id.clone(),
        sender: event.sender,
    };

    Ok(NormalizedMessage::new(
        event.event_id,
        Platform::Matrix,
        auth_user,
        content,
        timestamp,
        metadata,
    ))
}

fn parse_slack_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct SlackEventWrapper {
        r#type: String,
        challenge: Option<String>,
        event: Option<SlackInnerEvent>,
        team_id: Option<String>,
    }

    #[derive(Deserialize)]
    struct SlackInnerEvent {
        r#type: String,
        user: Option<String>,
        text: Option<String>,
        ts: Option<String>,
        channel: Option<String>,
        thread_ts: Option<String>,
    }

    let wrapper: SlackEventWrapper = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Slack parse error: {e}")))?;

    if wrapper.r#type == "url_verification" {
        return Err(MessagingError::ParseError(
            "URL verification challenge, not a message event".into(),
        ));
    }

    let inner = wrapper
        .event
        .ok_or_else(|| MessagingError::ParseError("No event payload in Slack webhook".into()))?;

    if inner.r#type != "message" || inner.user.is_none() {
        return Err(MessagingError::ParseError(
            "Not a user message event".into(),
        ));
    }

    let auth_user = AuthenticatedUser::new(inner.user.unwrap_or_default());

    let content = inner.text.unwrap_or_default();

    let metadata = PlatformMetadata::Slack {
        team_id: wrapper.team_id.unwrap_or_default(),
        channel_id: inner.channel.unwrap_or_default(),
        thread_ts: inner.thread_ts,
        parent_message_ts: None,
    };

    Ok(NormalizedMessage::new(
        inner.ts.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        Platform::Slack,
        auth_user,
        content,
        request.received_at,
        metadata,
    ))
}

fn parse_rocketchat_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    struct RocketChatWebhookBody {
        user_id: Option<String>,
        user_name: Option<String>,
        channel_id: Option<String>,
        message_id: Option<String>,
        text: Option<String>,
    }

    let body: RocketChatWebhookBody = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Rocket.Chat parse error: {e}")))?;

    let user_id = body.user_id.unwrap_or_default();
    let auth_user = AuthenticatedUser::new(&user_id)
        .with_display_name(body.user_name.clone().unwrap_or_default())
        .with_username(body.user_name.unwrap_or_default());

    let content = body.text.unwrap_or_default();

    let metadata = PlatformMetadata::RocketChat {
        room_id: body.channel_id.unwrap_or_default(),
        message_id: body.message_id.clone().unwrap_or_default(),
        user_id: user_id.clone(),
    };

    Ok(NormalizedMessage::new(
        body.message_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        Platform::RocketChat,
        auth_user,
        content,
        request.received_at,
        metadata,
    ))
}

fn parse_signal_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    struct SignalEnvelope {
        source: Option<String>,
        #[serde(rename = "sourceNumber")]
        source_number: Option<String>,
        #[serde(rename = "sourceName")]
        source_name: Option<String>,
        timestamp: Option<u64>,
        #[serde(rename = "dataMessage")]
        data_message: Option<SignalDataMessage>,
    }

    #[derive(Deserialize)]
    struct SignalDataMessage {
        message: Option<String>,
        timestamp: Option<u64>,
        #[serde(rename = "groupInfo")]
        group_info: Option<SignalGroupInfo>,
    }

    #[derive(Deserialize)]
    struct SignalGroupInfo {
        #[serde(rename = "groupId")]
        group_id: Option<String>,
    }

    let envelope: SignalEnvelope = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("Signal parse error: {e}")))?;

    let source = envelope
        .source
        .or(envelope.source_number)
        .unwrap_or_default();

    let auth_user =
        AuthenticatedUser::new(&source).with_display_name(envelope.source_name.unwrap_or_default());

    let content = envelope
        .data_message
        .as_ref()
        .and_then(|dm| dm.message.clone())
        .unwrap_or_default();

    let group_id = envelope
        .data_message
        .as_ref()
        .and_then(|dm| dm.group_info.as_ref())
        .and_then(|gi| gi.group_id.clone());

    let timestamp = envelope
        .data_message
        .as_ref()
        .and_then(|dm| dm.timestamp)
        .or(envelope.timestamp)
        .unwrap_or(0);

    let metadata = PlatformMetadata::Signal {
        group_id,
        timestamp,
    };

    Ok(NormalizedMessage::new(
        format!("signal_{}", timestamp),
        Platform::Signal,
        auth_user,
        content,
        request.received_at,
        metadata,
    ))
}

fn parse_whatsapp_body(request: &WebhookRequest) -> Result<NormalizedMessage> {
    #[derive(Deserialize)]
    struct WhatsAppPayload {
        entry: Vec<WhatsAppEntry>,
    }

    #[derive(Deserialize)]
    struct WhatsAppEntry {
        changes: Vec<WhatsAppChange>,
    }

    #[derive(Deserialize)]
    struct WhatsAppChange {
        value: WhatsAppValue,
    }

    #[derive(Deserialize)]
    struct WhatsAppValue {
        messages: Option<Vec<WhatsAppMessage>>,
        contacts: Option<Vec<WhatsAppContact>>,
        metadata: Option<WhatsAppMeta>,
    }

    #[derive(Deserialize)]
    struct WhatsAppMessage {
        id: String,
        from: String,
        text: Option<WhatsAppText>,
        r#type: String,
    }

    #[derive(Deserialize)]
    struct WhatsAppText {
        body: String,
    }

    #[derive(Deserialize)]
    struct WhatsAppContact {
        wa_id: String,
        profile_name: Option<String>,
    }

    #[derive(Deserialize)]
    struct WhatsAppMeta {
        display_phone_number: Option<String>,
    }

    let payload: WhatsAppPayload = serde_json::from_slice(&request.body)
        .map_err(|e| MessagingError::ParseError(format!("WhatsApp parse error: {e}")))?;

    let value = payload
        .entry
        .into_iter()
        .next()
        .and_then(|entry| {
            entry
                .changes
                .into_iter()
                .find(|c| c.value.messages.is_some())
                .map(|c| c.value)
        })
        .ok_or_else(|| {
            MessagingError::ParseError("No messages in WhatsApp webhook payload".into())
        })?;

    let msg = value
        .messages
        .and_then(|msgs| msgs.into_iter().next())
        .ok_or_else(|| {
            MessagingError::ParseError("Empty messages array in WhatsApp webhook".into())
        })?;

    if msg.r#type != "text" {
        return Err(MessagingError::ParseError(format!(
            "Unsupported WhatsApp message type: {}",
            msg.r#type
        )));
    }

    let display_name = value
        .contacts
        .and_then(|contacts| contacts.into_iter().find(|c| c.wa_id == msg.from))
        .and_then(|c| c.profile_name)
        .unwrap_or_default();

    let phone_number = value
        .metadata
        .and_then(|m| m.display_phone_number)
        .unwrap_or_default();

    let auth_user = AuthenticatedUser::new(&msg.from).with_display_name(display_name);

    let content = msg.text.map(|t| t.body).unwrap_or_default();

    let metadata = PlatformMetadata::WhatsApp {
        phone_number,
        message_id: msg.id.clone(),
        business_account_id: false,
    };

    Ok(NormalizedMessage::new(
        msg.id,
        Platform::WhatsApp,
        auth_user,
        content,
        request.received_at,
        metadata,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn slack_key(secret: &str) -> [u8; 32] {
        let mut key = [0u8; 32];
        let bytes = secret.as_bytes();
        let len = bytes.len().min(32);
        key[..len].copy_from_slice(&bytes[..len]);
        key
    }

    fn make_receiver() -> WebhookReceiver {
        let mut receiver = WebhookReceiver::new();
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "tg_secret_123".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::Discord(DiscordWebhookConfig {
                public_key_pem: "discord_pub_key".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::Matrix(MatrixWebhookConfig {
                access_token: "matrix_token".into(),
                homeserver_base_url: "https://matrix.org".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::Slack(SlackWebhookConfig {
                signing_secret: "slack_signing_secret".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::RocketChat(RocketChatWebhookConfig {
                token: "rc_token".into(),
                user_id: "rc_bot".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::Signal(SignalWebhookConfig {
                verification_token: "signal_token".into(),
            }),
            None,
        );
        receiver.register_platform(
            WebhookConfig::WhatsApp(WhatsAppWebhookConfig {
                verify_token: "wa_verify".into(),
                app_secret: "wa_secret".into(),
            }),
            None,
        );
        receiver
    }

    // -- Telegram verification tests --

    #[test]
    fn telegram_verify_via_query_param() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Telegram, vec![])
            .with_query_param("secret_token", "tg_secret_123");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn telegram_verify_via_header() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Telegram, vec![])
            .with_header("X-Telegram-Bot-Api-Secret-Token", "tg_secret_123");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn telegram_wrong_secret() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Telegram, vec![])
            .with_query_param("secret_token", "wrong");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    #[test]
    fn telegram_missing_credentials() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Telegram, vec![]);
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::MissingCredentials
        );
    }

    // -- Discord verification tests --

    #[test]
    fn discord_has_signature_headers() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Discord, vec![])
            .with_header("X-Signature-Ed25519", "abc123")
            .with_header("X-Signature-Timestamp", "1234567890");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    #[test]
    fn discord_missing_credentials() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Discord, vec![]);
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::MissingCredentials
        );
    }

    // -- Matrix verification tests --

    #[test]
    fn matrix_verify_bearer_token() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Matrix, vec![])
            .with_header("Authorization", "Bearer matrix_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn matrix_verify_raw_token() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Matrix, vec![])
            .with_header("Authorization", "matrix_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn matrix_wrong_token() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Matrix, vec![])
            .with_header("Authorization", "Bearer wrong_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    // -- Slack verification tests --

    #[test]
    fn slack_verify_signature() {
        let receiver = make_receiver();
        let body = b"payload=test";
        let timestamp = "1234567890";
        let base_string = format!("v0:{}:{}", timestamp, "payload=test");
        let computed =
            blake3::keyed_hash(&slack_key("slack_signing_secret"), base_string.as_bytes());
        let signature = format!("v0={}", computed.to_hex());

        let req = WebhookRequest::new(Platform::Slack, body.to_vec())
            .with_header("X-Slack-Request-Timestamp", timestamp)
            .with_header("X-Slack-Signature", &signature);
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn slack_wrong_signature() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Slack, b"{}".to_vec())
            .with_header("X-Slack-Request-Timestamp", "1234567890")
            .with_header("X-Slack-Signature", "v0=wrong");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    #[test]
    fn slack_missing_credentials() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Slack, vec![]);
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::MissingCredentials
        );
    }

    // -- Rocket.Chat verification tests --

    #[test]
    fn rocketchat_verify_via_header() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::RocketChat, vec![])
            .with_header("X-Rocket-Chat-Token", "rc_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn rocketchat_verify_via_query() {
        let receiver = make_receiver();
        let req =
            WebhookRequest::new(Platform::RocketChat, vec![]).with_query_param("token", "rc_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    // -- Signal verification tests --

    #[test]
    fn signal_verify() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Signal, vec![])
            .with_header("X-Signal-Token", "signal_token");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn signal_wrong_token() {
        let receiver = make_receiver();
        let req =
            WebhookRequest::new(Platform::Signal, vec![]).with_header("X-Signal-Token", "wrong");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    // -- WhatsApp verification tests --

    #[test]
    fn whatsapp_verify() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::WhatsApp, vec![])
            .with_query_param("hub.mode", "subscribe")
            .with_query_param("hub.verify_token", "wa_verify");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::Verified
        );
    }

    #[test]
    fn whatsapp_wrong_mode() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::WhatsApp, vec![])
            .with_query_param("hub.mode", "unsubscribe")
            .with_query_param("hub.verify_token", "wa_verify");
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::InvalidSignature
        );
    }

    // -- Unregistered platform --

    #[test]
    fn unregistered_platform() {
        let receiver = make_receiver();
        let req = WebhookRequest::new(Platform::Webhook, vec![]);
        assert_eq!(
            receiver.verify_signature(&req),
            VerificationResult::UnsupportedPlatform
        );
    }

    // -- Registration tests --

    #[test]
    fn register_and_list_platforms() {
        let receiver = make_receiver();
        let platforms = receiver.registered_platforms();
        assert!(platforms.contains(&Platform::Telegram));
        assert!(platforms.contains(&Platform::Discord));
        assert!(platforms.contains(&Platform::Slack));
        assert_eq!(platforms.len(), 7);
    }

    #[test]
    fn unregister_platform() {
        let mut receiver = make_receiver();
        assert!(receiver.is_registered(Platform::Telegram));
        receiver.unregister_platform(Platform::Telegram);
        assert!(!receiver.is_registered(Platform::Telegram));
    }

    // -- WebhookConfig platform test --

    #[test]
    fn webhook_config_platform() {
        let configs = vec![
            (
                WebhookConfig::Telegram(TelegramWebhookConfig {
                    secret_token: String::new(),
                }),
                Platform::Telegram,
            ),
            (
                WebhookConfig::Discord(DiscordWebhookConfig {
                    public_key_pem: String::new(),
                }),
                Platform::Discord,
            ),
            (
                WebhookConfig::Matrix(MatrixWebhookConfig {
                    access_token: String::new(),
                    homeserver_base_url: String::new(),
                }),
                Platform::Matrix,
            ),
            (
                WebhookConfig::Slack(SlackWebhookConfig {
                    signing_secret: String::new(),
                }),
                Platform::Slack,
            ),
            (
                WebhookConfig::RocketChat(RocketChatWebhookConfig {
                    token: String::new(),
                    user_id: String::new(),
                }),
                Platform::RocketChat,
            ),
            (
                WebhookConfig::Signal(SignalWebhookConfig {
                    verification_token: String::new(),
                }),
                Platform::Signal,
            ),
            (
                WebhookConfig::WhatsApp(WhatsAppWebhookConfig {
                    verify_token: String::new(),
                    app_secret: String::new(),
                }),
                Platform::WhatsApp,
            ),
        ];
        for (config, expected) in configs {
            assert_eq!(config.platform(), expected);
        }
    }

    // -- Telegram body parsing --

    #[test]
    fn parse_telegram_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "update_id": 42,
            "message": {
                "message_id": 100,
                "from": {
                    "id": 12345,
                    "is_bot": false,
                    "first_name": "Test",
                    "last_name": "User",
                    "username": "testuser"
                },
                "chat": { "id": 12345 },
                "text": "/clawd status",
                "date": 1700000000
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Telegram, body)
            .with_query_param("secret_token", "tg_secret_123");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::Telegram);
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.user.platform_user_id, "12345");
        assert_eq!(msg.user.username.as_deref(), Some("testuser"));
        assert_eq!(msg.id, "tg_42_100");
    }

    #[test]
    fn parse_telegram_no_message_fails() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "update_id": 42
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Telegram, body)
            .with_query_param("secret_token", "tg_secret_123");

        assert!(receiver.parse_webhook_body(&req).is_err());
    }

    // -- Discord body parsing --

    #[test]
    fn parse_discord_interaction() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "id": "disc_123",
            "type": 2,
            "data": {
                "id": "cmd_1",
                "name": "clawd",
                "options": [
                    { "name": "action", "value": "status" }
                ]
            },
            "member": {
                "user": {
                    "id": "user_1",
                    "username": "testuser",
                    "bot": false
                },
                "nick": "Test User"
            },
            "guild_id": "123456789",
            "channel_id": "987654321"
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Discord, body)
            .with_header("X-Signature-Ed25519", "sig")
            .with_header("X-Signature-Timestamp", "123");

        let result = receiver.parse_webhook_body(&req);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, MessagingError::AuthenticationFailed(_)));
    }

    // -- Matrix body parsing --

    #[test]
    fn parse_matrix_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "event_id": "$evt_abc",
            "type": "m.room.message",
            "room_id": "!room:matrix.org",
            "sender": "@testuser:matrix.org",
            "content": {
                "msgtype": "m.text",
                "body": "!clawd status"
            },
            "origin_server_ts": 1700000000000_i64
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Matrix, body)
            .with_header("Authorization", "Bearer matrix_token");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::Matrix);
        assert_eq!(msg.content, "!clawd status");
        assert_eq!(msg.user.platform_user_id, "testuser:matrix.org");
    }

    #[test]
    fn parse_matrix_unsupported_event_type() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "event_id": "$evt_abc",
            "type": "m.room.member",
            "room_id": "!room:matrix.org",
            "sender": "@testuser:matrix.org",
            "content": {}
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Matrix, body)
            .with_header("Authorization", "Bearer matrix_token");

        assert!(receiver.parse_webhook_body(&req).is_err());
    }

    // -- Slack body parsing --

    #[test]
    fn parse_slack_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "type": "event_callback",
            "team_id": "T12345",
            "event": {
                "type": "message",
                "user": "U12345",
                "text": "/clawd status",
                "ts": "1234567890.123456",
                "channel": "C12345",
                "thread_ts": "1234567890.000001"
            }
        })
        .to_string()
        .into_bytes();

        let timestamp = "1234567890";
        let base_string = format!("v0:{}:{}", timestamp, std::str::from_utf8(&body).unwrap());
        let computed =
            blake3::keyed_hash(&slack_key("slack_signing_secret"), base_string.as_bytes());
        let signature = format!("v0={}", computed.to_hex());

        let req = WebhookRequest::new(Platform::Slack, body)
            .with_header("X-Slack-Request-Timestamp", timestamp)
            .with_header("X-Slack-Signature", &signature);

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::Slack);
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.user.platform_user_id, "U12345");
    }

    #[test]
    fn parse_slack_url_verification_fails() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "type": "url_verification",
            "challenge": "challenge_token"
        })
        .to_string()
        .into_bytes();

        let timestamp = "1234567890";
        let base_string = format!("v0:{}:{}", timestamp, std::str::from_utf8(&body).unwrap());
        let computed =
            blake3::keyed_hash(&slack_key("slack_signing_secret"), base_string.as_bytes());
        let signature = format!("v0={}", computed.to_hex());

        let req = WebhookRequest::new(Platform::Slack, body)
            .with_header("X-Slack-Request-Timestamp", timestamp)
            .with_header("X-Slack-Signature", &signature);

        assert!(receiver.parse_webhook_body(&req).is_err());
    }

    // -- Rocket.Chat body parsing --

    #[test]
    fn parse_rocketchat_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "user_id": "rc_user_1",
            "user_name": "testuser",
            "channel_id": "room_1",
            "message_id": "msg_1",
            "text": "/clawd status"
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::RocketChat, body)
            .with_header("X-Rocket-Chat-Token", "rc_token");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::RocketChat);
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.user.platform_user_id, "rc_user_1");
        assert_eq!(msg.user.username.as_deref(), Some("testuser"));
    }

    // -- Signal body parsing --

    #[test]
    fn parse_signal_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "source": "+1234567890",
            "sourceNumber": "+1234567890",
            "sourceName": "Test User",
            "timestamp": 1700000000000_i64,
            "dataMessage": {
                "message": "/clawd status",
                "timestamp": 1700000000000_i64
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Signal, body)
            .with_header("X-Signal-Token", "signal_token");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::Signal);
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.user.platform_user_id, "+1234567890");
        assert_eq!(msg.user.display_name.as_deref(), Some("Test User"));
    }

    // -- WhatsApp body parsing --

    #[test]
    fn parse_whatsapp_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "id": "wamid_123",
                            "from": "1234567890",
                            "type": "text",
                            "text": { "body": "/clawd status" }
                        }],
                        "contacts": [{
                            "wa_id": "1234567890",
                            "profile_name": "Test User"
                        }],
                        "metadata": {
                            "display_phone_number": "0987654321"
                        }
                    }
                }]
            }]
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::WhatsApp, body)
            .with_query_param("hub.mode", "subscribe")
            .with_query_param("hub.verify_token", "wa_verify");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.platform, Platform::WhatsApp);
        assert_eq!(msg.content, "/clawd status");
        assert_eq!(msg.user.platform_user_id, "1234567890");
        assert_eq!(msg.user.display_name.as_deref(), Some("Test User"));
    }

    #[test]
    fn parse_whatsapp_unsupported_type() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "id": "wamid_123",
                            "from": "1234567890",
                            "type": "image"
                        }]
                    }
                }]
            }]
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::WhatsApp, body)
            .with_query_param("hub.mode", "subscribe")
            .with_query_param("hub.verify_token", "wa_verify");

        assert!(receiver.parse_webhook_body(&req).is_err());
    }

    // -- Auth failure on parse --

    #[test]
    fn parse_without_auth_returns_error() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "update_id": 42,
            "message": {
                "message_id": 100,
                "from": { "id": 1, "is_bot": false, "first_name": "T" },
                "chat": { "id": 1 },
                "text": "hello",
                "date": 1700000000
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Telegram, body);
        let result = receiver.parse_webhook_body(&req);
        assert!(matches!(
            result,
            Err(MessagingError::AuthenticationFailed(_))
        ));
    }

    // -- WebhookHeaders case-insensitive --

    #[test]
    fn headers_case_insensitive() {
        let mut headers = WebhookHeaders::new();
        headers.insert("Content-Type", "application/json");
        assert_eq!(
            headers.get_case_insensitive("content-type"),
            Some("application/json")
        );
        assert_eq!(
            headers.get_case_insensitive("CONTENT-TYPE"),
            Some("application/json")
        );
    }

    // -- WebhookRequest body_str --

    #[test]
    fn request_body_str() {
        let req = WebhookRequest::new(Platform::Telegram, b"hello world".to_vec());
        assert_eq!(req.body_str(), "hello world");
    }

    #[test]
    fn request_body_str_invalid_utf8() {
        let req = WebhookRequest::new(Platform::Telegram, vec![0xff, 0xfe]);
        assert_eq!(req.body_str(), "");
    }

    // -- Default --

    #[test]
    fn receiver_default() {
        let receiver = WebhookReceiver::default();
        assert_eq!(receiver.registered_platforms().len(), 0);
    }

    // -- Channel config storage --

    #[test]
    fn channel_config_storage() {
        let mut receiver = WebhookReceiver::new();
        let cc = ChannelConfig::new(Platform::Telegram);
        receiver.register_platform(
            WebhookConfig::Telegram(TelegramWebhookConfig {
                secret_token: "s".into(),
            }),
            Some(cc),
        );
        assert!(receiver.get_channel_config(Platform::Telegram).is_some());
        assert!(receiver.get_channel_config(Platform::Discord).is_none());
    }

    // -- Signal group message --

    #[test]
    fn parse_signal_group_message() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "source": "+1234567890",
            "sourceName": "Test User",
            "timestamp": 1700000000000_i64,
            "dataMessage": {
                "message": "hello group",
                "timestamp": 1700000000000_i64,
                "groupInfo": {
                    "groupId": "group_abc="
                }
            }
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::Signal, body)
            .with_header("X-Signal-Token", "signal_token");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.content, "hello group");
        assert!(matches!(
            &msg.metadata,
            PlatformMetadata::Signal {
                group_id: Some(gid),
                ..
            } if gid == "group_abc="
        ));
    }

    // -- WhatsApp no contacts --

    #[test]
    fn parse_whatsapp_no_contacts() {
        let receiver = make_receiver();
        let body = serde_json::json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "id": "wamid_456",
                            "from": "555",
                            "type": "text",
                            "text": { "body": "hi" }
                        }],
                        "metadata": {
                            "display_phone_number": "111"
                        }
                    }
                }]
            }]
        })
        .to_string()
        .into_bytes();

        let req = WebhookRequest::new(Platform::WhatsApp, body)
            .with_query_param("hub.mode", "subscribe")
            .with_query_param("hub.verify_token", "wa_verify");

        let msg = receiver.parse_webhook_body(&req).unwrap();
        assert_eq!(msg.user.display_name, Some("".to_string()));
    }
}
