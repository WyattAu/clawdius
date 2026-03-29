#![deny(unsafe_code)]

//! OAuth 2.0 Flows for Messaging Platforms
//!
//! Provides specialized OAuth 2.0 token management and authorization flows
//! for Slack and Discord, including bot token exchange, refresh, and revocation.
//!
//! This module extends the generic [`enterprise::sso::OAuthProvider`] with
//! platform-specific scopes, endpoints, and bot token flows.

use crate::messaging::types::{Platform, Result as MsgResult};
use parking_lot::RwLock;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, warn};

const SLACK_DEFAULT_SCOPES: &[&str] = &[
    "chat:write",
    "channels:history",
    "groups:history",
    "im:history",
    "app_mentions:read",
    "commands",
];

const DISCORD_DEFAULT_SCOPES: &[&str] = &["bot", "messages.read"];

const DISCORD_DEFAULT_PERMISSIONS: u64 = 274877975552;

// ---------------------------------------------------------------------------
// Platform-specific OAuth configurations
// ---------------------------------------------------------------------------

/// Platform-specific OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlatformOAuthConfig {
    /// Slack OAuth configuration
    Slack(SlackOAuthConfig),
    /// Discord OAuth configuration
    Discord(DiscordOAuthConfig),
}

impl PlatformOAuthConfig {
    /// Return the platform this config targets
    #[must_use]
    pub fn platform(&self) -> Platform {
        match self {
            Self::Slack(_) => Platform::Slack,
            Self::Discord(_) => Platform::Discord,
        }
    }

    /// Return the client ID
    #[must_use]
    pub fn client_id(&self) -> &str {
        match self {
            Self::Slack(c) => &c.client_id,
            Self::Discord(c) => &c.client_id,
        }
    }

    /// Return the client secret
    #[must_use]
    pub fn client_secret(&self) -> &str {
        match self {
            Self::Slack(c) => &c.client_secret,
            Self::Discord(c) => &c.client_secret,
        }
    }

    /// Return the redirect URI
    #[must_use]
    pub fn redirect_uri(&self) -> &str {
        match self {
            Self::Slack(c) => &c.redirect_uri,
            Self::Discord(c) => &c.redirect_uri,
        }
    }

    /// Return the scopes
    #[must_use]
    pub fn scopes(&self) -> &[String] {
        match self {
            Self::Slack(c) => &c.scopes,
            Self::Discord(c) => &c.scopes,
        }
    }

    /// Return the authorization endpoint URL
    #[must_use]
    pub fn auth_url(&self) -> &str {
        match self {
            Self::Slack(c) => &c.base_url_auth,
            Self::Discord(c) => &c.base_url_auth,
        }
    }

    /// Return the token endpoint URL
    #[must_use]
    pub fn token_url(&self) -> &str {
        match self {
            Self::Slack(c) => &c.base_url_token,
            Self::Discord(c) => &c.base_url_token,
        }
    }
}

/// Slack OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackOAuthConfig {
    /// OAuth client ID
    pub client_id: String,
    /// OAuth client secret
    pub client_secret: String,
    /// Redirect URI after authorization
    pub redirect_uri: String,
    /// Requested OAuth scopes
    pub scopes: Vec<String>,
    /// Bot token (obtained after OAuth flow)
    pub bot_token: Option<String>,
    /// User token (for user-authorized actions)
    pub user_token: Option<String>,
    /// Slack workspace ID
    pub team_id: Option<String>,
    /// Bot's Slack user ID
    pub bot_user_id: Option<String>,
    /// Overrideable auth endpoint (for testing)
    #[serde(default = "default_slack_auth_url")]
    pub base_url_auth: String,
    /// Overrideable token endpoint (for testing)
    #[serde(default = "default_slack_token_url")]
    pub base_url_token: String,
}

fn default_slack_auth_url() -> String {
    "https://slack.com/oauth/v2/authorize".to_string()
}

fn default_slack_token_url() -> String {
    "https://slack.com/api/oauth.v2.access".to_string()
}

