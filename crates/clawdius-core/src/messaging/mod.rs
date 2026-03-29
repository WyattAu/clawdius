//! Messaging Module
//!
//! Multi-platform messaging gateway for remote command execution.
//!
//! This module provides bidirectional messaging capabilities for Clawdius,
//! enabling remote control and monitoring through various messaging platforms
//! including Telegram, Discord, Matrix, Signal, WhatsApp, RocketChat, and Slack.

pub mod audit;
pub mod auth;
pub mod channels;
pub mod command_parser;
pub mod config_builder;
pub mod encrypted_store;
pub mod gateway;
pub mod handlers;
pub mod integration;
#[cfg(feature = "jwt")]
pub mod jwt_auth;
pub mod key_rotation;
pub mod llm_cache;
pub mod oauth;
pub mod pii_redaction;
pub mod protocol;
pub mod rate_limiter;
pub mod retry_queue;
pub mod secret_resolver;
pub mod server;
pub mod session_binder;
pub mod state_store;
pub mod tenant;
pub mod types;
pub mod usage_tracker;
pub mod webhook_receiver;

// Re-exports for convenience
pub use types::{
    ChannelConfig, CommandCategory, IncomingMessage, MessageChunk, MessagingError,
    MessagingSession, OutgoingMessage, ParsedCommand, PermissionSet, Platform, PlatformUserId,
    RateLimitConfig, Result, SessionState,
};

pub use crate::config::{
    AuditConfig, MessagingConfig, PiiRedactionConfig, RetryQueueConfig, StateStoreConfig,
    TenantSectionConfig,
};
