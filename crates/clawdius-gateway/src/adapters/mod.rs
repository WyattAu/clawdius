//! Platform adapter implementations.

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "matrix")]
pub mod matrix;

#[cfg(feature = "webhook")]
pub mod webhook;

/// Mock platform adapter for testing.
pub mod mock;