impl SlackOAuthConfig {
    /// Create a new Slack OAuth config with default scopes
    #[must_use]
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
            scopes: SLACK_DEFAULT_SCOPES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            bot_token: None,
            user_token: None,
            team_id: None,
            bot_user_id: None,
            base_url_auth: default_slack_auth_url(),
            base_url_token: default_slack_token_url(),
        }
    }

    /// Set the bot token (typically obtained after OAuth exchange)
    pub fn with_bot_token(mut self, token: impl Into<String>) -> Self {
        self.bot_token = Some(token.into());
        self
    }

    /// Set the user token
    pub fn with_user_token(mut self, token: impl Into<String>) -> Self {
        self.user_token = Some(token.into());
        self
    }

    /// Set the team (workspace) ID
    pub fn with_team_id(mut self, team_id: impl Into<String>) -> Self {
        self.team_id = Some(team_id.into());
        self
    }

    /// Set the bot user ID
    pub fn with_bot_user_id(mut self, bot_user_id: impl Into<String>) -> Self {
        self.bot_user_id = Some(bot_user_id.into());
        self
    }

    /// Override the auth URL (useful for testing)
    pub fn with_auth_url(mut self, url: impl Into<String>) -> Self {
        self.base_url_auth = url.into();
        self
    }

    /// Override the token URL (useful for testing)
    pub fn with_token_url(mut self, url: impl Into<String>) -> Self {
        self.base_url_token = url.into();
        self
    }
}

/// Discord OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordOAuthConfig {
    /// OAuth client ID
    pub client_id: String,
    /// OAuth client secret
    pub client_secret: String,
    /// Redirect URI after authorization
    pub redirect_uri: String,
    /// Requested OAuth scopes
    pub scopes: Vec<String>,
    /// Bot token (obtained after OAuth flow)
    pub bot_token: Option<String>,
    /// Discord server (guild) ID
    pub guild_id: Option<String>,
    /// Bot permissions bitmask
    pub permissions: u64,
    /// Overrideable auth endpoint (for testing)
    #[serde(default = "default_discord_auth_url")]
    pub base_url_auth: String,
    /// Overrideable token endpoint (for testing)
    #[serde(default = "default_discord_token_url")]
    pub base_url_token: String,
}

fn default_discord_auth_url() -> String {
    "https://discord.com/api/oauth2/authorize".to_string()
}

fn default_discord_token_url() -> String {
    "https://discord.com/api/oauth2/token".to_string()
}

impl DiscordOAuthConfig {
    /// Create a new Discord OAuth config with default scopes and permissions
    #[must_use]
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
            scopes: DISCORD_DEFAULT_SCOPES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            bot_token: None,
            guild_id: None,
            permissions: DISCORD_DEFAULT_PERMISSIONS,
            base_url_auth: default_discord_auth_url(),
            base_url_token: default_discord_token_url(),
        }
    }

    /// Set the bot token
    pub fn with_bot_token(mut self, token: impl Into<String>) -> Self {
        self.bot_token = Some(token.into());
        self
    }

    /// Set the guild (server) ID
    pub fn with_guild_id(mut self, guild_id: impl Into<String>) -> Self {
        self.guild_id = Some(guild_id.into());
        self
    }

    /// Set bot permissions
    pub fn with_permissions(mut self, permissions: u64) -> Self {
        self.permissions = permissions;
        self
    }

    /// Override the auth URL (useful for testing)
    pub fn with_auth_url(mut self, url: impl Into<String>) -> Self {
        self.base_url_auth = url.into();
        self
    }

    /// Override the token URL (useful for testing)
    pub fn with_token_url(mut self, url: impl Into<String>) -> Self {
        self.base_url_token = url.into();
        self
    }
}

// ---------------------------------------------------------------------------
// OAuthToken
// ---------------------------------------------------------------------------

/// Represents an OAuth token set returned by a messaging platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// Bot / access token
    pub bot_token: String,
    /// User token (Slack-specific, may be present)
    pub user_token: Option<String>,
    /// Refresh token (for long-lived access)
    pub refresh_token: Option<String>,
    /// Expiration as UNIX timestamp (seconds), `None` = never expires
    pub expires_at: Option<u64>,
    /// Scopes granted
    pub scopes: Vec<String>,
    /// Extra platform-specific fields
    pub extra: HashMap<String, String>,
}

