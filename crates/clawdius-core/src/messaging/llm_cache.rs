//! LLM Response Cache
//!
//! In-memory TTL cache for LLM completions. Deduplicates identical requests
//! within a configurable time window. Uses a content-addressed key (hash of
//! the message sequence) for cache lookups.
//!
//! # Feature Gate
//!
//! This module is always compiled but the cache is only active when explicitly
//! wired into the `LlmClient` chain via `LlmCache::wrap()`.
//!
//! # Usage
//!
//! ```ignore
//! let raw_client: Arc<dyn LlmClient> = Arc::new(MyClient::new(...));
//! let cached_client: Arc<dyn LlmClient> = LlmCache::new(raw_client)
//!     .with_ttl(Duration::from_secs(300))   // 5 min TTL
//!     .with_max_entries(1000)
//!     .wrap();
//! ```

#![deny(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::super::llm::ChatMessage;
use crate::llm::providers::LlmClient;
use crate::Result;

/// Configuration for the LLM response cache.
#[derive(Debug, Clone)]
pub struct LlmCacheConfig {
    /// Time-to-live for cached responses (default: 5 minutes).
    pub ttl: Duration,
    /// Maximum number of cached responses (default: 1000).
    pub max_entries: usize,
}

impl Default for LlmCacheConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(300),
            max_entries: 1000,
        }
    }
}

/// A single cached LLM response.
struct CacheEntry {
    response: String,
    created_at: Instant,
}

impl CacheEntry {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// In-memory TTL cache for LLM responses.
///
/// Thread-safe via `tokio::sync::RwLock`. When the cache is full,
/// the oldest expired entry is evicted; if none are expired, the
/// oldest entry is evicted regardless.
pub struct LlmCache {
    inner: Arc<dyn LlmClient>,
    config: LlmCacheConfig,
    entries: tokio::sync::RwLock<std::collections::HashMap<u64, CacheEntry>>,
    /// Running counter of cache hits (for metrics).
    hits: std::sync::atomic::AtomicU64,
    /// Running counter of cache misses.
    misses: std::sync::atomic::AtomicU64,
}

impl LlmCache {
    /// Create a new cache wrapping the given LLM client.
    #[must_use]
    pub fn new(inner: Arc<dyn LlmClient>) -> Self {
        Self {
            inner,
            config: LlmCacheConfig::default(),
            entries: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Set the cache TTL.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.config.ttl = ttl;
        self
    }

    /// Set the maximum number of cached entries.
    #[must_use]
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.config.max_entries = max_entries;
        self
    }

    /// Wrap this cache into an `Arc<dyn LlmClient>` for use anywhere
    /// an `LlmClient` is expected.
    #[must_use]
    pub fn wrap(self) -> Arc<dyn LlmClient> {
        Arc::new(self)
    }

    /// Compute a content-addressed cache key from a message sequence.
    fn cache_key(messages: &[ChatMessage]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for msg in messages {
            hasher.write_u8(msg.role as u8);
            hasher.write(msg.content.as_bytes());
        }
        hasher.finish()
    }

    /// Evict expired entries and, if still over capacity, the oldest entry.
    async fn maybe_evict(&self) {
        let mut entries = self.entries.write().await;
        // Remove expired
        entries.retain(|_, entry| !entry.is_expired(self.config.ttl));

        // If still over capacity, remove oldest
        if entries.len() > self.config.max_entries {
            if let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(k, _)| *k)
            {
                entries.remove(&oldest_key);
            }
        }
    }

    /// Cache hit count (for metrics).
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Cache miss count (for metrics).
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Current number of entries in the cache.
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Clear all cached entries.
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }
}

#[async_trait]
impl LlmClient for LlmCache {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let key = Self::cache_key(&messages);

        // Check cache
        {
            let entries = self.entries.read().await;
            if let Some(entry) = entries.get(&key) {
                if !entry.is_expired(self.config.ttl) {
                    self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(entry.response.clone());
                }
            }
        }

        // Cache miss — call inner client
        self.misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let response = self.inner.chat(messages).await?;

        // Store in cache
        {
            let mut entries = self.entries.write().await;
            entries.insert(
                key,
                CacheEntry {
                    response: response.clone(),
                    created_at: Instant::now(),
                },
            );
        }

        // Evict if needed (async, non-blocking)
        self.maybe_evict().await;

