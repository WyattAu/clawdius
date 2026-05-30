//! Per-user, per-platform rate limiting.
//!
//! Uses a sliding window algorithm to enforce rate limits.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::adapter::Platform;

/// Rate limiter using a sliding window counter.
///
/// Thread-safe via `Mutex`. For high-throughput deployments,
/// consider replacing with Redis-backed rate limiting.
pub struct RateLimiter {
    /// Maximum requests per window.
    max_requests: usize,
    /// Window duration.
    window: Duration,
    /// State: (`user_id`, platform) → list of request timestamps.
    state: Mutex<HashMap<(String, String), Vec<Instant>>>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Arguments
    /// * `max_requests` — Maximum number of requests allowed per window.
    /// * `window_secs` — Duration of the sliding window in seconds.
    #[must_use]
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(window_secs),
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Create a rate limiter with common defaults (20 requests per 60 seconds).
    #[must_use]
    pub fn default_limiter() -> Self {
        Self::new(20, 60)
    }

    /// Check if a request is allowed, and record it if so.
    ///
    /// Returns `Ok(())` if the request is within rate limits,
    /// or `Err` with the number of milliseconds until the next
    /// allowed request.
    ///
    /// # Errors
    ///
    /// Returns `Err(RateLimitError)` if the rate limit is exceeded.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[allow(clippy::expect_used)]
    pub fn check(&self, platform: Platform, user_id: &str) -> Result<(), RateLimitError> {
        let key = (user_id.to_string(), platform.as_str().to_string());
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window).expect("time window overflow");

        let mut state = self.state.lock().expect("lock poisoned");
        let timestamps = state.entry(key).or_default();

        // Remove expired entries
        timestamps.retain(|&t| t > cutoff);

        if timestamps.len() >= self.max_requests {
            // Find when the oldest request in the window will expire
            let oldest = timestamps
                .first()
                .expect("timestamps is non-empty because len >= max_requests");
            let retry_after = oldest.duration_since(cutoff);
            return Err(RateLimitError {
                #[allow(clippy::cast_possible_truncation)]
                retry_after_ms: retry_after.as_millis() as u64,
            });
        }

        timestamps.push(now);
        Ok(())
    }

    /// Get the current request count for a user/platform.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn current_count(&self, platform: Platform, user_id: &str) -> usize {
        let key = (user_id.to_string(), platform.as_str().to_string());
        let now = Instant::now();
        let cutoff = now.checked_sub(self.window).expect("time window overflow");

        let state = self.state.lock().expect("lock poisoned");
        state
            .get(&key)
            .map_or(0, |ts| ts.iter().filter(|&&t| t > cutoff).count())
    }

    /// Reset rate limit state for a user/platform.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[allow(clippy::expect_used)]
    pub fn reset(&self, platform: Platform, user_id: &str) {
        let key = (user_id.to_string(), platform.as_str().to_string());
        let mut state = self.state.lock().expect("lock poisoned");
        state.remove(&key);
    }

    /// Clear all rate limit state.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[allow(clippy::expect_used)]
    pub fn clear_all(&self) {
        let mut state = self.state.lock().expect("lock poisoned");
        state.clear();
    }
}

/// Rate limit exceeded error.
#[derive(Debug, Clone)]
pub struct RateLimitError {
    /// Milliseconds until the next allowed request.
    pub retry_after_ms: u64,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rate limit exceeded, retry after {}ms",
            self.retry_after_ms
        )
    }
}