impl OAuthToken {
    /// Check whether this token is expired (or about to expire within the
    /// given buffer).
    #[must_use]
    pub fn is_expired(&self, buffer_secs: u64) -> bool {
        match self.expires_at {
            None => false,
            Some(expires) => {
                let now_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                now_secs + buffer_secs >= expires
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OAuthTokenStore
// ---------------------------------------------------------------------------

/// Storage backend for [`OAuthToken`] instances
enum StoreBackend {
    Memory(RwLock<HashMap<String, OAuthToken>>),
    Sqlite {
        conn: Arc<parking_lot::Mutex<rusqlite::Connection>>,
    },
}

/// Manages OAuth tokens for messaging platforms
pub struct OAuthTokenStore {
    backend: StoreBackend,
}

impl OAuthTokenStore {
    /// Create an in-memory token store
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: StoreBackend::Memory(RwLock::new(HashMap::new())),
        }
    }

    /// Create a SQLite-backed token store
    pub fn with_persistence(
        db_path: impl AsRef<Path>,
    ) -> std::result::Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS oauth_tokens (
                platform TEXT PRIMARY KEY,
                bot_token TEXT,
                user_token TEXT,
                refresh_token TEXT,
                expires_at INTEGER,
                scopes TEXT,
                extra TEXT,
                created_at INTEGER,
                updated_at INTEGER
            );",
        )?;
        Ok(Self {
            backend: StoreBackend::Sqlite {
                conn: Arc::new(parking_lot::Mutex::new(conn)),
            },
        })
    }

    /// Store a bot token for the given platform
    pub fn store_bot_token(
        &self,
        platform: &Platform,
        token: OAuthToken,
    ) -> std::result::Result<(), crate::messaging::types::MessagingError> {
        let key = platform.as_str().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        debug!("Stored bot token for platform: {}", key);
        match &self.backend {
            StoreBackend::Memory(map) => {
                map.write().insert(key, token);
            }
            StoreBackend::Sqlite { conn } => {
                let scopes_json = serde_json::to_string(&token.scopes).unwrap_or_default();
                let extra_json = serde_json::to_string(&token.extra).unwrap_or_default();
                let expires = token.expires_at.map(|v| v as i64).unwrap_or(0);
                let conn = conn.lock();
                conn.execute(
                    "INSERT INTO oauth_tokens (platform, bot_token, user_token, refresh_token, expires_at, scopes, extra, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT(platform) DO UPDATE SET
                        bot_token = excluded.bot_token,
                        user_token = excluded.user_token,
                        refresh_token = excluded.refresh_token,
                        expires_at = excluded.expires_at,
                        scopes = excluded.scopes,
                        extra = excluded.extra,
                        updated_at = excluded.updated_at",
                    params![
                        key,
                        token.bot_token,
                        token.user_token,
                        token.refresh_token,
                        expires,
                        scopes_json,
                        extra_json,
                        now,
                        now
                    ],
                ).map_err(|e| {
                    let msg = format!("SQLite insert failed for {key}: {e}");
                    error!("{msg}");
                    crate::messaging::types::MessagingError::InvalidConfig(msg)
                })?;
            }
        }
        Ok(())
    }

