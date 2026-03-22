//! Rate limiting for LLM API calls
//!
//! This module provides proactive rate limiting to prevent hitting API rate limits.
//! It implements a token bucket algorithm with configurable rate and burst capacity.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::sleep;

use crate::error::Result;

/// Configuration for the rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum requests per minute (rate)
    pub requests_per_minute: u32,
    /// Maximum burst capacity (allows temporary spikes)
    pub burst_capacity: u32,
    /// Enable adaptive rate limiting based on 429 responses
    pub adaptive: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,        // 1 request per second average
            burst_capacity: 10,             // Allow 10 burst requests
            adaptive: true,
        }
    }
}

impl RateLimiterConfig {
    /// Creates a configuration for Anthropic's rate limits.
    ///
    /// Anthropic typically allows ~60 requests/minute for standard tier.
    #[must_use]
    pub fn anthropic() -> Self {
        Self {
            requests_per_minute: 60,
            burst_capacity: 10,
            adaptive: true,
        }
    }

    /// Creates a configuration for OpenAI's rate limits.
    ///
    /// OpenAI varies by tier, but we use conservative defaults.
    #[must_use]
    pub fn openai() -> Self {
        Self {
            requests_per_minute: 60,
            burst_capacity: 20,
            adaptive: true,
        }
    }

    /// Creates a configuration for local LLMs (no rate limiting needed).
    #[must_use]
    pub fn local() -> Self {
        Self {
            requests_per_minute: u32::MAX,
            burst_capacity: u32::MAX,
            adaptive: false,
        }
    }

    /// Creates a configuration for Ollama (no rate limiting needed).
    #[must_use]
    pub fn ollama() -> Self {
        Self::local()
    }
}

/// Token bucket rate limiter for API calls.
///
/// Uses a token bucket algorithm to allow controlled burst traffic
/// while maintaining an average rate limit.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimiterConfig,
    /// Tokens currently available
    tokens: Arc<Mutex<f64>>,
    /// Last time tokens were updated
    last_update: Arc<Mutex<Instant>>,
    /// Semaphore for waiting when rate limited
    semaphore: Arc<Semaphore>,
    /// Current adaptive rate (requests per minute)
    adaptive_rate: Arc<Mutex<f64>>,
}

impl RateLimiter {
    /// Creates a new rate limiter with the given configuration.
    #[must_use]
    pub fn new(config: RateLimiterConfig) -> Self {
        let initial_tokens = f64::from(config.burst_capacity);
        let rate = f64::from(config.requests_per_minute);
        
        Self {
            config,
            tokens: Arc::new(Mutex::new(initial_tokens)),
            last_update: Arc::new(Mutex::new(Instant::now())),
            semaphore: Arc::new(Semaphore::new(config.burst_capacity)),
            adaptive_rate: Arc::new(Mutex::new(rate)),
        }
    }

    /// Acquires a permit to make an API call.
    ///
    /// This will block if the rate limit has been exceeded.
    ///
    /// # Errors
    ///
    /// Returns an error if the semaphore is closed.
    pub async fn acquire(&self) -> Result<RateLimitPermit> {
        // Refill tokens based on elapsed time
        self.refill_tokens().await;
        
        // Try to acquire a token
        let tokens = *self.tokens.lock().await;
        if tokens >= 1.0 {
            // We have tokens available
            *self.tokens.lock().await -= 1.0;
            let permit = self.semaphore.clone().acquire_owned().await
                .map_err(|_| crate::Error::RateLimited {
                    retry_after_ms: 1000,
                })?;
            return Ok(RateLimitPermit {
                permit,
                limiter: self.clone(),
            });
        }
        
        // Calculate wait time based on token refill rate
        let rate = *self.adaptive_rate.lock().await;
        let ms_per_token = 60_000.0 / rate; // ms per token at current rate
        let wait_ms = (1.0 - tokens) * ms_per_token;
        
        tracing::debug!(
            wait_ms = wait_ms as u64,
            current_tokens = tokens,
            rate = rate,
            "Rate limit reached, waiting"
        );
        
        // Wait for token to be available (minimum 1000ms for refill)
        sleep(Duration::from_millis(1000.max(wait_ms as u64))).await;
        
        // Try again after waiting
        self.refill_tokens().await;
        *self.tokens.lock().await -= 1.0;
        
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| crate::Error::RateLimited {
                retry_after_ms: 1000,
            })?;
        
