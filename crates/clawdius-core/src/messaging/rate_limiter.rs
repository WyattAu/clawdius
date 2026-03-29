//! Token Bucket Rate Limiter
//!
//! Implements a token bucket algorithm for rate limiting messaging requests.
//! ALG-RATE-001: O(1) time complexity for check and consume operations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};

use super::state_store::StateStore;
use super::types::{MessagingError, RateLimitConfig, Result};

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

#[derive(Serialize, Deserialize)]
struct StoredBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill_epoch_secs: f64,
}

impl StoredBucket {
    fn from_bucket(bucket: &TokenBucket) -> Self {
        let last_refill_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            - bucket.last_refill.elapsed().as_secs_f64();
        Self {
            tokens: bucket.tokens,
            max_tokens: bucket.max_tokens,
            refill_rate: bucket.refill_rate,
            last_refill_epoch_secs,
        }
    }

    fn into_bucket(self) -> TokenBucket {
        let last_refill_epoch = UNIX_EPOCH + Duration::from_secs_f64(self.last_refill_epoch_secs);
        let now_instant = Instant::now();
        let sys_now = SystemTime::now();

        let elapsed_since_store = sys_now
            .duration_since(last_refill_epoch)
            .unwrap_or_default();
        let last_refill_instant = now_instant - elapsed_since_store;

        let elapsed_secs = elapsed_since_store.as_secs_f64();
        let refilled_tokens = (self.tokens + elapsed_secs * self.refill_rate).min(self.max_tokens);

        TokenBucket {
            tokens: refilled_tokens,
            max_tokens: self.max_tokens,
            refill_rate: self.refill_rate,
            last_refill: last_refill_instant,
        }
    }
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        Self {
            tokens: f64::from(config.burst_capacity),
            max_tokens: f64::from(config.burst_capacity),
            refill_rate: f64::from(config.requests_per_minute) / 60.0,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
        self.last_refill = now;
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn time_until_available(&self, tokens: f64) -> Duration {
        if self.tokens >= tokens {
            Duration::ZERO
        } else {
            let needed = tokens - self.tokens;
            let secs = needed / self.refill_rate;
            Duration::from_secs_f64(secs)
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
    db_path: Option<String>,
    pub state_store: Option<Arc<dyn StateStore>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
            db_path: None,
            state_store: None,
        }
    }

    pub fn with_persistence(db_path: impl Into<String>, config: RateLimitConfig) -> Self {
        let path = db_path.into();
        Self::init_db(&path);
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
            db_path: Some(path),
            state_store: None,
        }
    }

    pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.state_store = Some(store);
        self
    }

    pub fn has_persistence(&self) -> bool {
        self.db_path.is_some()
    }

    pub async fn load_from_db(&self) -> usize {
        if let Some(store) = &self.state_store {
            let keys = match store.keys("rate_limits", "*").await {
                Ok(k) => k,
                Err(_) => return 0,
            };
            let count = keys.len();
            let mut buckets = self.buckets.write().await;
            for key in &keys {
                if let Ok(Some(value)) = store.get("rate_limits", key).await {
                    if let Ok(stored) = serde_json::from_slice::<StoredBucket>(&value) {
                        buckets.entry(key.clone()).or_insert(stored.into_bucket());
                    }
                }
            }
            return count;
        }

        let Some(ref path) = self.db_path else {
            return 0;
        };
        let path = path.clone();
        let refill_rate = f64::from(self.config.requests_per_minute) / 60.0;
        let max_tokens = f64::from(self.config.burst_capacity);

        let result = tokio::task::spawn_blocking(move || {
            let conn = match rusqlite::Connection::open(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to open rate limiter DB");
                    return Vec::new();
                }
            };

            let mut stmt = match conn
                .prepare("SELECT key, tokens, last_refill_epoch_secs FROM rate_limit_buckets")
            {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };

            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            });

            let mut loaded = Vec::new();
            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    let (key, tokens, epoch_secs) = row;
                    let last_refill_epoch = UNIX_EPOCH + Duration::from_secs_f64(epoch_secs);
                    let now_instant = Instant::now();
                    let sys_now = SystemTime::now();

                    let elapsed_since_store = sys_now
                        .duration_since(last_refill_epoch)
                        .unwrap_or_default();
                    let last_refill_instant = now_instant - elapsed_since_store;

                    let elapsed_secs = elapsed_since_store.as_secs_f64();
                    let refilled_tokens = (tokens + elapsed_secs * refill_rate).min(max_tokens);

                    loaded.push((
                        key,
                        TokenBucket {
                            tokens: refilled_tokens,
                            max_tokens,
                            refill_rate,
                            last_refill: last_refill_instant,
                        },
                    ));
                }
            }
            loaded
        })
        .await;

        match result {
            Ok(loaded) => {
                let count = loaded.len();
                let mut buckets = self.buckets.write().await;
                for (key, bucket) in loaded {
                    buckets.entry(key).or_insert(bucket);
                }
                count
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load rate limits from DB");
                0
            }
        }
    }

    fn init_db(path: &str) {
        let conn = rusqlite::Connection::open(path).expect("Failed to open rate limiter database");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rate_limit_buckets (
                key                     TEXT PRIMARY KEY,
                tokens                  REAL NOT NULL,
                last_refill_epoch_secs  REAL NOT NULL
            )",
            [],
        )
        .expect("Failed to create rate limit table");
    }

    async fn persist_bucket_db(&self, key: &str, bucket: &TokenBucket) {
        if let Some(store) = &self.state_store {
            let key = key.to_string();
            let stored = StoredBucket::from_bucket(bucket);
            let value = serde_json::to_vec(&stored).unwrap_or_default();
            let _ = store.set("rate_limits", &key, &value, None).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let key = key.to_string();
        let tokens = bucket.tokens;
        let epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "INSERT OR REPLACE INTO rate_limit_buckets (key, tokens, last_refill_epoch_secs) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, tokens, epoch_secs],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to persist rate limit bucket"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for rate limit persist"),
        }
    }

    async fn delete_bucket_db(&self, key: &str) {
        if let Some(store) = &self.state_store {
            let _ = store.delete("rate_limits", key).await;
            return;
        }

        let Some(ref path) = self.db_path else {
            return;
        };
        let path = path.clone();
        let key = key.to_string();

        match tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute(
                "DELETE FROM rate_limit_buckets WHERE key = ?1",
                rusqlite::params![key],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "Failed to delete rate limit bucket from DB"),
            Err(e) => tracing::warn!(error = %e, "spawn_blocking failed for rate limit delete"),
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<()> {
        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));

        if bucket.try_consume(1.0) {
            let bucket = bucket.clone();
            drop(buckets);
            self.persist_bucket_db(key, &bucket).await;
            Ok(())
        } else {
            let retry_after = bucket.time_until_available(1.0);
            let bucket = bucket.clone();
            drop(buckets);
            self.persist_bucket_db(key, &bucket).await;
            Err(MessagingError::RateLimited {
                retry_after_secs: retry_after.as_secs(),
            })
        }
    }

    pub async fn try_consume(&self, key: &str, tokens: u32) -> Result<()> {
        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));

        let tokens_f64 = f64::from(tokens);
        if bucket.try_consume(tokens_f64) {
            let bucket = bucket.clone();
            drop(buckets);
            self.persist_bucket_db(key, &bucket).await;
            Ok(())
        } else {
            let retry_after = bucket.time_until_available(tokens_f64);
            let bucket = bucket.clone();
            drop(buckets);
            self.persist_bucket_db(key, &bucket).await;
            Err(MessagingError::RateLimited {
                retry_after_secs: retry_after.as_secs(),
            })
        }
    }

    pub async fn cleanup_inactive(&self, max_age: Duration) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        let removed: Vec<String> = buckets
            .iter()
            .filter(|(_, bucket)| now.duration_since(bucket.last_refill) >= max_age)
            .map(|(k, _)| k.clone())
            .collect();
        buckets.retain(|_, bucket| now.duration_since(bucket.last_refill) < max_age);
        drop(buckets);

        for key in &removed {
            self.delete_bucket_db(key).await;
        }
    }

    pub async fn bucket_count(&self) -> usize {
        self.buckets.read().await.len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 10,
            tokens_per_refill: 1,
            refill_interval_ms: 1000,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..5 {
            assert!(limiter.check_rate_limit("user1").await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 3,
            tokens_per_refill: 1,
            refill_interval_ms: 100,
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user1").await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_refills() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 2,
            tokens_per_refill: 1,
            refill_interval_ms: 100,
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user1").await.is_err());

        tokio::time::sleep(Duration::from_millis(1500)).await;

        assert!(limiter.check_rate_limit("user1").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_per_user() {
        let limiter = RateLimiter::default();

        assert!(limiter.check_rate_limit("user1").await.is_ok());
        assert!(limiter.check_rate_limit("user2").await.is_ok());

        assert_eq!(limiter.bucket_count().await, 2);
    }

    #[tokio::test]
    async fn test_no_persistence() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        assert!(!limiter.has_persistence());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rate_limits.db");
        let path_str = path.to_str().unwrap().to_string();

        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 5,
            tokens_per_refill: 1,
            refill_interval_ms: 1000,
        };

        let limiter = RateLimiter::with_persistence(&path_str, config);
        assert!(limiter.has_persistence());

        for _ in 0..3 {
            limiter.check_rate_limit("persist-user").await.unwrap();
        }

        limiter.check_rate_limit("persist-user2").await.unwrap();

        drop(limiter);

        let path_check = path_str.clone();
        let db_count = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path_check).unwrap();
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM rate_limit_buckets", [], |r| r.get(0))
                .unwrap();
            count
        })
        .await
        .unwrap();
        assert_eq!(db_count, 2, "DB should have 2 buckets, has {}", db_count);

        let config2 = RateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 5,
            tokens_per_refill: 1,
            refill_interval_ms: 1000,
        };
        let limiter2 = RateLimiter::with_persistence(&path_str, config2);
        let loaded_count = limiter2.load_from_db().await;
        assert_eq!(loaded_count, 2);

        assert_eq!(limiter2.bucket_count().await, 2);

        limiter2.check_rate_limit("persist-user").await.unwrap();
        limiter2.check_rate_limit("persist-user").await.unwrap();
        assert!(limiter2.check_rate_limit("persist-user").await.is_err());
    }
}