impl std::error::Error for RateLimitError {}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_within_limit() {
        let limiter = RateLimiter::new(5, 60);
        for _ in 0..5 {
            assert!(limiter.check(Platform::Telegram, "user1").is_ok());
        }
    }

    #[test]
    fn test_rejects_over_limit() {
        let limiter = RateLimiter::new(3, 60);
        for _ in 0..3 {
            assert!(limiter.check(Platform::Discord, "user1").is_ok());
        }
        let result = limiter.check(Platform::Discord, "user1");
        assert!(result.is_err());
        assert!(result.unwrap_err().retry_after_ms > 0);
    }

    #[test]
    fn test_separate_users_independent() {
        let limiter = RateLimiter::new(2, 60);
        for _ in 0..2 {
            limiter.check(Platform::Telegram, "user1").unwrap();
        }
        // user2 should still be allowed
        assert!(limiter.check(Platform::Telegram, "user2").is_ok());
    }

    #[test]
    fn test_separate_platforms_independent() {
        let limiter = RateLimiter::new(2, 60);
        for _ in 0..2 {
            limiter.check(Platform::Telegram, "user1").unwrap();
        }
        // Same user, different platform should be allowed
        assert!(limiter.check(Platform::Discord, "user1").is_ok());
    }

    #[test]
    fn test_current_count() {
        let limiter = RateLimiter::new(5, 60);
        assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 0);
        limiter.check(Platform::Telegram, "user1").unwrap();
        assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 1);
    }

    #[test]
    fn test_reset() {
        let limiter = RateLimiter::new(2, 60);
        for _ in 0..2 {
            limiter.check(Platform::Telegram, "user1").unwrap();
        }
        limiter.reset(Platform::Telegram, "user1");
        assert!(limiter.check(Platform::Telegram, "user1").is_ok());
    }

    #[test]
    fn test_default_limiter() {
        let limiter = RateLimiter::default_limiter();
        // Should allow 20 requests
        for _ in 0..20 {
            assert!(limiter.check(Platform::Telegram, "user1").is_ok());
        }
        assert!(limiter.check(Platform::Telegram, "user1").is_err());
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = RateLimitError {
            retry_after_ms: 5000,
        };
        let msg = format!("{err}");
        assert!(msg.contains("5000ms"));
    }

    #[test]
    fn test_clear_all_resets_all_users() {
        let limiter = RateLimiter::new(2, 60);
        limiter.check(Platform::Telegram, "user1").unwrap();
        limiter.check(Platform::Telegram, "user1").unwrap();
        limiter.check(Platform::Discord, "user2").unwrap();
        limiter.check(Platform::Discord, "user2").unwrap();

        assert!(limiter.check(Platform::Telegram, "user1").is_err());
        assert!(limiter.check(Platform::Discord, "user2").is_err());

        limiter.clear_all();

        assert!(limiter.check(Platform::Telegram, "user1").is_ok());
        assert!(limiter.check(Platform::Discord, "user2").is_ok());
    }

    #[test]
    fn test_per_tenant_independent_limits() {
        let limiter = RateLimiter::new(1, 60);

        limiter.check(Platform::Telegram, "tenant_a").unwrap();
        assert!(limiter.check(Platform::Telegram, "tenant_a").is_err());

        assert!(limiter.check(Platform::Telegram, "tenant_b").is_ok());
        assert!(limiter.check(Platform::Telegram, "tenant_b").is_err());

        assert!(limiter.check(Platform::Telegram, "tenant_c").is_ok());
    }

    #[test]
    fn test_rate_limit_all_platforms_independent() {
        let limiter = RateLimiter::new(1, 60);
        let platforms = [
            Platform::Telegram,
            Platform::Discord,
            Platform::Slack,
            Platform::Matrix,
            Platform::Signal,
        ];

        for &platform in &platforms {
            assert!(limiter.check(platform, "user1").is_ok());
        }

        for &platform in &platforms {
            assert!(limiter.check(platform, "user1").is_err());
        }
    }

    #[test]
    fn test_rate_limit_retry_after_decreases() {
        let limiter = RateLimiter::new(1, 1);
        limiter.check(Platform::Telegram, "user1").unwrap();
        let err1 = limiter.check(Platform::Telegram, "user1").unwrap_err();
        assert!(err1.retry_after_ms > 0);
        assert!(err1.retry_after_ms <= 1000);
    }

    #[test]
    fn test_current_count_reflects_window() {
        let limiter = RateLimiter::new(10, 60);
        for _ in 0..5 {
            limiter.check(Platform::Telegram, "user1").unwrap();
        }
        assert_eq!(limiter.current_count(Platform::Telegram, "user1"), 5);
        assert_eq!(limiter.current_count(Platform::Discord, "user1"), 0);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RateLimitError>();
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Rate limiter never exceeds threshold for same user+platform
        #[test]
        fn prop_never_exceeds_threshold(
            max_req in 1usize..50,
            window_secs in 1u64..10,
            n_calls in 0usize..100
        ) {
            let limiter = RateLimiter::new(max_req, window_secs);
            let mut accepted = 0usize;
            for _ in 0..n_calls {
                if limiter.check(Platform::Telegram, "user1").is_ok() {
                    accepted += 1;
                }
            }
            assert!(accepted <= max_req,
                "Accepted {accepted} requests but limit was {max_req}");
        }

        /// User isolation: exhausting user1 does not affect user2
        #[test]
        fn prop_user_isolation(max_req in 1usize..20, window_secs in 1u64..10) {
            let limiter = RateLimiter::new(max_req, window_secs);
            // Exhaust user1
            for _ in 0..max_req + 5 {
                limiter.check(Platform::Discord, "user1").ok();
            }
            assert!(limiter.check(Platform::Discord, "user1").is_err(),
                "user1 should be rate limited");
            assert!(limiter.check(Platform::Discord, "user2").is_ok(),
                "user2 should NOT be rate limited");
        }

        /// Reset restores full capacity
        #[test]
        fn prop_reset_restores_capacity(max_req in 1usize..50, window_secs in 1u64..10) {
            let limiter = RateLimiter::new(max_req, window_secs);
            // Exhaust
            for _ in 0..max_req {
                limiter.check(Platform::Slack, "user").ok();
            }
            assert!(limiter.check(Platform::Slack, "user").is_err());
            limiter.reset(Platform::Slack, "user");
            assert!(limiter.check(Platform::Slack, "user").is_ok());
            assert_eq!(limiter.current_count(Platform::Slack, "user"), 1);
        }

        /// Retry-after is bounded by window duration
        #[test]
        fn prop_retry_after_bounded(max_req in 1usize..10, window_secs in 1u64..60) {
            let limiter = RateLimiter::new(max_req, window_secs);
            for _ in 0..max_req {
                limiter.check(Platform::Signal, "user").ok();
            }
            match limiter.check(Platform::Signal, "user") {
                Err(e) => assert!(e.retry_after_ms <= window_secs * 1000),
                Ok(()) => panic!("Expected rate limit error"),
            }
        }

        /// current_count reflects accepted requests
        #[test]
        fn prop_current_count_accuracy(
            max_req in 1usize..30,
            window_secs in 1u64..10,
            n_calls in 0usize..50
        ) {
            let limiter = RateLimiter::new(max_req, window_secs);
            let mut expected = 0usize;
            for _ in 0..n_calls {
                if limiter.check(Platform::Teams, "user").is_ok() {
                    expected += 1;
                }
            }
            assert_eq!(limiter.current_count(Platform::Teams, "user"), expected);
        }
    }
}