        Ok(RateLimitPermit {
            permit,
            limiter: self.clone(),
        })
    }

    /// Tries to acquire a permit without blocking.
    ///
    /// Returns `None` if the rate limit has been exceeded.
    pub async fn try_acquire(&self) -> Option<RateLimitPermit> {
        self.refill_tokens().await;
        
        let mut tokens = self.tokens.lock().await;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            let permit = self.semaphore.clone().try_acquire_owned().ok()?;
            Some(RateLimitPermit {
                permit,
                limiter: self.clone(),
            })
        } else {
            None
        }
    }

    /// Notifies the rate limiter of a rate limit response (429).
    ///
    /// When adaptive rate limiting is enabled, this will reduce
    /// the rate to prevent future rate limits.
    pub async fn on_rate_limited(&self, retry_after_ms: Option<u64>) {
        if !self.config.adaptive {
            return;
        }
        
        let mut rate = self.adaptive_rate.lock().await;
        
        // Reduce rate by 50%
        *rate = (*rate * 0.5).max(10.0); // Minimum 10 requests per minute
        
        tracing::warn!(
            new_rate = *rate,
            retry_after_ms = retry_after_ms.unwrap_or(0),
            "Rate limit hit, reducing rate"
        );
        
        // If retry-after is provided, wait that long
        if let Some(ms) = retry_after_ms {
            sleep(Duration::from_millis(ms)).await;
        }
    }

    /// Notifies the rate limiter of a successful request.
    ///
    /// When adaptive rate limiting is enabled and we've been rate limited,
    /// this will gradually increase the rate back towards the configured limit.
    pub async fn on_success(&self) {
        if !self.config.adaptive {
            return;
        }
        
        let mut rate = self.adaptive_rate.lock().await;
        let max_rate = f64::from(self.config.requests_per_minute);
        
        // Increase rate by 10% if below max
        if *rate < max_rate {
            *rate = (*rate * 1.1).min(max_rate);
            tracing::debug!(
                new_rate = *rate,
                max_rate = max_rate,
                "Increasing rate after successful request"
            );
        }
    }

    /// Returns the current rate (requests per minute).
    pub async fn current_rate(&self) -> f64 {
        *self.adaptive_rate.lock().await
    }

    /// Returns the number of available tokens.
    pub async fn available_tokens(&self) -> f64 {
        self.refill_tokens().await;
        *self.tokens.lock().await
    }

    async fn refill_tokens(&self) {
        let mut tokens = self.tokens.lock().await;
        let mut last_update = self.last_update.lock().await;
        
        let now = Instant::now();
        let elapsed = now.duration_since(*last_update).as_secs_f64();
        
        // Calculate tokens to add based on rate
        let rate = *self.adaptive_rate.lock().await;
        let tokens_per_second = rate / 60.0;
        let tokens_to_add = elapsed * tokens_per_second;
        
        // Add tokens up to burst capacity
        *tokens = (*tokens + tokens_to_add).min(f64::from(self.config.burst_capacity));
        *last_update = now;
    }

    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            tokens: Arc::clone(&self.tokens),
            last_update: Arc::clone(&self.last_update),
            semaphore: Arc::clone(&self.semaphore),
            adaptive_rate: Arc::clone(&self.adaptive_rate),
        }
    }
}

/// A permit to make an API call.
///
/// When dropped, it notifies the rate limiter of completion.
#[must_use]
pub struct RateLimitPermit {
    /// The semaphore permit
    permit: OwnedSemaphorePermit,
    /// Reference to the rate limiter for tracking
    limiter: RateLimiter,
}

impl Drop for RateLimitPermit {
    fn drop(&mut self) {
        // Rate limiter is notified via semaphore release
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_rate_limiter_allows_burst() {
        let config = RateLimiterConfig {
            requests_per_minute: 60,
            burst_capacity: 5,
            adaptive: false,
        };
        let limiter = RateLimiter::new(config);
        
        // Should allow 5 burst requests immediately
        for _ in 0..5 {
            let permit = limiter.try_acquire().await;
            assert!(permit.is_some(), "Should acquire permit within burst capacity");
        }
        
        // 6th request should be denied
        let permit = limiter.try_acquire().await;
        assert!(permit.is_none(), "Should not acquire permit beyond burst capacity");
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_tokens() {
        let config = RateLimiterConfig {
            requests_per_minute: 60, // 1 per second
            burst_capacity: 1,
            adaptive: false,
        };
        let limiter = RateLimiter::new(config);
        
        // Use the token
        let _permit = limiter.acquire().await.expect("Should acquire initial permit");
        
        // Should be empty
        let tokens = limiter.available_tokens().await;
        assert!(tokens < 1.0, "Tokens should be depleted");
        
        // Wait for refill
        sleep(Duration::from_millis(1100)).await;
        
        // Should have refilled
        let tokens = limiter.available_tokens().await;
        assert!(tokens >= 1.0, "Tokens should be refilled");
    }

    #[tokio::test]
    async fn test_adaptive_rate_limiting() {
        let config = RateLimiterConfig {
            requests_per_minute: 60,
            burst_capacity: 5,
            adaptive: true,
        };
        let limiter = RateLimiter::new(config);
        
        let initial_rate = limiter.current_rate().await;
        assert_eq!(initial_rate, 60.0);
        
        // Simulate rate limit
        limiter.on_rate_limited(Some(1000)).await;
        
        let reduced_rate = limiter.current_rate().await;
        assert!(reduced_rate < initial_rate, "Rate should be reduced after rate limit");
        
        // Simulate successful requests
        for _ in 0..10 {
            limiter.on_success().await;
        }
        
        let recovered_rate = limiter.current_rate().await;
        assert!(recovered_rate > reduced_rate, "Rate should recover after successful requests");
    }

    #[tokio::test]
    async fn test_acquire_blocks_when_limited() {
        let config = RateLimiterConfig {
            requests_per_minute: 60,
            burst_capacity: 1,
            adaptive: false,
        };
        let limiter = RateLimiter::new(config);
        
        // Use the token
        let _permit = limiter.acquire().await.expect("Should acquire initial permit");
        
        // This should block until token is refilled
        let result = timeout(Duration::from_millis(2000), async {
            limiter.acquire().await
        }).await;
        
        assert!(result.is_ok(), "acquire should complete after waiting");
    }
}
