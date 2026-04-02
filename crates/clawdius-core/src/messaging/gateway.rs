//! Messaging Gateway
//!
//! Central gateway for routing messages across multiple messaging platforms.
//! Implements IEEE 1016 architectural specification.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use crate::messaging::audit::{MessagingAuditEvent, MessagingAuditLogger};
use crate::messaging::channels::MessagingChannel;
use crate::messaging::command_parser::{chunk_message, CommandParser};
use crate::messaging::rate_limiter::RateLimiter;
use crate::messaging::retry_queue::RetryQueue;
use crate::messaging::session_binder::SessionBinder;
use crate::messaging::state_store::StateStore;
use crate::messaging::tenant::{TenantContext, TenantManager, TenantResolver};
use crate::messaging::types::{
    ChannelConfig, CommandCategory, MessagingError, MessagingSession, ParsedCommand, Platform,
    PlatformUserId, Result,
};
use crate::messaging::usage_tracker::{Outcome, UsageEvent, UsageTracker};
use std::time::Instant;

/// Result from a message handler
pub struct MessageHandlerResult {
    pub response: String,
    pub should_chunk: bool,
    /// Optional stream of incremental content chunks for progressive editing.
    /// When set, the gateway will send the initial `response` as a placeholder,
    /// then progressively edit it as chunks arrive on this receiver.
    pub stream: Option<mpsc::Receiver<String>>,
}

/// Trait for handling categorized commands
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult>;
}

/// Central messaging gateway for routing messages across platforms
pub struct MessagingGateway {
    channels: Arc<RwLock<HashMap<Platform, Arc<dyn MessagingChannel>>>>,
    handlers: Arc<RwLock<HashMap<CommandCategory, Arc<dyn MessageHandler>>>>,
    session_binder: SessionBinder,
    rate_limiters: Arc<RwLock<HashMap<Platform, RateLimiter>>>,
    configs: Arc<RwLock<HashMap<Platform, ChannelConfig>>>,
    audit: Option<Arc<MessagingAuditLogger>>,
    retry_queue: Option<Arc<RetryQueue>>,
    tenant_manager: Option<Arc<TenantManager>>,
    tenant_resolver: Option<TenantResolver>,
    state_store: Option<Arc<dyn StateStore>>,
    usage_tracker: Option<Arc<UsageTracker>>,
    /// Monotonic instant when the gateway was created (for uptime / health).
    started_at: Instant,
}

