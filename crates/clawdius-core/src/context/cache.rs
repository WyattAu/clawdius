//! Context caching for frequently accessed items

use std::collections::HashMap;
use std::time::{Duration, Instant};

use blake3::Hash;

use super::ContextItem;

/// Cached context item with metadata
#[derive(Debug, Clone)]
pub struct CachedContext {
    /// The cached item
    pub item: ContextItem,
    /// When it was cached
    pub cached_at: Instant,
    /// Content hash for invalidation
    pub hash: Hash,
}

impl CachedContext {
    /// Check if cache entry is expired
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }
}

/// Context cache
pub struct ContextCache {
    entries: HashMap<String, CachedContext>,
    ttl: Duration,
    max_entries: usize,
}

impl ContextCache {
    /// Create a new context cache
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            max_entries,
        }
    }

    /// Get a cached item
    pub fn get(&self, key: &str) -> Option<&ContextItem> {
        self.entries
            .get(key)
            .filter(|cached| !cached.is_expired(self.ttl))
            .map(|cached| &cached.item)
    }

    /// Put an item in the cache
    pub fn put(&mut self, key: String, item: ContextItem) {
        // Evict old entries if full
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        let hash = Self::hash_item(&item);

        self.entries.insert(
            key,
            CachedContext {
                item,
                cached_at: Instant::now(),
                hash,
            },
        );
    }

    /// Invalidate a cached item
    pub fn invalidate(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Invalidate all items matching a pattern
    pub fn invalidate_pattern(&mut self, pattern: &str) {
        self.entries.retain(|key, _| !key.starts_with(pattern));
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let expired = self
            .entries
            .values()
            .filter(|c| c.is_expired(self.ttl))
            .count();

        CacheStats {
            total_entries: self.entries.len(),
            expired_entries: expired,
            max_entries: self.max_entries,
        }
    }

    fn evict_oldest(&mut self) {
        if let Some((oldest_key, _)) = self
            .entries
            .iter()
            .min_by_key(|(_, cached)| cached.cached_at)
        {
            let key = oldest_key.clone();
            self.entries.remove(&key);
        }
    }

    fn hash_item(item: &ContextItem) -> Hash {
        match item {
            ContextItem::File { content, .. } => blake3::hash(content.as_bytes()),
            ContextItem::Folder { files, .. } => {
                let joined = files.join(",");
                blake3::hash(joined.as_bytes())
            }
            ContextItem::Url { content, .. } => blake3::hash(content.as_bytes()),
            ContextItem::Problems { diagnostics } => {
                let msgs: Vec<String> = diagnostics
                    .iter()
                    .map(|d| format!("{}:{}:{}", d.file, d.line, d.message))
                    .collect();
                let joined = msgs.join("|");
                blake3::hash(joined.as_bytes())
            }
            ContextItem::GitDiff { diff, .. } => blake3::hash(diff.as_bytes()),
            ContextItem::GitLog { commits } => {
                let msgs: Vec<String> = commits
                    .iter()
                    .map(|c| format!("{}:{}", c.hash, c.message))
                    .collect();
                let joined = msgs.join("|");
                blake3::hash(joined.as_bytes())
            }
            ContextItem::Symbol { content, .. } => blake3::hash(content.as_bytes()),
            ContextItem::Search { results, .. } => {
                let msgs: Vec<String> = results
                    .iter()
                    .map(|r| format!("{}:{}", r.file, r.content))
                    .collect();
                let joined = msgs.join("|");
                blake3::hash(joined.as_bytes())
            }
        }
    }
}

impl Default for ContextCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(300), 100)
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total entries
    pub total_entries: usize,
    /// Expired entries
    pub expired_entries: usize,
    /// Maximum entries
    pub max_entries: usize,
}
