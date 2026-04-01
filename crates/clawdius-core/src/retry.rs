//! Retry logic and circuit breaker for transient error recovery

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::Error;
use crate::Result;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Circuit breaker for protecting against cascading failures
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Service name
    service: String,
    /// Number of failures before opening circuit
    failure_threshold: u64,
    /// Time to wait before attempting recovery (milliseconds)
    recovery_timeout_ms: u64,
    /// Current state
    state: Arc<RwLock<CircuitState>>,
    /// Failure count
    failure_count: AtomicU64,
    /// Last failure time
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    /// Last error message
    last_error: Arc<RwLock<String>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(
        service: impl Into<String>,
        failure_threshold: u64,
        recovery_timeout_ms: u64,
    ) -> Self {
        Self {
            service: service.into(),
            failure_threshold,
            recovery_timeout_ms,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU64::new(0),
            last_failure_time: Arc::new(RwLock::new(None)),
            last_error: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Check if requests are allowed
    pub async fn is_allowed(&self) -> Result<()> {
        let state = self.state.read().await;

        match *state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                let last_failure = self.last_failure_time.read().await;
                if let Some(time) = *last_failure {
                    let elapsed = time.elapsed().as_millis() as u64;
                    if elapsed >= self.recovery_timeout_ms {
                        drop(last_failure);
                        drop(state);

                        let mut state = self.state.write().await;
                        *state = CircuitState::HalfOpen;
                        debug!(
                            service = %self.service,
                            "Circuit breaker entering half-open state"
                        );
                        Ok(())
                    } else {
                        Err(Error::CircuitBreakerOpen {
                            service: self.service.clone(),
                            last_error: self.last_error.read().await.clone(),
                        })
                    }
                } else {
                    Err(Error::CircuitBreakerOpen {
                        service: self.service.clone(),
                        last_error: self.last_error.read().await.clone(),
                    })
                }
            },
            CircuitState::HalfOpen => Ok(()),
        }
    }

    /// Record a success
    pub async fn record_success(&self) {
        let count = self.failure_count.load(Ordering::Relaxed);
        if count > 0 {
            self.failure_count.store(0, Ordering::Relaxed);
            let mut state = self.state.write().await;
            *state = CircuitState::Closed;
            debug!(
                service = %self.service,
                "Circuit breaker reset to closed state"
            );
        }
    }

    /// Record a failure
    pub async fn record_failure(&self, error: &Error) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        *self.last_error.write().await = error.to_string();
        *self.last_failure_time.write().await = Some(Instant::now());

        if count >= self.failure_threshold {
            let mut state = self.state.write().await;
            if *state != CircuitState::Open {
                *state = CircuitState::Open;
                warn!(
                    service = %self.service,
                    failures = count,
                    threshold = self.failure_threshold,
                    "Circuit breaker opened"
                );
            }
        }
    }

    /// Get current state
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Execute an async function with retry logic and optional circuit breaker
pub async fn with_retry_and_circuit<T, F, Fut>(
    max_retries: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    exponential_base: f64,
    circuit_breaker: Option<&CircuitBreaker>,
    mut f: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = initial_delay_ms;
    let mut attempts = 0u32;

    loop {
        attempts += 1;

        if let Some(cb) = &circuit_breaker {
            cb.is_allowed().await?;
        }

        match f().await {
            Ok(result) => {
                if attempts > 1 {
                    debug!("Operation succeeded on attempt {}", attempts);
                }
                if let Some(cb) = &circuit_breaker {
                    cb.record_success().await;
                }
                return Ok(result);
            },
            Err(e) => {
                if let Some(cb) = &circuit_breaker {
                    cb.record_failure(&e).await;
                }

                if !e.is_retryable() || attempts > max_retries {
                    return Err(e);
                }

                let retry_delay = e.retry_after_ms().unwrap_or(delay);
                warn!(
                    attempt = attempts,
                    max_attempts = max_retries,
                    delay_ms = retry_delay,
                    error = %e,
                    "Operation failed, retrying"
                );

                tokio::time::sleep(Duration::from_millis(retry_delay)).await;

                delay = ((delay as f64) * exponential_base).min(max_delay_ms as f64) as u64;
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed() {
        let cb = CircuitBreaker::new("test", 3, 1000);
        assert!(cb.is_allowed().await.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let cb = CircuitBreaker::new("test", 2, 1000);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert!(cb.is_allowed().await.is_ok());

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert!(matches!(
            cb.is_allowed().await,
            Err(Error::CircuitBreakerOpen { .. })
        ));
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let cb = CircuitBreaker::new("test", 1, 100);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert!(matches!(cb.state().await, CircuitState::Open));

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(cb.is_allowed().await.is_ok());
        assert!(matches!(cb.state().await, CircuitState::HalfOpen));

        cb.record_success().await;
        assert!(matches!(cb.state().await, CircuitState::Closed));
    }
}
