//! Integration Tests for Messaging Gateway
//!
//! Tests the complete message flow from reception to response.

use std::sync::Arc;

use clawdius_core::messaging::command_parser::CommandParser;
use clawdius_core::messaging::types::{
    ChannelConfig, CommandCategory, MessagingSession, ParsedCommand, PermissionSet, Platform,
    PlatformUserId, RateLimitConfig,
};

/// Test that command parsing works for Telegram
#[tokio::test]
async fn test_command_parsing_telegram() {
    let parser = CommandParser::new(Platform::Telegram).unwrap();

    // Test status command
    let result = parser.parse("/clawd status").unwrap();
    assert_eq!(result.category, CommandCategory::Status);
    assert_eq!(result.action, "status");

    // Test help command
    let result = parser.parse("/clawd help").unwrap();
    assert_eq!(result.category, CommandCategory::Help);

    // Test generate command with args
    let result = parser
        .parse("/clawd generate function --lang rust")
        .unwrap();
    assert_eq!(result.category, CommandCategory::Generate);
    assert_eq!(result.flag("lang"), Some("rust"));
}

/// Test that command parsing works for Matrix
#[tokio::test]
async fn test_command_parsing_matrix() {
    let parser = CommandParser::new(Platform::Matrix).unwrap();

    // Matrix uses ! prefix
    let result = parser.parse("!clawd status").unwrap();
    assert_eq!(result.category, CommandCategory::Status);
}

/// Test that command parsing works for Discord
#[tokio::test]
async fn test_command_parsing_discord() {
    let parser = CommandParser::new(Platform::Discord).unwrap();

    let result = parser.parse("/clawd analyze this code").unwrap();
    assert_eq!(result.category, CommandCategory::Analyze);
}

/// Test invalid command format
#[tokio::test]
async fn test_invalid_command_format() {
    let parser = CommandParser::new(Platform::Telegram).unwrap();

    // Missing prefix
    let result = parser.parse("status");
    assert!(result.is_err());

    // Empty command
    let result = parser.parse("/clawd ");
    assert!(result.is_err());
}

/// Test message chunking
#[tokio::test]
async fn test_message_chunking() {
    use clawdius_core::messaging::command_parser::chunk_message;

    let long_text = "This is a very long message that exceeds the Telegram limit. ".repeat(100);
    let chunks = chunk_message(&long_text, 1000);

    assert!(chunks.len() > 1);
    for chunk in &chunks {
        assert!(chunk.len() <= 1000);
    }
}

/// Test platform specific limits
#[tokio::test]
async fn test_platform_specific_limits() {
    // Telegram: 4096
    assert_eq!(Platform::Telegram.max_message_length(), 4096);

    // Discord: 2000
    assert_eq!(Platform::Discord.max_message_length(), 2000);

    // Matrix: 65536
    assert_eq!(Platform::Matrix.max_message_length(), 65536);

    // Slack: 4000
    assert_eq!(Platform::Slack.max_message_length(), 4000);
}

/// Test session permissions
#[tokio::test]
async fn test_session_permissions() {
    let user = PlatformUserId::new(Platform::Telegram, "admin-user");
    let session = MessagingSession::new_admin(user.clone());

    assert!(session.permissions.can_admin);
    assert!(session.permissions.can_generate);
    assert!(session.permissions.can_execute);

    let regular_user = PlatformUserId::new(Platform::Discord, "regular-user");
    let regular_session = MessagingSession::new(regular_user);

    assert!(!regular_session.permissions.can_admin);
    assert!(!regular_session.permissions.can_execute);
    assert!(regular_session.permissions.can_generate); // Default allows generate
}

/// Test command categories
#[tokio::test]
async fn test_command_categories() {
    let parser = CommandParser::new(Platform::Telegram).unwrap();

    // Session commands
    let result = parser.parse("/clawd start session").unwrap();
    assert_eq!(result.category, CommandCategory::Session);

    // Config commands
    let result = parser.parse("/clawd config show").unwrap();
    assert_eq!(result.category, CommandCategory::Config);

    // Admin commands
    let result = parser.parse("/clawd admin status").unwrap();
    assert_eq!(result.category, CommandCategory::Admin);
}

/// Test flag parsing
#[tokio::test]
async fn test_flag_parsing() {
    let parser = CommandParser::new(Platform::Telegram).unwrap();

    let result = parser
        .parse("/clawd generate code --lang rust --verbose --count=5")
        .unwrap();

    assert_eq!(result.flag("lang"), Some("rust"));
    assert_eq!(result.flag("verbose"), Some("true"));
    assert_eq!(result.flag("count"), Some("5"));
}

/// Test all platforms have correct prefixes
#[tokio::test]
async fn test_all_platform_prefixes() {
    let platforms_and_prefixes = [
        (Platform::Telegram, "/clawd "),
        (Platform::Discord, "/clawd "),
        (Platform::Matrix, "!clawd "),
        (Platform::Signal, "/clawd "),
        (Platform::RocketChat, "/clawd "),
        (Platform::WhatsApp, "/clawd "),
        (Platform::Slack, "/clawd "),
    ];

    for (platform, prefix) in platforms_and_prefixes {
        assert_eq!(platform.command_prefix(), prefix);
    }
}

/// Test rate limit config defaults
#[tokio::test]
async fn test_rate_limit_config_defaults() {
    let config = RateLimitConfig::default();
    assert_eq!(config.requests_per_minute, 20);
    assert_eq!(config.burst_capacity, 10);
}

/// Test channel config creation
#[tokio::test]
async fn test_channel_config_creation() {
    let config = ChannelConfig::new(Platform::Telegram);
    assert_eq!(config.platform, Platform::Telegram);
    assert!(config.enabled);
}

/// Test permission set presets
#[tokio::test]
async fn test_permission_set_presets() {
    let admin = PermissionSet::admin();
    assert!(admin.can_admin);
    assert!(admin.can_generate);
    assert!(admin.can_execute);
    assert!(admin.can_modify_files);

    let read_only = PermissionSet::read_only();
    assert!(!read_only.can_generate);
    assert!(read_only.can_analyze);
    assert!(!read_only.can_modify_files);

    let standard = PermissionSet::new();
    assert!(standard.can_generate);
    assert!(standard.can_analyze);
    assert!(!standard.can_admin);

    let default = PermissionSet::default();
    assert!(!default.can_generate);
    assert!(!default.can_admin);
}