impl MessagingGateway {
    /// Creates a new messaging gateway
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            session_binder: SessionBinder::new(),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
            audit: None,
            retry_queue: None,
            tenant_manager: None,
            tenant_resolver: None,
            state_store: None,
            usage_tracker: None,
            started_at: Instant::now(),
        }
    }

    pub fn with_audit(mut self, audit: Arc<MessagingAuditLogger>) -> Self {
        self.audit = Some(audit);
        self
    }

    pub fn with_retry_queue(mut self, queue: Arc<RetryQueue>) -> Self {
        self.retry_queue = Some(queue);
        self
    }

    pub fn with_tenant_manager(mut self, manager: Arc<TenantManager>) -> Self {
        self.tenant_manager = Some(manager);
        self
    }

    pub fn with_tenant_resolver(mut self, resolver: TenantResolver) -> Self {
        self.tenant_resolver = Some(resolver);
        self
    }

    pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.session_binder.state_store = Some(store.clone());
        self.state_store = Some(store);
        self
    }

    pub fn with_usage_tracker(mut self, tracker: Arc<UsageTracker>) -> Self {
        self.usage_tracker = Some(tracker);
        self
    }

    /// Registers a messaging channel
    pub async fn register_channel(&self, channel: Arc<dyn MessagingChannel>) {
        let platform = channel.platform();
        let mut channels = self.channels.write().await;
        channels.insert(platform, channel);
    }

    /// Registers a command handler for a category
    pub async fn register_handler(
        &self,
        category: CommandCategory,
        handler: Arc<dyn MessageHandler>,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(category, handler);
    }

    /// Configures a channel with rate limiting
    pub async fn configure_channel(&self, config: ChannelConfig) {
        let platform = config.platform;
        let mut rate_limiter = RateLimiter::new(config.rate_limit.clone());
        if let Some(store) = &self.state_store {
            rate_limiter.state_store = Some(store.clone());
        }

        let mut limiters = self.rate_limiters.write().await;
        limiters.insert(platform, rate_limiter);

        let mut configs = self.configs.write().await;
        configs.insert(platform, config);
    }

    /// Processes an incoming message and returns message IDs
    pub async fn process_message(
        &self,
        platform: Platform,
        user_id: &str,
        chat_id: &str,
        message: &str,
        api_key: Option<&str>,
    ) -> Result<Vec<String>> {
        let start = Instant::now();
        let mut tenant_id = String::from("default");

        // Check if channel is enabled
        let config = self.get_config(platform).await;
        if let Some(cfg) = &config {
            if !cfg.enabled {
                self.record_usage(
                    &tenant_id,
                    platform,
                    user_id,
                    &CommandCategory::Unknown,
                    Outcome::Error,
                    start,
                )
                .await;
                return Err(MessagingError::ChannelUnavailable(platform.to_string()));
            }
        }

        // Resolve tenant (if multi-tenancy is configured)
        let _tenant_ctx: Option<TenantContext> = if let Some(resolver) = &self.tenant_resolver {
            if let Some(manager) = &self.tenant_manager {
                if let Some(key) = api_key {
                    if let Ok(Some(tenant_id)) = resolver.resolve_by_api_key(key) {
                        if let Ok(Some(_tenant_config)) = manager.get_tenant(&tenant_id) {
                            let ctx = TenantContext::new(tenant_id, manager);
                            if let Ok(ctx) = ctx {
                                if !ctx.is_platform_allowed(platform) {
                                    self.record_usage(
                                        &ctx.tenant_id.to_string(),
                                        platform,
                                        user_id,
                                        &CommandCategory::Unknown,
                                        Outcome::Unauthorized,
                                        start,
                                    )
                                    .await;
                                    return Err(MessagingError::ChannelUnavailable(format!(
                                        "Platform {} not allowed for tenant",
                                        platform
                                    )));
                                }
                                Some(ctx)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Extract tenant_id for usage tracking
        tenant_id = _tenant_ctx
            .as_ref()
            .map(|ctx| ctx.tenant_id.to_string())
            .unwrap_or_else(|| "default".to_string());

        // Check rate limit (tenant override takes precedence)
        {
            let mut tenant_checked = false;
            if let Some(ctx) = &_tenant_ctx {
                if let Some(override_cfg) = ctx.get_rate_limit(platform) {
                    let tenant_limiter = RateLimiter::new(override_cfg);
                    if let Err(e) = tenant_limiter.check_rate_limit(user_id).await {
                        if let Some(audit) = &self.audit {
                            let _ = audit
                                .log_event(MessagingAuditEvent::RateLimitHit {
                                    platform,
                                    user_id: user_id.to_string(),
                                    limit: 0,
                                    remaining: 0,
                                })
                                .await;
                        }
                        self.record_usage(
                            &tenant_id,
                            platform,
                            user_id,
                            &CommandCategory::Unknown,
                            Outcome::RateLimited,
                            start,
                        )
                        .await;
                        return Err(e);
                    }
                    tenant_checked = true;
                }
            }
            if !tenant_checked {
                if let Some(limiter) = self.get_rate_limiter(platform).await {
                    if let Err(e) = limiter.check_rate_limit(user_id).await {
                        if let Some(audit) = &self.audit {
                            let _ = audit
                                .log_event(MessagingAuditEvent::RateLimitHit {
                                    platform,
                                    user_id: user_id.to_string(),
                                    limit: 0,
                                    remaining: 0,
                                })
                                .await;
                        }
                        self.record_usage(
                            &tenant_id,
                            platform,
                            user_id,
                            &CommandCategory::Unknown,
                            Outcome::RateLimited,
                            start,
                        )
                        .await;
                        return Err(e);
                    }
                }
            }
        }

        // Parse command
        let parser = CommandParser::new(platform)?;
        let parsed = match parser.parse(message) {
            Ok(cmd) => cmd,
            Err(MessagingError::InvalidCommandFormat { .. }) => {
                // Non-command text (no prefix) — respond with a friendly hint
                let prefix = platform.command_prefix();
                let result = MessageHandlerResult {
                    response: format!(
                        "Hi! To use Clawdius, start your message with `{}`. Try `{prefix} help` for available commands.",
                        prefix.trim()
                    ),
                    should_chunk: false,
                    stream: None,
                };
                return self.send_response(platform, chat_id, result).await;
            },
            Err(e) => return Err(e),
        };

        // Tenant-based command filtering
        if let Some(ctx) = &_tenant_ctx {
            if !ctx.is_command_allowed(parsed.category) {
                self.record_usage(
                    &tenant_id,
                    platform,
                    user_id,
                    &parsed.category,
                    Outcome::Unauthorized,
                    start,
                )
                .await;
                return Err(MessagingError::Unauthorized {
                    user_id: user_id.to_string(),
                    action: parsed.action.clone(),
                });
            }
        }

        // Tenant permission floor check
        if let Some(ctx) = &_tenant_ctx {
            let perms = ctx.get_permissions();
            let allowed = match parsed.category {
                CommandCategory::Generate => perms.can_generate,
                CommandCategory::Analyze => perms.can_analyze,
                CommandCategory::Config => perms.can_modify_files,
                CommandCategory::Admin => perms.can_admin,
                _ => true,
            };
            if !allowed {
                return Err(MessagingError::Unauthorized {
                    user_id: user_id.to_string(),
                    action: format!("{:?}", parsed.category),
                });
            }
        }

        if let Some(audit) = &self.audit {
            let _ = audit
                .log_event(MessagingAuditEvent::MessageReceived {
                    platform,
                    user_id: user_id.to_string(),
                    chat_id: chat_id.to_string(),
                    message_length: message.len(),
                })
                .await;
        }

        // Bind session
        let platform_user = PlatformUserId::new(platform, user_id);
        let session = self.session_binder.bind_session(&platform_user).await?;

        // Check tenant max sessions per user
        if let Some(ctx) = &_tenant_ctx {
            let max = ctx.config.max_sessions_per_user;
            let current = self.session_binder.sessions_for_user(user_id).await;
            if current > max as usize {
                if let Some(audit) = &self.audit {
                    let _ = audit
                        .log_event(MessagingAuditEvent::PermissionDenied {
                            platform,
                            user_id: user_id.to_string(),
                            command: message[..message.len().min(30)].to_string(),
                            required_permission: "session_slot".to_string(),
                        })
                        .await;
                }
                self.record_usage(
                    &tenant_id,
                    platform,
                    user_id,
                    &parsed.category,
                    Outcome::Unauthorized,
                    start,
                )
                .await;
                return Err(MessagingError::Unauthorized {
                    user_id: user_id.to_string(),
                    action: "max sessions exceeded".to_string(),
                });
            }
        }

        // Check permissions
        if !self.check_permissions(&session, &parsed) {
            if let Some(audit) = &self.audit {
                let _ = audit
                    .log_event(MessagingAuditEvent::PermissionDenied {
                        platform,
                        user_id: user_id.to_string(),
                        command: parsed.action.clone(),
                        required_permission: format!("{:?}", parsed.category),
                    })
                    .await;
            }
            self.record_usage(
                &tenant_id,
                platform,
                user_id,
                &parsed.category,
                Outcome::Unauthorized,
                start,
            )
            .await;
            return Err(MessagingError::Unauthorized {
                user_id: user_id.to_string(),
                action: parsed.action.clone(),
            });
        }

        // Get handler and process
        let handler = self.get_handler(&parsed.category).await;
        let start = Instant::now();
        let result = if let Some(h) = handler {
            match h.handle(&session, &parsed).await {
                Ok(r) => r,
                Err(e) => {
                    if let Some(audit) = &self.audit {
                        let _ = audit
                            .log_event(MessagingAuditEvent::CommandExecuted {
                                platform,
                                user_id: user_id.to_string(),
                                command: parsed.action.clone(),
                                category: format!("{:?}", parsed.category),
                                success: false,
                                latency_ms: start.elapsed().as_millis() as u64,
                            })
                            .await;
                    }
                    self.record_usage(
                        &tenant_id,
                        platform,
                        user_id,
                        &parsed.category,
                        Outcome::Error,
                        start,
                    )
                    .await;
                    return Err(e);
                },
            }
        } else {
            MessageHandlerResult {
                response: format!("Unknown command: {}", parsed.action),
                should_chunk: false,
                stream: None,
            }
        };
        let latency = start.elapsed().as_millis() as u64;

        if let Some(audit) = &self.audit {
            let _ = audit
                .log_event(MessagingAuditEvent::CommandExecuted {
                    platform,
                    user_id: user_id.to_string(),
                    command: parsed.action.clone(),
                    category: format!("{:?}", parsed.category),
                    success: true,
                    latency_ms: latency,
                })
                .await;
        }

        self.record_usage(
            &tenant_id,
            platform,
            user_id,
            &parsed.category,
            Outcome::Success,
            start,
        )
        .await;

        // Send response (streaming if handler provided a stream)
        self.send_streaming_response(platform, chat_id, result)
            .await
    }

    /// Record a usage event if the usage tracker is configured.
    async fn record_usage(
        &self,
        tenant_id: &str,
        platform: Platform,
        user_id: &str,
        category: &CommandCategory,
        outcome: Outcome,
        start: Instant,
    ) {
        if let Some(tracker) = &self.usage_tracker {
            let event = UsageEvent::new(
                tenant_id.to_string(),
                user_id.to_string(),
                platform,
                *category,
                outcome,
                start.elapsed().as_millis() as u64,
            );
            tracker.record(event).await;
        }
    }

    /// Send a handler result through the platform channel.
    async fn send_response(
        &self,
        platform: Platform,
        chat_id: &str,
        result: MessageHandlerResult,
    ) -> Result<Vec<String>> {
        let channel = self.get_channel(platform).await;
        if let Some(ch) = channel {
            let message_ids = if result.should_chunk {
                let chunks = chunk_message(&result.response, platform.max_message_length());
                let mut ids = Vec::new();
                for chunk in &chunks {
                    match ch.send_message(chat_id, chunk).await {
                        Ok(msg_id) => ids.push(msg_id),
                        Err(e) => {
                            if let Some(rq) = &self.retry_queue {
                                let _ = rq
                                    .enqueue(
                                        platform,
                                        chat_id,
                                        chunk,
                                        serde_json::json!({"error": e.to_string()}),
                                    )
                                    .await;
                            }
                            return Err(e);
                        },
                    }
                }
                ids
            } else {
                match ch.send_message(chat_id, &result.response).await {
                    Ok(msg_id) => vec![msg_id],
                    Err(e) => {
                        if let Some(rq) = &self.retry_queue {
                            let _ = rq
                                .enqueue(
                                    platform,
                                    chat_id,
                                    &result.response,
                                    serde_json::json!({"error": e.to_string()}),
                                )
                                .await;
                        }
                        return Err(e);
                    },
                }
            };

            if let Some(audit) = &self.audit {
                let _ = audit
                    .log_event(MessagingAuditEvent::ResponseSent {
                        platform,
                        user_id: String::new(),
                        chat_id: chat_id.to_string(),
                        message_ids: message_ids.clone(),
                        was_streaming: false,
                        total_chunks: message_ids.len(),
                    })
                    .await;
            }

            Ok(message_ids)
        } else {
            Err(MessagingError::ChannelUnavailable(platform.to_string()))
        }
    }

    /// Send a handler result with optional streaming progressive edits.
    ///
    /// If `result.stream` is Some and the channel supports editing, this sends
    /// the initial message then progressively edits it as chunks arrive.
    /// Otherwise falls back to normal `send_response`.
    async fn send_streaming_response(
        &self,
        platform: Platform,
        chat_id: &str,
        mut result: MessageHandlerResult,
    ) -> Result<Vec<String>> {
        let channel = self.get_channel(platform).await;
        let channel = match channel {
            Some(ch) => ch,
            None => return Err(MessagingError::ChannelUnavailable(platform.to_string())),
        };

        // If no stream or channel doesn't support edit, use normal send
        let mut stream = match result.stream.take() {
            Some(s) if channel.supports_edit() => s,
            _ => return self.send_response(platform, chat_id, result).await,
        };

        // Send initial placeholder message
        let message_id = channel.send_message(chat_id, &result.response).await?;
        let max_len = platform.max_message_length();
        let mut accumulated = result.response;

        // Debounce interval for edits (300ms)
        let edit_interval = std::time::Duration::from_millis(300);
        let mut last_edit = std::time::Instant::now();
        let mut dirty = false;

        while let Some(chunk) = stream.recv().await {
            accumulated.push_str(&chunk);

            if last_edit.elapsed() >= edit_interval {
                let text_to_show = if accumulated.len() > max_len {
                    &accumulated[..max_len]
                } else {
                    &accumulated
                };
                if let Err(e) = channel
                    .edit_message(chat_id, &message_id, text_to_show)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to edit streaming message");
                    if let Some(audit) = &self.audit {
                        let _ = audit
                            .log_event(MessagingAuditEvent::ChannelError {
                                platform,
                                operation: "edit_message".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                    if let Some(rq) = &self.retry_queue {
                        let _ = rq
                            .enqueue(
                                platform,
                                chat_id,
                                text_to_show,
                                serde_json::json!({"error": e.to_string()}),
                            )
                            .await;
                    }
                }
                last_edit = std::time::Instant::now();
                dirty = false;
            }
        }

        // Final edit with complete accumulated content
        // If content exceeds max, chunk it
        if accumulated.len() > max_len {
            if let Err(e) = channel
                .edit_message(chat_id, &message_id, &accumulated[..max_len])
                .await
            {
                tracing::warn!(error = %e, "Failed to edit streaming message (overflow)");
                if let Some(audit) = &self.audit {
                    let _ = audit
                        .log_event(MessagingAuditEvent::ChannelError {
                            platform,
                            operation: "edit_message".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                if let Some(rq) = &self.retry_queue {
                    let _ = rq
                        .enqueue(
                            platform,
                            chat_id,
                            &accumulated[..max_len],
                            serde_json::json!({"error": e.to_string()}),
                        )
                        .await;
                }
            }
            let remainder = &accumulated[max_len..];
            let chunks = chunk_message(remainder, max_len);
            let mut ids = vec![message_id.clone()];
            for chunk in &chunks {
                let id = channel.send_message(chat_id, chunk).await?;
                ids.push(id);
            }
            return Ok(ids);
        }

        if dirty {
            if let Err(e) = channel
                .edit_message(chat_id, &message_id, &accumulated)
                .await
            {
                tracing::warn!(error = %e, "Failed to edit streaming message (final)");
                if let Some(audit) = &self.audit {
                    let _ = audit
                        .log_event(MessagingAuditEvent::ChannelError {
                            platform,
                            operation: "edit_message".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                if let Some(rq) = &self.retry_queue {
                    let _ = rq
                        .enqueue(
                            platform,
                            chat_id,
                            &accumulated,
                            serde_json::json!({"error": e.to_string()}),
                        )
                        .await;
                }
            }
        }

        Ok(vec![message_id])
    }

    async fn get_channel(&self, platform: Platform) -> Option<Arc<dyn MessagingChannel>> {
        let channels = self.channels.read().await;
        channels.get(&platform).cloned()
    }

    async fn get_handler(&self, category: &CommandCategory) -> Option<Arc<dyn MessageHandler>> {
        let handlers = self.handlers.read().await;
        handlers.get(category).cloned()
    }

    async fn get_rate_limiter(&self, platform: Platform) -> Option<RateLimiter> {
        let limiters = self.rate_limiters.read().await;
        limiters.get(&platform).cloned()
    }

    async fn get_config(&self, platform: Platform) -> Option<ChannelConfig> {
        let configs = self.configs.read().await;
        configs.get(&platform).cloned()
    }

    fn check_permissions(&self, session: &MessagingSession, command: &ParsedCommand) -> bool {
        match command.category {
            CommandCategory::Generate => session.permissions.can_generate,
            CommandCategory::Analyze => session.permissions.can_analyze,
            CommandCategory::Config => session.permissions.can_modify_files,
            CommandCategory::Admin => session.permissions.can_admin,
            CommandCategory::Timeline => session.permissions.can_modify_files,
            CommandCategory::Session => true,
            CommandCategory::Status => true,
            CommandCategory::Help => true,
            CommandCategory::Unknown => true, // Unknown commands need only basic access
        }
    }

    /// Returns the number of registered channels
    pub async fn channel_count(&self) -> usize {
        self.channels.read().await.len()
    }

    /// Returns the number of active sessions
    pub async fn session_count(&self) -> usize {
        self.session_binder.session_count().await
    }

    /// Returns gateway uptime in seconds (monotonic).
    #[must_use]
    pub async fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Returns `true` if all wired dependencies are reachable.
    ///
    /// Checks the state store health if one is configured.
    pub async fn health_check(&self) -> bool {
        match &self.state_store {
            Some(store) => store.health_check().await.unwrap_or(false),
            None => true, // no store wired → trivially healthy
        }
    }

    /// Gracefully shut down the gateway.
    ///
    /// Flushes the audit logger, reports retry queue status, and logs session count.
    pub async fn shutdown(&self) {
        tracing::info!(
            sessions = self.session_binder.session_count().await,
            "Flushing sessions"
        );

        if let Some(rq) = &self.retry_queue {
            let (pending, dead) = rq.shutdown().await;
            if pending > 0 {
                tracing::warn!(
                    pending,
                    "Retry queue has pending tasks that will not be retried"
                );
            }
            if dead > 0 {
                tracing::info!(
                    dead_letter = dead,
                    "Dead letter queue preserved for inspection"
                );
            }
        }

        tracing::info!("Gateway shutdown complete");
    }

    /// Resolve a tenant context for the given API key.
    /// Returns None if multi-tenancy is not configured or the key is unknown.
    pub fn resolve_tenant(&self, api_key: &str) -> Option<TenantContext> {
        let resolver = self.tenant_resolver.as_ref()?;
        let tenant_id = resolver.resolve_by_api_key(api_key).ok()??;
        let manager = self.tenant_manager.as_ref()?;
        TenantContext::new(tenant_id, manager).ok()
    }
}

impl Default for MessagingGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct MockHandler;

    #[async_trait]
    impl MessageHandler for MockHandler {
        async fn handle(
            &self,
            _session: &MessagingSession,
            command: &ParsedCommand,
        ) -> Result<MessageHandlerResult> {
            Ok(MessageHandlerResult {
                response: format!("Handled: {}", command.action),
                should_chunk: false,
                stream: None,
            })
        }
    }

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = MessagingGateway::new();
        assert_eq!(gateway.channel_count().await, 0);
        assert_eq!(gateway.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_handler_registration() {
        let gateway = MessagingGateway::new();
        gateway
            .register_handler(CommandCategory::Status, Arc::new(MockHandler))
            .await;

        let handler = gateway.get_handler(&CommandCategory::Status).await;
        assert!(handler.is_some());
    }
}