        Ok(response)
    }

    async fn chat_stream(&self, messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
        // Streaming responses are not cached — pass through directly.
        self.inner.chat_stream(messages).await
    }

    fn count_tokens(&self, text: &str) -> usize {
        self.inner.count_tokens(text)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock LLM client that returns a predictable response.
    struct MockClient {
        response: String,
        call_count: std::sync::atomic::AtomicU64,
    }

    impl MockClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                call_count: std::sync::atomic::AtomicU64::new(0),
            }
        }

        fn calls(&self) -> u64 {
            self.call_count.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl LlmClient for MockClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self.response.clone())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send(self.response.clone()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.len()
        }
    }

    fn test_messages(content: &str) -> Vec<ChatMessage> {
        vec![ChatMessage {
            role: crate::llm::ChatRole::User,
            content: content.to_string(),
        }]
    }

    #[tokio::test]
    async fn cache_returns_cached_response() {
        let mock = Arc::new(MockClient::new("cached response"));
        let cache = LlmCache::new(mock.clone())
            .with_ttl(Duration::from_secs(60))
            .wrap();

        let msgs = test_messages("hello");
        let r1 = cache.chat(msgs.clone()).await.unwrap();
        let r2 = cache.chat(msgs.clone()).await.unwrap();

        assert_eq!(r1, "cached response");
        assert_eq!(r2, "cached response");
        assert_eq!(mock.calls(), 1); // Only called once — second was cached
    }

    #[tokio::test]
    async fn cache_miss_for_different_messages() {
        let mock = Arc::new(MockClient::new("response"));
        let cache = LlmCache::new(mock.clone())
            .with_ttl(Duration::from_secs(60))
            .wrap();

        let r1 = cache.chat(test_messages("msg1")).await.unwrap();
        let r2 = cache.chat(test_messages("msg2")).await.unwrap();

        assert_eq!(r1, "response");
        assert_eq!(r2, "response");
        assert_eq!(mock.calls(), 2); // Different messages → two calls
    }

    #[tokio::test]
    async fn cache_hits_misses_counters() {
        let mock = Arc::new(MockClient::new("ok"));
        // Access the inner cache for metrics
        let cache = LlmCache::new(mock.clone()).with_ttl(Duration::from_secs(60));

        let client: Arc<dyn LlmClient> = cache.wrap();

        // We can't access hits/misses through the dyn trait, so test via
        // call count (misses = calls, hits = total - misses).
        let _ = client.chat(test_messages("a")).await.unwrap();
        let _ = client.chat(test_messages("a")).await.unwrap(); // hit
        let _ = client.chat(test_messages("a")).await.unwrap(); // hit
        let _ = client.chat(test_messages("b")).await.unwrap(); // miss

        assert_eq!(mock.calls(), 2); // 2 misses (a, b) → 2 actual LLM calls
    }

    #[tokio::test]
    async fn cache_eviction_on_max_entries() {
        let mock = Arc::new(MockClient::new("ok"));
        let cache = LlmCache::new(mock.clone())
            .with_ttl(Duration::from_secs(60))
            .with_max_entries(3);

        // Fill cache with 4 distinct messages (exceeds max of 3)
        for i in 0..4 {
            let _ = cache
                .chat(test_messages(&format!("msg-{i}")))
                .await
                .unwrap();
        }

        assert_eq!(cache.entry_count().await, 3);
    }

    #[tokio::test]
    async fn cache_expired_entries_are_not_returned() {
        let mock = Arc::new(MockClient::new("fresh"));
        let cache = LlmCache::new(mock.clone())
            .with_ttl(Duration::from_millis(50))
            .wrap();

        let msgs = test_messages("expiring");

        let _ = cache.chat(msgs.clone()).await.unwrap();
        assert_eq!(mock.calls(), 1);

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = cache.chat(msgs.clone()).await.unwrap();
        assert_eq!(mock.calls(), 2); // Expired → re-fetched
    }

    #[tokio::test]
    async fn streaming_passthrough_not_cached() {
        let mock = Arc::new(MockClient::new("streamed"));
        let cache = LlmCache::new(mock.clone()).wrap();

        let _ = cache.chat_stream(test_messages("stream")).await.unwrap();
        assert_eq!(mock.calls(), 0); // chat_stream doesn't increment call_count
    }

    #[test]
    fn cache_key_is_deterministic() {
        let msgs1 = test_messages("hello");
        let msgs2 = test_messages("hello");
        assert_eq!(LlmCache::cache_key(&msgs1), LlmCache::cache_key(&msgs2));
    }

    #[test]
    fn cache_key_differs_for_different_content() {
        let msgs1 = test_messages("hello");
        let msgs2 = test_messages("world");
        assert_ne!(LlmCache::cache_key(&msgs1), LlmCache::cache_key(&msgs2));
    }

    #[tokio::test]
    async fn clear_empties_cache() {
        let mock = Arc::new(MockClient::new("ok"));
        let cache = LlmCache::new(mock.clone()).with_ttl(Duration::from_secs(60));

        let _ = cache.chat(test_messages("a")).await.unwrap();
        assert_eq!(cache.entry_count().await, 1);

        cache.clear().await;
        assert_eq!(cache.entry_count().await, 0);
    }
}
