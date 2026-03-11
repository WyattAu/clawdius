//! Error recovery patterns for Nexus FSM
//!
//! Implements retry logic with exponential backoff, circuit breaker pattern,
//! and graceful degradation strategies.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::config::RecoveryConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<parking_lot::Mutex<CircuitState>>,
    failure_count: AtomicU64,
    last_failure: Arc<parking_lot::Mutex<Option<Instant>>>,
    config: RecoveryConfig,
}

impl CircuitBreaker {
    pub fn new(config: &RecoveryConfig) -> Self {
        Self {
            state: Arc::new(parking_lot::Mutex::new(CircuitState::Closed)),
            failure_count: AtomicU64::new(0),
            last_failure: Arc::new(parking_lot::Mutex::new(None)),
            config: config.clone(),
        }
    }

    pub fn state(&self) -> CircuitState {
        *self.state.lock()
    }

    pub fn is_available(&self) -> bool {
        let state = *self.state.lock();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = *self.last_failure.lock() {
                    if last.elapsed() >= Duration::from_millis(self.config.circuit_breaker_reset_ms)
                    {
                        let mut state = self.state.lock();
                        *state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        let mut state = self.state.lock();
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            self.failure_count.store(0, Ordering::Relaxed);
        }
    }

    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure.lock() = Some(Instant::now());

        let threshold = self.config.circuit_breaker_threshold as u64;
        if count >= threshold {
            let mut state = self.state.lock();
            *state = CircuitState::Open;
        }
    }

    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        *self.state.lock() = CircuitState::Closed;
        *self.last_failure.lock() = None;
    }
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl From<&RecoveryConfig> for RetryConfig {
    fn from(config: &RecoveryConfig) -> Self {
        Self {
            max_retries: config.max_retries,
            initial_delay: Duration::from_millis(config.initial_delay_ms),
            max_delay: Duration::from_millis(config.max_delay_ms),
            backoff_multiplier: config.backoff_multiplier,
        }
    }
}

pub struct RetryExecutor {
    config: RetryConfig,
    attempt: usize,
    current_delay: Duration,
}

impl RetryExecutor {
    pub fn new(config: RetryConfig) -> Self {
        let initial_delay = config.initial_delay;
        Self {
            config,
            attempt: 0,
            current_delay: initial_delay,
        }
    }

    pub fn attempt<T, E, F>(&mut self, mut f: F) -> std::result::Result<T, E>
    where
        F: FnMut() -> std::result::Result<T, E>,
    {
        loop {
            match f() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if self.attempt >= self.config.max_retries {
                        return Err(e);
                    }
                    self.attempt += 1;
                    std::thread::sleep(self.current_delay);
                    self.current_delay = std::cmp::min(
                        Duration::from_secs_f64(
                            self.current_delay.as_secs_f64() * self.config.backoff_multiplier,
                        ),
                        self.config.max_delay,
                    );
                }
            }
        }
    }

    pub async fn attempt_async<T, E, F, Fut>(&mut self, mut f: F) -> std::result::Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
    {
        loop {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if self.attempt >= self.config.max_retries {
                        return Err(e);
                    }
                    self.attempt += 1;
                    tokio::time::sleep(self.current_delay).await;
                    self.current_delay = std::cmp::min(
                        Duration::from_secs_f64(
                            self.current_delay.as_secs_f64() * self.config.backoff_multiplier,
                        ),
                        self.config.max_delay,
                    );
                }
            }
        }
    }

    pub fn attempt_count(&self) -> usize {
        self.attempt
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
        self.current_delay = self.config.initial_delay;
    }
}

#[derive(Debug)]
pub struct RecoveryManager {
    config: RecoveryConfig,
    circuit_breaker: CircuitBreaker,
}

impl RecoveryManager {
    pub fn new(config: &RecoveryConfig) -> Self {
        Self {
            circuit_breaker: CircuitBreaker::new(config),
            config: config.clone(),
        }
    }

    pub fn execute<T, E, F>(&self, f: F) -> std::result::Result<T, RecoveryError<E>>
    where
        F: Fn() -> std::result::Result<T, E>,
        E: std::fmt::Debug,
    {
        if !self.circuit_breaker.is_available() {
            return Err(RecoveryError::CircuitOpen);
        }

        let retry_config = RetryConfig::from(&self.config);
        let mut executor = RetryExecutor::new(retry_config);

        let result = executor.attempt(f);

        match &result {
            Ok(_) => {
                self.circuit_breaker.record_success();
            }
            Err(_) => {
                self.circuit_breaker.record_failure();
            }
        }

        result.map_err(|e| RecoveryError::OperationFailed(format!("{:?}", e)))
    }

