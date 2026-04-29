//! Platform adapter implementations.
//!
//! Each module provides a [`PlatformAdapter`](crate::adapter::PlatformAdapter)
//! implementation for a specific chat platform. Adapters are feature-gated
//! to avoid pulling in unnecessary dependencies.

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
