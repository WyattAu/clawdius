//! Session management and persistence for conversation history.
//!
//! This module provides comprehensive session management with SQLite-based persistence,
//! automatic context compaction, and efficient message storage.
//!
//! # Features
//!
//! - **Persistent storage**: SQLite-backed session storage with efficient queries
//! - **Automatic compaction**: Context-aware message compaction to stay within limits
//! - **Session metadata**: Rich metadata tracking for sessions
//! - **Search and filtering**: Query sessions by metadata, date, or content
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clawdius_core::session::{SessionManager, SessionStore, Session, Message, MessageRole};
//! use std::path::PathBuf;
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Initialize session store
//! let store = SessionStore::new(&PathBuf::from(".clawdius/sessions.db"))?;
//!
//! // Create a new session
//! let mut session = Session::new("My Session".to_string());
//! session.add_message(Message {
//!     role: MessageRole::User,
//!     content: "Hello, Clawdius!".to_string(),
//!     ..Default::default()
//! });
//!
//! // Save session
//! store.save_session(&session)?;
//!
//! // Load session later
//! let loaded = store.load_session(&session.id)?;
//!
//! // List all sessions
//! let sessions = store.list_sessions(Some(10), None)?;
//! for session_meta in sessions {
//!     println!("Session: {} - {}", session_meta.name, session_meta.created_at);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Session Manager
//!
//! The [`SessionManager`] provides higher-level session operations with automatic
//! compaction and context management:
//!
//! ```rust,no_run
//! use clawdius_core::session::{SessionManager, SessionStore};
//! use clawdius_core::config::SessionConfig;
//! use std::path::PathBuf;
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let store = SessionStore::new(&PathBuf::from(".clawdius/sessions.db"))?;
//! let config = SessionConfig::default();
//!
//! let manager = SessionManager::new(store, config);
//!
//! // Create or load session
//! let session_id = manager.create_session("New Chat".to_string())?;
//!
//! // Add messages
//! manager.add_message(&session_id, "user", "Hello")?;
//! manager.add_message(&session_id, "assistant", "Hi there!")?;
//!
//! // Get messages for LLM context
//! let messages = manager.get_messages(&session_id)?;
//!
//! // Automatic compaction when context gets too large
//! manager.maybe_compact(&session_id)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Context Compaction
//!
//! Sessions automatically compact when they approach context limits. The compactor
//! preserves recent messages and creates summaries of older content:
//!
//! ```rust,no_run
//! use clawdius_core::session::{Compactor, CompactConfig, Session, Message, MessageRole};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let config = CompactConfig {
//!     max_tokens: 100000,
//!     keep_recent: 10,
//!     min_messages: 20,
//! };
//!
//! let compactor = Compactor::new(config);
//!
//! let mut session = Session::new("Long Conversation".to_string());
//! // ... add many messages ...
//!
//! // Compact the session
//! let summary = compactor.compact(&mut session)?;
//! println!("Compacted {} messages", summary.messages_removed);
//! println!("Summary: {}", summary.summary);
//! # Ok(())
//! # }
//! ```
//!
//! # Session Metadata
//!
//! Sessions include rich metadata for organization and search:
//!
//! ```rust,no_run
//! use clawdius_core::session::{Session, SessionMeta};
//! use chrono::Utc;
//!
//! let session = Session::new("API Design Discussion".to_string());
//! let meta = SessionMeta {
//!     id: session.id.clone(),
//!     name: session.name.clone(),
//!     created_at: Utc::now(),
//!     updated_at: Utc::now(),
//!     message_count: 0,
//!     total_tokens: 0,
//!     tags: vec!["api".to_string(), "design".to_string()],
//!     metadata: serde_json::json!({
//!         "project": "clawdius-core",
//!         "focus": "session management"
//!     }),
//! };
//! ```
//!
//! # Thread Safety
//!
//! The session store uses `SQLite` with proper locking and can be safely shared
//! across threads. The [`SessionManager`] is designed for concurrent access.
//!
//! # Error Handling
//!
//! Session operations return [`Error`] variants:
//!
//! - [`Error::Session`]: General session errors
//! - [`Error::SessionNotFound`]: Session doesn't exist
//! - [`Error::Database`]: `SQLite` database errors
//!
//! [`Error`]: crate::Error
//! [`SessionManager`]: manager::SessionManager
//! [`SessionStore`]: store::SessionStore
//! [`Compactor`]: compactor::Compactor

pub mod compactor;
pub mod manager;
pub mod store;
pub mod types;

pub use compactor::{CompactConfig, CompactSummary, Compactor};
pub use manager::SessionManager;
pub use store::SessionStore;
pub use types::{Message, MessageContent, MessageRole, Session, SessionId, SessionMeta};