    /// Retrieve the bot token for the given platform
    pub fn get_bot_token(
        &self,
        platform: &Platform,
    ) -> std::result::Result<Option<OAuthToken>, crate::messaging::types::MessagingError> {
        let key = platform.as_str();
        match &self.backend {
            StoreBackend::Memory(map) => Ok(map.read().get(key).cloned()),
            StoreBackend::Sqlite { conn } => {
                let conn = conn.lock();
                let mut stmt = conn
                    .prepare(
                        "SELECT bot_token, user_token, refresh_token, expires_at, scopes, extra
                         FROM oauth_tokens WHERE platform = ?1",
                    )
                    .map_err(|e| {
                        let msg = format!("SQLite prepare failed: {e}");
                        error!("{msg}");
                        crate::messaging::types::MessagingError::InvalidConfig(msg)
                    })?;
                let result = stmt.query_row(params![key], |row| {
                    let scopes_str: String = row.get(4)?;
                    let extra_str: String = row.get(5)?;
                    Ok(OAuthToken {
                        bot_token: row.get(0)?,
                        user_token: row.get(1)?,
                        refresh_token: row.get(2)?,
                        expires_at: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                        scopes: serde_json::from_str(&scopes_str).unwrap_or_default(),
                        extra: serde_json::from_str(&extra_str).unwrap_or_default(),
                    })
                });
                match result {
                    Ok(token) => Ok(Some(token)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => {
                        let msg = format!("SQLite query failed: {e}");
                        error!("{msg}");
                        Err(crate::messaging::types::MessagingError::InvalidConfig(msg))
                    }
                }
            }
        }
    }

    /// Refresh the token for the given platform if expired
    pub fn refresh_token(
        &self,
        platform: &Platform,
    ) -> std::result::Result<Option<OAuthToken>, crate::messaging::types::MessagingError> {
        let current = match self.get_bot_token(platform)? {
            Some(t) => t,
            None => return Ok(None),
        };
        if !current.is_expired(60) {
            return Ok(Some(current));
        }
        warn!(
            "Token for platform {} is expired or expiring soon, needs refresh",
            platform.as_str()
        );
        Ok(None)
    }

    /// Revoke (delete) the token for the given platform
    pub fn revoke_token(
        &self,
        platform: &Platform,
    ) -> std::result::Result<(), crate::messaging::types::MessagingError> {
        let key = platform.as_str().to_string();
        match &self.backend {
            StoreBackend::Memory(map) => {
                map.write().remove(&key);
            }
            StoreBackend::Sqlite { conn } => {
                let conn = conn.lock();
                conn.execute("DELETE FROM oauth_tokens WHERE platform = ?1", params![key])
                    .map_err(|e| {
                        let msg = format!("SQLite delete failed: {e}");
                        error!("{msg}");
                        crate::messaging::types::MessagingError::InvalidConfig(msg)
                    })?;
            }
        }
        debug!("Revoked token for platform: {key}");
        Ok(())
    }

    /// Check whether a platform has a configured token
    #[must_use]
    pub fn is_configured(&self, platform: &Platform) -> bool {
        self.get_bot_token(platform)
            .map(|t| t.is_some())
            .unwrap_or(false)
    }
}

impl Default for OAuthTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Slack token response (from /api/oauth.v2.access)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SlackTokenResponse {
    ok: bool,
    access_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    bot_user_id: Option<String>,
    app_id: Option<String>,
    team: Option<SlackTeamInfo>,
    authed_user: Option<SlackAuthedUser>,
    error: Option<String>,
    scope: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackTeamInfo {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackAuthedUser {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

// ---------------------------------------------------------------------------
// Discord token response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: String,
    guild_id: Option<String>,
}

// ---------------------------------------------------------------------------
// PlatformOAuthClient
// ---------------------------------------------------------------------------

/// Handles OAuth authorization flows for messaging platforms
pub struct PlatformOAuthClient {
    config: PlatformOAuthConfig,
    token_store: OAuthTokenStore,
    http: reqwest::Client,
}

impl PlatformOAuthClient {
    /// Create a new OAuth client with the given config and token store
    #[must_use]
    pub fn new(config: PlatformOAuthConfig, token_store: OAuthTokenStore) -> Self {
        Self {
            config,
            token_store,
            http: reqwest::Client::new(),
        }
    }

    /// Generate the OAuth authorization URL for redirecting the user
    pub fn authorization_url(&self, _platform: &Platform) -> String {
        let config = &self.config;
        let scopes = config.scopes().join(",");
        let state = uuid::Uuid::new_v4().to_string();

        match config {
            PlatformOAuthConfig::Slack(c) => {
                let mut url = format!(
                    "{}?client_id={}&scope={}&redirect_uri={}&state={}",
                    c.base_url_auth,
                    c.client_id,
                    scopes,
                    urlencoding::encode(&c.redirect_uri),
                    state,
                );
                if let Some(team) = &c.team_id {
                    url.push_str(&format!("&team={}", urlencoding::encode(team)));
                }
                url
            }
            PlatformOAuthConfig::Discord(c) => {
                let mut url = format!(
                    "{}?client_id={}&scope={}&redirect_uri={}&response_type=code&state={}&permissions={}",
                    c.base_url_auth,
                    c.client_id,
                    scopes,
                    urlencoding::encode(&c.redirect_uri),
                    state,
                    c.permissions,
                );
                if let Some(guild) = &c.guild_id {
                    url.push_str(&format!("&guild_id={}", urlencoding::encode(guild)));
                }
                url
            }
        }
    }

    /// Exchange an authorization code for tokens
    pub async fn exchange_code(&self, _platform: &Platform, code: &str) -> MsgResult<OAuthToken> {
        let config = &self.config;
        match config {
            PlatformOAuthConfig::Slack(c) => self.exchange_slack_code(c, code).await,
            PlatformOAuthConfig::Discord(c) => self.exchange_discord_code(c, code).await,
        }
    }

    async fn exchange_slack_code(
        &self,
        config: &SlackOAuthConfig,
        code: &str,
    ) -> MsgResult<OAuthToken> {
        let resp = self
            .http
            .post(&config.base_url_token)
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Slack token request failed: {e}");
                error!("{msg}");
                crate::messaging::types::MessagingError::AuthenticationFailed(msg)
            })?;

        let body: SlackTokenResponse = resp.json().await.map_err(|e| {
            let msg = format!("Failed to parse Slack token response: {e}");
            error!("{msg}");
            crate::messaging::types::MessagingError::ParseError(msg)
        })?;

        if !body.ok {
            let err_msg = body.error.unwrap_or_else(|| "unknown error".to_string());
            let msg = format!("Slack OAuth error: {err_msg}");
            error!("{msg}");
            return Err(crate::messaging::types::MessagingError::AuthenticationFailed(msg));
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut extra = HashMap::new();
        if let Some(bot_user_id) = &body.bot_user_id {
            extra.insert("bot_user_id".to_string(), bot_user_id.clone());
        }
        if let Some(app_id) = &body.app_id {
            extra.insert("app_id".to_string(), app_id.clone());
        }
        if let Some(team) = &body.team {
            if let Some(team_id) = &team.id {
                extra.insert("team_id".to_string(), team_id.clone());
            }
        }

        let bot_token = body.access_token.unwrap_or_default();
        let scopes: Vec<String> = body
            .scope
            .unwrap_or_default()
            .split(',')
            .map(|s| s.to_string())
            .collect();
        let expires_in = body.expires_in;
        let refresh_token = body.refresh_token;

        let user_token = body.authed_user.as_ref().and_then(|u| {
            u.access_token.clone().or_else(|| {
                if u.refresh_token.is_some() || u.expires_in.is_some() {
                    None
                } else {
                    None
                }
            })
        });

        let token = OAuthToken {
            bot_token,
            user_token,
            refresh_token,
            expires_at: expires_in.map(|e| now_secs + e),
            scopes,
            extra,
        };

        self.token_store
            .store_bot_token(&Platform::Slack, token.clone())?;

        Ok(token)
    }

    async fn exchange_discord_code(
        &self,
        config: &DiscordOAuthConfig,
        code: &str,
    ) -> MsgResult<OAuthToken> {
        let resp = self
            .http
            .post(&config.base_url_token)
            .basic_auth(&config.client_id, Some(&config.client_secret))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Discord token request failed: {e}");
                error!("{msg}");
                crate::messaging::types::MessagingError::AuthenticationFailed(msg)
            })?;

