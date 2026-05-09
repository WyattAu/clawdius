//! Platform adapter implementations.
//!
//! Each module provides a [`PlatformAdapter`](crate::adapter::PlatformAdapter)
//! implementation for a specific chat platform. Adapters are feature-gated
//! to avoid pulling in unnecessary dependencies.
//!
//! # Always Available
//!
//! - [`mock`] — In-memory adapter for testing
//! - [`webhook`] — Generic HTTP webhook adapter
//! - [`signal`] — Signal (via signal-cli REST API)
//! - [`teams`] — Microsoft Teams (via Bot Framework REST API)
//! - [`whatsapp`] — WhatsApp (via Meta Cloud API)
//! - [`rocketchat`] — Rocket.Chat (via REST API)
//!
//! # Feature-Gated
//!
//! - `telegram` — `#[cfg(feature = "telegram")]` — teloxide SDK
//! - `discord` — `#[cfg(feature = "discord")]` — serenity SDK
//! - `slack` — `#[cfg(feature = "slack")]` — slack-morphism SDK
//! - `matrix` — `#[cfg(feature = "matrix")]` — matrix-sdk

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "matrix")]
pub mod matrix;

/// Mock platform adapter for testing.
pub mod mock;

/// Generic webhook adapter (always available).
pub mod webhook;

/// Signal adapter via signal-cli REST API.
pub mod signal;

/// Microsoft Teams adapter via Bot Framework REST API.
pub mod teams;

/// WhatsApp adapter via Meta Cloud API.
pub mod whatsapp;

/// Rocket.Chat adapter via REST API.
pub mod rocketchat;
