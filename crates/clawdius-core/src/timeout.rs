//! Timeout handling utilities
//!
//! This module provides consistent timeout handling across the codebase,
//! including configurable timeouts, graceful degradation, and timeout combinators.

use std::future::Future;
use std::time::Duration;
use tokio::time::{timeout, timeout_at, Instant};

use crate::error::{Error, Result};

/// Configuration for timeout behavior
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout duration
    pub default: Duration,
    /// Timeout for LLM operations (typically longer)
    pub llm: Duration,
    /// Timeout for file operations
    pub file: Duration,
    /// Timeout for network operations
    pub network: Duration,
    /// Enable timeout warnings before expiry
    pub warn_before: Option<Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default: Duration::from_secs(30),
            llm: Duration::from_secs(120), // 2 minutes for LLM calls
            file: Duration::from_secs(10),
            network: Duration::from_secs(60),
            warn_before: Some(Duration::from_secs(5)),
        }
    }
}

impl TimeoutConfig {
    /// Creates a new timeout configuration with custom values.
    #[must_use]
    pub fn new(default: Duration, llm: Duration, file: Duration, network: Duration) -> Self {
        Self {
            default,
            llm,
            file,
            network,
            warn_before: Some(Duration::from_secs(5)),
        }
    }

    /// Creates a strict timeout configuration (shorter timeouts).
    #[must_use]
    pub fn strict() -> Self {
        Self {
            default: Duration::from_secs(10),
            llm: Duration::from_secs(60),
            file: Duration::from_secs(5),
            network: Duration::from_secs(30),
            warn_before: Some(Duration::from_secs(2)),
        }
    }

    /// Creates a relaxed timeout configuration (longer timeouts).
    #[must_use]
    pub fn relaxed() -> Self {
        Self {
            default: Duration::from_secs(60),
            llm: Duration::from_secs(300), // 5 minutes
            file: Duration::from_secs(30),
            network: Duration::from_secs(120),
            warn_before: Some(Duration::from_secs(10)),
        }
    }
}

/// Execute a future with a timeout, returning a custom error on timeout.
///
/// # Example
/// ```rust,no_run
/// use clawdius_core::timeout::with_timeout;
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() {
/// let result = with_timeout(
///     Duration::from_secs(30),
///     async { Ok::<_, clawdius_core::Error>(42) }
/// ).await;
/// # }
/// ```
pub async fn with_timeout<T>(
    duration: Duration,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    match timeout(duration, future).await {
        Ok(result) => result,
        Err(_) => Err(Error::Timeout(duration)),
    }
}

/// Execute a future with a timeout and a label for better error messages.
pub async fn with_timeout_labelled<T, F>(duration: Duration, label: &str, future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    match timeout(duration, future).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(
                label = label,
                timeout_ms = duration.as_millis(),
                "Operation timed out"
            );
            Err(Error::Timeout(duration))
        }
    }
}

/// Execute a future with a deadline (absolute time).
pub async fn with_deadline<T>(
    deadline: Instant,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    let now = Instant::now();
    match timeout_at(deadline, future).await {
        Ok(result) => result,
        Err(_) => Err(Error::Timeout(deadline.saturating_duration_since(now))),
    }
}

/// A timeout guard that can be used to track remaining time.
#[derive(Debug)]
pub struct TimeoutGuard {
    deadline: Instant,
    label: String,
}

impl TimeoutGuard {
    /// Creates a new timeout guard with the given duration.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            deadline: Instant::now() + duration,
            label: String::new(),
        }
    }

    /// Creates a new timeout guard with a label.
    #[must_use]
    pub fn with_label(duration: Duration, label: impl Into<String>) -> Self {
        Self {
            deadline: Instant::now() + duration,
            label: label.into(),
        }
    }

    /// Returns the remaining time before timeout.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Returns true if the timeout has elapsed.
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Checks if time is remaining, returns error if elapsed.
    ///
    /// # Errors
    ///
    /// Returns `Error::Timeout` if the deadline has passed.
    pub fn check(&self) -> Result<()> {
        if self.is_elapsed() {
            Err(Error::Timeout(self.remaining()))
        } else {
            Ok(())
        }
    }

    /// Returns the deadline as an `Instant`.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Wraps a future with this timeout guard.
    pub async fn wrap<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        with_deadline(self.deadline, future).await
    }
}

/// Execute multiple futures with a shared timeout.
pub async fn race_with_timeout<T>(
    duration: Duration,
    futures: Vec<impl Future<Output = Result<T>>>,
) -> Result<T> {
    let deadline = Instant::now() + duration;

    for future in futures {
        if Instant::now() >= deadline {
            return Err(Error::Timeout(duration));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, future).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(e)) => {
                tracing::debug!("Future failed in race: {}", e);
                continue;
            }
            Err(_) => continue,
        }
    }

    Err(Error::Timeout(duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_with_timeout_success() {
        let result = with_timeout(Duration::from_secs(1), async { Ok::<_, Error>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_timeout_elapsed() {
        let result = with_timeout(Duration::from_millis(10), async {
            sleep(Duration::from_millis(100)).await;
            Ok::<_, Error>(42)
        })
        .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Timeout(_)));
    }

    #[tokio::test]
    async fn test_timeout_guard_remaining() {
        let guard = TimeoutGuard::new(Duration::from_secs(10));
        let remaining = guard.remaining();

        assert!(remaining <= Duration::from_secs(10));
        assert!(remaining > Duration::from_secs(9));
    }

    #[tokio::test]
    async fn test_timeout_guard_elapsed() {
        let guard = TimeoutGuard::new(Duration::from_millis(1));
        sleep(Duration::from_millis(10)).await;

        assert!(guard.is_elapsed());
        assert!(guard.check().is_err());
    }

    #[tokio::test]
    async fn test_timeout_guard_wrap() {
        let guard = TimeoutGuard::new(Duration::from_secs(1));
        let result = guard.wrap(async { Ok::<_, Error>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_timeout_config_defaults() {
        let config = TimeoutConfig::default();
        assert_eq!(config.default, Duration::from_secs(30));
        assert_eq!(config.llm, Duration::from_secs(120));
    }

    #[test]
    fn test_timeout_config_strict() {
        let config = TimeoutConfig::strict();
        assert!(config.default < TimeoutConfig::default().default);
    }

    #[test]
    fn test_timeout_config_relaxed() {
        let config = TimeoutConfig::relaxed();
        assert!(config.default > TimeoutConfig::default().default);
    }
}