        let body: DiscordTokenResponse = resp.json().await.map_err(|e| {
            let msg = format!("Failed to parse Discord token response: {e}");
            error!("{msg}");
            crate::messaging::types::MessagingError::ParseError(msg)
        })?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut extra = HashMap::new();
        if let Some(guild_id) = &body.guild_id {
            extra.insert("guild_id".to_string(), guild_id.clone());
        }

        let scopes: Vec<String> = body.scope.split(',').map(|s| s.to_string()).collect();

        let token = OAuthToken {
            bot_token: body.access_token,
            user_token: None,
            refresh_token: body.refresh_token,
            expires_at: Some(now_secs + body.expires_in),
            scopes,
            extra,
        };

        self.token_store
            .store_bot_token(&Platform::Discord, token.clone())?;

        Ok(token)
    }

    /// Refresh an expired access token
    pub async fn refresh_access_token(&self, platform: &Platform) -> MsgResult<OAuthToken> {
        let current = self.token_store.get_bot_token(platform)?.ok_or_else(|| {
            let msg = format!("No token stored for platform {}", platform.as_str());
            crate::messaging::types::MessagingError::AuthenticationFailed(msg)
        })?;

        let refresh = current.refresh_token.ok_or_else(|| {
            let msg = format!(
                "No refresh token available for platform {}",
                platform.as_str()
            );
            crate::messaging::types::MessagingError::AuthenticationFailed(msg)
        })?;

        match &self.config {
            PlatformOAuthConfig::Slack(c) => self.refresh_slack_token(c, &refresh).await,
            PlatformOAuthConfig::Discord(c) => self.refresh_discord_token(c, &refresh).await,
        }
    }