    pub async fn execute_async<T, E, F, Fut>(
        &self,
        f: F,
    ) -> std::result::Result<T, RecoveryError<E>>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Debug,
    {
        if !self.circuit_breaker.is_available() {
            return Err(RecoveryError::CircuitOpen);
        }

        let retry_config = RetryConfig::from(&self.config);
        let mut executor = RetryExecutor::new(retry_config);

        let result = executor.attempt_async(f).await;

        match &result {
            Ok(_) => {
                self.circuit_breaker.record_success();
            }
            Err(_) => {
                self.circuit_breaker.record_failure();
            }
        }

        result.map_err(|e| RecoveryError::OperationFailed(format!("{:?}", e)))
    }

    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    pub fn reset(&self) {
        self.circuit_breaker.reset();
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new(&RecoveryConfig::default())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,

    #[error("Operation failed after retries: {0}")]
    OperationFailed(String),

    #[error("Operation failed: {0}")]
    Inner(#[from] E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DegradationLevel {
    Full,
    Degraded,
    Minimal,
}

impl Default for DegradationLevel {
    fn default() -> Self {
        Self::Full
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GracefulDegradation {
    level: Arc<parking_lot::Mutex<DegradationLevel>>,
    error_count: AtomicU64,
    recovery_threshold: u64,
}

impl GracefulDegradation {
    pub fn new(recovery_threshold: u64) -> Self {
        Self {
            level: Arc::new(parking_lot::Mutex::new(DegradationLevel::Full)),
            error_count: AtomicU64::new(0),
            recovery_threshold,
        }
    }

    pub fn level(&self) -> DegradationLevel {
        *self.level.lock()
    }

    pub fn record_error(&self) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
        let mut level = self.level.lock();

        *level = match count {
            0..=2 => DegradationLevel::Full,
            3..=5 => DegradationLevel::Degraded,
            _ => DegradationLevel::Minimal,
        };
    }

    pub fn record_success(&self) {
        let count = self.error_count.fetch_sub(1, Ordering::Relaxed);
        if count <= 1 {
            self.error_count.store(0, Ordering::Relaxed);
            *self.level.lock() = DegradationLevel::Full;
        }
    }

    pub fn should_perform_operation(&self, operation_type: OperationType) -> bool {
        let level = *self.level.lock();
        match level {
            DegradationLevel::Full => true,
            DegradationLevel::Degraded => {
                matches!(
                    operation_type,
                    OperationType::Critical | OperationType::Read
                )
            }
            DegradationLevel::Minimal => matches!(operation_type, OperationType::Critical),
        }
    }

    pub fn reset(&self) {
        self.error_count.store(0, Ordering::Relaxed);
        *self.level.lock() = DegradationLevel::Full;
    }
}

impl Default for GracefulDegradation {
    fn default() -> Self {
        Self::new(5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Critical,
    Read,
    Write,
    Optional,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_circuit_breaker_closed() {
        let config = RecoveryConfig {
            circuit_breaker_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(&config);

        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_available());

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_available());

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_available());
    }

    #[test]
    fn test_circuit_breaker_recovery() {
        let config = RecoveryConfig {
            circuit_breaker_threshold: 2,
            circuit_breaker_reset_ms: 10,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(&config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(15));
        assert!(cb.is_available());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_retry_executor_success() {
        let config = RetryConfig::default();
        let mut executor = RetryExecutor::new(config);

        let mut attempts = 0;
        let result = executor.attempt(|| {
            attempts += 1;
            if attempts < 2 {
                Err("temporary error")
            } else {
                Ok(42)
            }
        });

        assert_eq!(result, Ok(42));
        assert_eq!(executor.attempt_count(), 1);
    }

    #[test]
    fn test_retry_executor_max_retries() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_multiplier: 2.0,
        };
        let mut executor = RetryExecutor::new(config);

        let mut attempts = 0;
        let result: std::result::Result<i32, &str> = executor.attempt(|| {
            attempts += 1;
            Err("persistent error")
        });

        assert!(result.is_err());
        assert_eq!(attempts, 3);
    }

    #[test]
    fn test_graceful_degradation() {
        let gd = GracefulDegradation::new(5);

        assert_eq!(gd.level(), DegradationLevel::Full);
        assert!(gd.should_perform_operation(OperationType::Optional));

        for _ in 0..3 {
            gd.record_error();
        }

        assert_eq!(gd.level(), DegradationLevel::Degraded);
        assert!(gd.should_perform_operation(OperationType::Read));
        assert!(!gd.should_perform_operation(OperationType::Optional));

        for _ in 0..5 {
            gd.record_error();
        }

        assert_eq!(gd.level(), DegradationLevel::Minimal);
        assert!(gd.should_perform_operation(OperationType::Critical));
        assert!(!gd.should_perform_operation(OperationType::Read));
    }

    #[test]
    fn test_recovery_manager() {
        let config = RecoveryConfig {
            max_retries: 2,
            initial_delay_ms: 1,
            max_delay_ms: 10,
            backoff_multiplier: 2.0,
            circuit_breaker_threshold: 3,
            circuit_breaker_reset_ms: 100,
        };
        let manager = RecoveryManager::new(&config);

        let calls = AtomicUsize::new(0);
        let result = manager.execute(|| {
            let c = calls.fetch_add(1, Ordering::SeqCst) + 1;
            if c < 2 {
                Err("fail")
            } else {
                Ok(100)
            }
        });

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_retry_executor_async() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_multiplier: 2.0,
        };
        let mut executor = RetryExecutor::new(config);

        let attempts = AtomicUsize::new(0);
        let result = executor
            .attempt_async(|| {
                let attempts = &attempts;
                async move {
                    let a = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if a < 2 {
                        Err("temporary error")
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert_eq!(result, Ok(42));
    }
}
