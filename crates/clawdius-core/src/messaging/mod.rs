//! Messaging platform integrations.
//!
//! Provides a Telegram bot that bridges messaging platforms to the Clawdius
//! LLM agent. Each Telegram chat gets its own session with full conversation
//! history, tool-use support, and configurable rate limiting.
//!
//! # Quick Start
//!
//! ```toml
//! # clawdius.toml
//! [messaging]
//! enabled = true
//! telegram_bot_token = "123456:ABC-DEF..."  # or set CLAWDIUS_TELEGRAM_BOT_TOKEN
//! allowed_chat_ids = [-1001234567890]        # restrict to specific chats
//! ```
//!
//! ```bash
//! clawdius serve                    # starts REST API + Telegram bot
//! ```
//!
//! # Architecture
//!
//! ```text
//! Telegram API  ──►  TelegramBot  ──►  MessageRouter  ──►  LlmProvider
//!                   (long poll)       (session mgmt)       + SessionStore
//!                                        │                   + MCP tools
//!                                        ▼
//!                                   TelegramBot.sendMessage()
//! ```

pub mod discord;
pub mod matrix;
pub mod router;
pub mod telegram;