    async fn refresh_slack_token(
        &self,
        config: &SlackOAuthConfig,
        refresh_token: &str,
    ) -> MsgResult<OAuthToken> {
        let resp = self
            .http
            .post(&config.base_url_token)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Slack token refresh failed: {e}");
                error!("{msg}");
                crate::messaging::types::MessagingError::AuthenticationFailed(msg)
            })?;

        let body: SlackTokenResponse = resp.json().await.map_err(|e| {
            let msg = format!("Failed to parse Slack refresh response: {e}");
            error!("{msg}");
            crate::messaging::types::MessagingError::ParseError(msg)
        })?;

        if !body.ok {
            let err_msg = body.error.unwrap_or_else(|| "unknown error".to_string());
            let msg = format!("Slack token refresh error: {err_msg}");
            error!("{msg}");
            return Err(crate::messaging::types::MessagingError::AuthenticationFailed(msg));
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let token = OAuthToken {
            bot_token: body.access_token.unwrap_or_default(),
            user_token: body
                .authed_user
                .as_ref()
                .and_then(|u| u.access_token.clone()),
            refresh_token: body
                .refresh_token
                .or_else(|| Some(refresh_token.to_string())),
            expires_at: body.expires_in.map(|e| now_secs + e),
            scopes: body
                .scope
                .unwrap_or_default()
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            extra: HashMap::new(),
        };

        self.token_store
            .store_bot_token(&Platform::Slack, token.clone())?;

        Ok(token)
    }

    async fn refresh_discord_token(
        &self,
        config: &DiscordOAuthConfig,
        refresh_token: &str,
    ) -> MsgResult<OAuthToken> {
        let resp = self
            .http
            .post(&config.base_url_token)
            .basic_auth(&config.client_id, Some(&config.client_secret))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| {
                let msg = format!("Discord token refresh failed: {e}");
                error!("{msg}");
                crate::messaging::types::MessagingError::AuthenticationFailed(msg)
            })?;

        let body: DiscordTokenResponse = resp.json().await.map_err(|e| {
            let msg = format!("Failed to parse Discord refresh response: {e}");
            error!("{msg}");
            crate::messaging::types::MessagingError::ParseError(msg)
        })?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut extra = HashMap::new();
        if let Some(guild_id) = &body.guild_id {
            extra.insert("guild_id".to_string(), guild_id.clone());
        }

        let token = OAuthToken {
            bot_token: body.access_token,
            user_token: None,
            refresh_token: body
                .refresh_token
                .or_else(|| Some(refresh_token.to_string())),
            expires_at: Some(now_secs + body.expires_in),
            scopes: body.scope.split(',').map(|s| s.to_string()).collect(),
            extra,
        };

        self.token_store
            .store_bot_token(&Platform::Discord, token.clone())?;

        Ok(token)
    }

    /// Revoke the access token for the given platform
    pub async fn revoke_access_token(&self, platform: &Platform) -> MsgResult<()> {
        self.token_store.revoke_token(platform)?;
        debug!("Revoked access token for platform: {}", platform.as_str());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn make_slack_token() -> OAuthToken {
        OAuthToken {
            bot_token: "xoxb-test-bot-token".to_string(),
            user_token: Some("xoxp-test-user-token".to_string()),
            refresh_token: Some("test-refresh-token".to_string()),
            expires_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() + 3600)
                    .unwrap_or(0),
            ),
            scopes: vec!["chat:write".to_string()],
            extra: HashMap::new(),
        }
    }

    fn make_expired_token() -> OAuthToken {
        OAuthToken {
            bot_token: "xoxb-expired".to_string(),
            user_token: None,
            refresh_token: Some("expired-refresh".to_string()),
            expires_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() - 100)
                    .unwrap_or(0),
            ),
            scopes: vec!["chat:write".to_string()],
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_slack_default_scopes() {
        let config = SlackOAuthConfig::new("cid", "secret", "http://localhost/callback");
        assert_eq!(config.scopes.len(), SLACK_DEFAULT_SCOPES.len());
        assert!(config.scopes.contains(&"chat:write".to_string()));
        assert!(config.scopes.contains(&"app_mentions:read".to_string()));
    }

    #[test]
    fn test_discord_default_scopes() {
        let config = DiscordOAuthConfig::new("cid", "secret", "http://localhost/callback");
        assert_eq!(config.scopes.len(), DISCORD_DEFAULT_SCOPES.len());
        assert!(config.scopes.contains(&"bot".to_string()));
        assert!(config.scopes.contains(&"messages.read".to_string()));
    }

    #[test]
    fn test_discord_default_permissions() {
        let config = DiscordOAuthConfig::new("cid", "secret", "http://localhost/callback");
        assert_eq!(config.permissions, DISCORD_DEFAULT_PERMISSIONS);
    }

    #[test]
    fn test_slack_config_builder() {
        let config =
            SlackOAuthConfig::new("my-client-id", "my-secret", "https://example.com/callback")
                .with_bot_token("xoxb-123")
                .with_user_token("xoxp-456")
                .with_team_id("T12345")
                .with_bot_user_id("U12345");

        assert_eq!(config.client_id, "my-client-id");
        assert_eq!(config.bot_token.as_deref(), Some("xoxb-123"));
        assert_eq!(config.user_token.as_deref(), Some("xoxp-456"));
        assert_eq!(config.team_id.as_deref(), Some("T12345"));
        assert_eq!(config.bot_user_id.as_deref(), Some("U12345"));
    }

    #[test]
    fn test_discord_config_builder() {
        let config =
            DiscordOAuthConfig::new("my-client-id", "my-secret", "https://example.com/callback")
                .with_bot_token("test-bot-token")
                .with_guild_id("G12345")
                .with_permissions(8);

        assert_eq!(config.client_id, "my-client-id");
        assert_eq!(config.bot_token.as_deref(), Some("test-bot-token"));
        assert_eq!(config.guild_id.as_deref(), Some("G12345"));
        assert_eq!(config.permissions, 8);
    }

    #[test]
    fn test_slack_authorization_url() {
        let config = SlackOAuthConfig::new(
            "slack-client-id",
            "slack-secret",
            "http://localhost:8080/callback",
        )
        .with_team_id("T99999");
        let platform_config = PlatformOAuthConfig::Slack(config);
        let store = OAuthTokenStore::new();
        let client = PlatformOAuthClient::new(platform_config, store);
        let url = client.authorization_url(&Platform::Slack);

        assert!(url.starts_with("https://slack.com/oauth/v2/authorize"));
        assert!(url.contains("client_id=slack-client-id"));
        assert!(url.contains("scope="));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("team=T99999"));
        assert!(url.contains("state="));
    }

    #[test]
    fn test_discord_authorization_url() {
        let config = DiscordOAuthConfig::new(
            "discord-client-id",
            "discord-secret",
            "http://localhost:8080/callback",
        )
        .with_guild_id("G99999");
        let platform_config = PlatformOAuthConfig::Discord(config);
        let store = OAuthTokenStore::new();
        let client = PlatformOAuthClient::new(platform_config, store);
        let url = client.authorization_url(&Platform::Discord);

        assert!(url.starts_with("https://discord.com/api/oauth2/authorize"));
        assert!(url.contains("client_id=discord-client-id"));
        assert!(url.contains("scope="));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("permissions=274877975552"));
        assert!(url.contains("guild_id=G99999"));
        assert!(url.contains("state="));
    }

    #[test]
    fn test_token_store_retrieve_round_trip() {
        let store = OAuthTokenStore::new();
        let token = make_slack_token();

        store
            .store_bot_token(&Platform::Slack, token.clone())
            .unwrap_or_else(|e| panic!("store failed: {e}"));

        let retrieved = store
            .get_bot_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("get failed: {e}"));

        let retrieved = retrieved.expect("token should exist");
        assert_eq!(retrieved.bot_token, token.bot_token);
        assert_eq!(retrieved.user_token, token.user_token);
        assert_eq!(retrieved.refresh_token, token.refresh_token);
        assert_eq!(retrieved.scopes, token.scopes);
    }

    #[test]
    fn test_token_expiration_check() {
        let valid = make_slack_token();
        assert!(!valid.is_expired(60));

        let expired = make_expired_token();
        assert!(expired.is_expired(0));
    }

    #[test]
    fn test_revoke_removes_token() {
        let store = OAuthTokenStore::new();
        let token = make_slack_token();

        store
            .store_bot_token(&Platform::Slack, token)
            .unwrap_or_else(|e| panic!("store failed: {e}"));
        assert!(store.is_configured(&Platform::Slack));

        store
            .revoke_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("revoke failed: {e}"));
        assert!(!store.is_configured(&Platform::Slack));

        let retrieved = store
            .get_bot_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("get failed: {e}"));
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_is_configured() {
        let store = OAuthTokenStore::new();
        assert!(!store.is_configured(&Platform::Slack));
        assert!(!store.is_configured(&Platform::Discord));

        store
            .store_bot_token(&Platform::Slack, make_slack_token())
            .unwrap_or_else(|e| panic!("store failed: {e}"));

        assert!(store.is_configured(&Platform::Slack));
        assert!(!store.is_configured(&Platform::Discord));
    }

    #[test]
    fn test_sqlite_persistence() {
        let tmp = NamedTempFile::new().unwrap_or_else(|e| panic!("tempfile: {e}"));
        let path = tmp.path().to_path_buf();

        let store =
            OAuthTokenStore::with_persistence(&path).unwrap_or_else(|e| panic!("open: {e}"));

        let token = make_slack_token();
        store
            .store_bot_token(&Platform::Slack, token.clone())
            .unwrap_or_else(|e| panic!("store: {e}"));

        drop(store);

        let store2 =
            OAuthTokenStore::with_persistence(&path).unwrap_or_else(|e| panic!("reopen: {e}"));
        let retrieved = store2
            .get_bot_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("get: {e}"));

        let retrieved = retrieved.expect("token should persist across reopen");
        assert_eq!(retrieved.bot_token, token.bot_token);
        assert_eq!(retrieved.user_token, token.user_token);
        assert_eq!(retrieved.refresh_token, token.refresh_token);
        assert_eq!(retrieved.scopes, token.scopes);
    }

    #[test]
    fn test_refresh_token_expired_returns_none() {
        let store = OAuthTokenStore::new();
        let expired = make_expired_token();
        store
            .store_bot_token(&Platform::Slack, expired)
            .unwrap_or_else(|e| panic!("store: {e}"));

        let result = store
            .refresh_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("refresh: {e}"));
        assert!(result.is_none());
    }

    #[test]
    fn test_refresh_token_valid_returns_some() {
        let store = OAuthTokenStore::new();
        let valid = make_slack_token();
        store
            .store_bot_token(&Platform::Slack, valid)
            .unwrap_or_else(|e| panic!("store: {e}"));

        let result = store
            .refresh_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("refresh: {e}"));
        assert!(result.is_some());
    }

    #[test]
    fn test_get_bot_token_returns_expired_token_with_refresh() {
        let store = OAuthTokenStore::new();
        let expired = make_expired_token();
        assert!(expired.refresh_token.is_some());
        store
            .store_bot_token(&Platform::Slack, expired.clone())
            .unwrap_or_else(|e| panic!("store: {e}"));

        let result = store
            .get_bot_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("get: {e}"));

        let token = result.expect("expired token should still be returned");
        assert_eq!(token.bot_token, "xoxb-expired");
        assert!(token.is_expired(0));
        assert_eq!(token.refresh_token.as_deref(), Some("expired-refresh"));
    }

    #[test]
    fn test_refresh_token_returns_none_for_expired_with_refresh() {
        let store = OAuthTokenStore::new();
        let expired = make_expired_token();
        assert!(expired.refresh_token.is_some());
        store
            .store_bot_token(&Platform::Slack, expired)
            .unwrap_or_else(|e| panic!("store: {e}"));

        let result = store
            .refresh_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("refresh: {e}"));

        assert!(
            result.is_none(),
            "refresh_token() should return None for expired tokens even when refresh_token is present"
        );
    }

    #[test]
    fn test_refresh_token_returns_none_for_no_token() {
        let store = OAuthTokenStore::new();
        let result = store
            .refresh_token(&Platform::Slack)
            .unwrap_or_else(|e| panic!("refresh: {e}"));
        assert!(result.is_none());
    }

    #[test]
    fn test_platform_oauth_config_accessors() {
        let slack_config = SlackOAuthConfig::new("s-cid", "s-sec", "http://slack/cb")
            .with_auth_url("http://slack-auth")
            .with_token_url("http://slack-token");
        let config = PlatformOAuthConfig::Slack(slack_config);

        assert_eq!(config.platform(), Platform::Slack);
        assert_eq!(config.client_id(), "s-cid");
        assert_eq!(config.client_secret(), "s-sec");
        assert_eq!(config.redirect_uri(), "http://slack/cb");
        assert_eq!(config.auth_url(), "http://slack-auth");
        assert_eq!(config.token_url(), "http://slack-token");
        assert!(!config.scopes().is_empty());

        let discord_config = DiscordOAuthConfig::new("d-cid", "d-sec", "http://discord/cb")
            .with_auth_url("http://discord-auth")
            .with_token_url("http://discord-token");
        let config = PlatformOAuthConfig::Discord(discord_config);

        assert_eq!(config.platform(), Platform::Discord);
        assert_eq!(config.client_id(), "d-cid");
        assert_eq!(config.client_secret(), "d-sec");
        assert_eq!(config.redirect_uri(), "http://discord/cb");
        assert_eq!(config.auth_url(), "http://discord-auth");
        assert_eq!(config.token_url(), "http://discord-token");
    }
}
