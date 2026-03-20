//! Completion Cache
//!
//! Caches completion responses for improved performance.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::types::{CompletionRequest, CompletionResponse};

/// Cache entry with expiration.
#[derive(Debug, Clone)]
struct CacheEntry {
    response: CompletionResponse,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(response: CompletionResponse, ttl: Duration) -> Self {
        Self {
            response,
            created_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// Cache key for completion requests.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct CacheKey {
    /// Hash of the prefix
    prefix_hash: u64,
    /// Hash of the suffix
    suffix_hash: u64,
    /// Language
    language: String,
    /// Trigger type
    trigger: String,
}

impl CacheKey {
    fn from_request(request: &CompletionRequest) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash prefix
        request.prefix().hash(&mut hasher);
        let prefix_hash = hasher.finish();

        // Hash suffix
        hasher = std::collections::hash_map::DefaultHasher::new();
        request.suffix().hash(&mut hasher);
        let suffix_hash = hasher.finish();

        Self {
            prefix_hash,
            suffix_hash,
            language: request.language.clone(),
            trigger: format!("{:?}", request.trigger),
        }
    }
}

/// Completion cache configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_entries: usize,
    /// Time-to-live for entries
    pub ttl: Duration,
    /// Enable/disable caching
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
            enabled: true,
        }
    }
}

/// Completion cache.
#[derive(Debug, Clone)]
pub struct CompletionCache {
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    config: CacheConfig,
    /// Cache hit count
    hits: Arc<RwLock<u64>>,
    /// Cache miss count
    misses: Arc<RwLock<u64>>,
}

impl CompletionCache {
    /// Creates a new cache with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Creates a new cache with custom configuration.
    #[must_use]
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Gets a cached completion.
    pub async fn get(&self, request: &CompletionRequest) -> Option<CompletionResponse> {
        if !self.config.enabled {
            return None;
        }

        let key = CacheKey::from_request(request);
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(&key) {
            if entry.is_expired() {
                return None;
            }

            *self.hits.write().await += 1;

            let mut response = entry.response.clone();
            response.source = super::types::CompletionSource::Cache;
            return Some(response);
        }

        *self.misses.write().await += 1;
        None
    }

    /// Stores a completion in the cache.
    pub async fn put(&self, request: &CompletionRequest, response: CompletionResponse) {
        if !self.config.enabled {
            return;
        }

        let key = CacheKey::from_request(request);
        let entry = CacheEntry::new(response, self.config.ttl);

        let mut entries = self.entries.write().await;

        // Evict expired entries if at capacity
        if entries.len() >= self.config.max_entries {
            self.evict_expired(&mut entries);

            // If still at capacity, remove oldest
            if entries.len() >= self.config.max_entries {
                self.evict_oldest(&mut entries);
            }
        }

        entries.insert(key, entry);
    }

    /// Clears the cache.
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    /// Returns the number of entries.
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Returns true if the cache is empty.
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }

    /// Returns cache statistics.
    pub async fn stats(&self) -> CacheStats {
        let hits = *self.hits.read().await;
        let misses = *self.misses.read().await;
        let entries = self.entries.read().await.len();

        CacheStats {
            hits,
            misses,
            entries,
            hit_rate: if hits + misses > 0 {
                hits as f64 / (hits + misses) as f64
            } else {
                0.0
            },
        }
    }

    fn evict_expired(&self, entries: &mut HashMap<CacheKey, CacheEntry>) {
        entries.retain(|_, entry| !entry.is_expired());
    }

    fn evict_oldest(&self, entries: &mut HashMap<CacheKey, CacheEntry>) {
        if entries.is_empty() {
            return;
        }

        let oldest_key = entries
            .iter()
            .min_by_key(|(_, e)| e.created_at)
            .map(|(k, _)| k.clone());

        if let Some(key) = oldest_key {
            entries.remove(&key);
        }
    }
}

impl Default for CompletionCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of entries
    pub entries: usize,
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::Position;

    #[tokio::test]
    async fn test_cache_basic() {
        let cache = CompletionCache::new();
        let request = CompletionRequest::new("fn main() {", Position::new(0, 11), "rust");
        let response = CompletionResponse::new("let x = 5; }");

        cache.put(&request, response.clone()).await;

        let cached = cache.get(&request).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().text, "let x = 5; }");
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = CompletionCache::new();
        let request = CompletionRequest::new("test", Position::zero(), "text");
        let response = CompletionResponse::new("completion");

        cache.put(&request, response).await;

        // Hit
        let _ = cache.get(&request).await;

        // Miss
        let miss_request = CompletionRequest::new("different", Position::zero(), "text");
        let _ = cache.get(&miss_request).await;

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let config = CacheConfig {
            enabled: false,
            ..Default::default()
        };
        let cache = CompletionCache::with_config(config);
        let request = CompletionRequest::new("test", Position::zero(), "text");
        let response = CompletionResponse::new("completion");

        cache.put(&request, response).await;

        let cached = cache.get(&request).await;
        assert!(cached.is_none());
    }
}
