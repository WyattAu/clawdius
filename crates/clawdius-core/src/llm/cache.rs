use crate::llm::{ChatMessage, ChatRole, LlmResponse};
use blake3::Hasher;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct LlmCacheKey {
    hash: [u8; 32],
    message_count: usize,
}

#[derive(Debug)]
pub struct LlmCacheEntry {
    pub response: LlmResponse,
    pub created_at: Instant,
    pub hit_count: AtomicU32,
}

impl Clone for LlmCacheEntry {
    fn clone(&self) -> Self {
        Self {
            response: self.response.clone(),
            created_at: self.created_at,
            hit_count: AtomicU32::new(self.hit_count.load(Ordering::Relaxed)),
        }
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Clone for CacheStats {
    fn clone(&self) -> Self {
        Self {
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            misses: AtomicU64::new(self.misses.load(Ordering::Relaxed)),
            evictions: AtomicU64::new(self.evictions.load(Ordering::Relaxed)),
        }
    }
}

pub struct LlmResponseCache {
    entries: RwLock<HashMap<LlmCacheKey, LlmCacheEntry>>,
    ttl: Duration,
    max_entries: usize,
    stats: CacheStats,
}

impl LlmResponseCache {
    #[must_use] 
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
            max_entries,
            stats: CacheStats::default(),
        }
    }

    pub fn get(&self, messages: &[ChatMessage]) -> Option<LlmResponse> {
        let key = compute_cache_key(messages);
        let mut entries = self.entries.write().unwrap_or_else(|e| { tracing::error!("RwLock poisoned in llm cache: {}", e); e.into_inner() });

        let entry = if let Some(e) = entries.get(&key) { e } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        };

        if entry.created_at.elapsed() > self.ttl {
            entries.remove(&key);
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        entry.hit_count.fetch_add(1, Ordering::Relaxed);
        self.stats.hits.fetch_add(1, Ordering::Relaxed);
        Some(entry.response.clone())
    }

    pub fn insert(&self, messages: &[ChatMessage], response: LlmResponse) {
        let key = compute_cache_key(messages);
        let mut entries = self.entries.write().unwrap_or_else(|e| { tracing::error!("RwLock poisoned in llm cache: {}", e); e.into_inner() });

        if self.max_entries > 0 && entries.len() >= self.max_entries && !entries.contains_key(&key)
        {
            evict_oldest(&mut entries, &self.stats);
        }

        entries.insert(
            key,
            LlmCacheEntry {
                response,
                created_at: Instant::now(),
                hit_count: AtomicU32::new(0),
            },
        );
    }

    pub fn invalidate(&self, messages: &[ChatMessage]) -> bool {
        let key = compute_cache_key(messages);
        let mut entries = self.entries.write().unwrap_or_else(|e| { tracing::error!("RwLock poisoned in llm cache: {}", e); e.into_inner() });
        entries.remove(&key).is_some()
    }

    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap_or_else(|e| { tracing::error!("RwLock poisoned in llm cache: {}", e); e.into_inner() });
        entries.clear();
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.read().unwrap_or_else(|e| { tracing::error!("RwLock poisoned in llm cache: {}", e); e.into_inner() });
        entries.len()
    }

    pub fn stats(&self) -> CacheStats {
        self.stats.clone()
    }
}

const fn role_as_str(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

fn compute_cache_key(messages: &[ChatMessage]) -> LlmCacheKey {
    let mut hasher = Hasher::new();
    for msg in messages {
        hasher.update(role_as_str(msg.role).as_bytes());
        hasher.update(msg.content.as_bytes());
        hasher.update(b"\x00");
    }
    let hash = hasher.finalize();
    LlmCacheKey {
        hash: *hash.as_bytes(),
        message_count: messages.len(),
    }
}

fn evict_oldest(entries: &mut HashMap<LlmCacheKey, LlmCacheEntry>, stats: &CacheStats) {
    if let Some(oldest_key) = entries
        .iter()
        .min_by_key(|(_, entry)| entry.created_at)
        .map(|(k, _)| k.clone())
    {
        entries.remove(&oldest_key);
        stats.evictions.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(content: &str) -> Vec<ChatMessage> {
        vec![ChatMessage {
            role: ChatRole::User,
            content: content.to_string(),
        }]
    }

    fn make_response(text: &str) -> LlmResponse {
        LlmResponse {
            text: text.to_string(),
            usage: TokenUsage {
                input: 10,
                output: 20,
                cached: 0,
            },
            tool_calls: vec![],
        }
    }

    #[test]
    fn test_new_cache() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats().hit_rate(), 0.0);
    }

    #[test]
    fn test_insert_and_get() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        let messages = make_messages("hello");
        let response = make_response("world");

        cache.insert(&messages, response.clone());

        let cached = cache.get(&messages).unwrap();
        assert_eq!(cached.text, "world");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_miss() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        let messages = make_messages("hello");
        cache.insert(&messages, make_response("world"));

        let different = make_messages("different");
        let result = cache.get(&different);
        assert!(result.is_none());
    }

    #[test]
    fn test_ttl_expiry() {
        let cache = LlmResponseCache::new(Duration::from_millis(10), 100);
        let messages = make_messages("hello");
        cache.insert(&messages, make_response("world"));

        std::thread::sleep(Duration::from_millis(20));

        let result = cache.get(&messages);
        assert!(result.is_none());
    }

    #[test]
    fn test_max_entries_eviction() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 2);

        cache.insert(&make_messages("msg1"), make_response("resp1"));
        cache.insert(&make_messages("msg2"), make_response("resp2"));
        assert_eq!(cache.len(), 2);

        cache.insert(&make_messages("msg3"), make_response("resp3"));
        assert_eq!(cache.len(), 2);

        let result = cache.get(&make_messages("msg1"));
        assert!(result.is_none(), "oldest entry should have been evicted");

        let result = cache.get(&make_messages("msg3")).unwrap();
        assert_eq!(result.text, "resp3");

        let stats = cache.stats();
        assert_eq!(stats.evictions.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_invalidate() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        let messages = make_messages("hello");
        cache.insert(&messages, make_response("world"));
        assert_eq!(cache.len(), 1);

        let removed = cache.invalidate(&messages);
        assert!(removed);
        assert_eq!(cache.len(), 0);

        let removed_again = cache.invalidate(&messages);
        assert!(!removed_again);
    }

    #[test]
    fn test_clear() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        cache.insert(&make_messages("a"), make_response("1"));
        cache.insert(&make_messages("b"), make_response("2"));
        cache.insert(&make_messages("c"), make_response("3"));
        assert_eq!(cache.len(), 3);

        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        let messages = make_messages("hello");
        cache.insert(&messages, make_response("world"));

        assert_eq!(cache.stats().hits.load(Ordering::Relaxed), 0);
        assert_eq!(cache.stats().misses.load(Ordering::Relaxed), 0);

        cache.get(&messages).unwrap();
        assert_eq!(cache.stats().hits.load(Ordering::Relaxed), 1);

        cache.get(&make_messages("missing"));
        assert_eq!(cache.stats().misses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_hit_rate() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);
        let messages = make_messages("hello");
        cache.insert(&messages, make_response("world"));

        cache.get(&messages).unwrap();
        cache.get(&messages).unwrap();
        cache.get(&make_messages("missing"));

        let stats = cache.stats();
        assert!((stats.hit_rate() - 2.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_different_messages_different_keys() {
        let cache = LlmResponseCache::new(Duration::from_secs(60), 100);

        let user_msg = vec![ChatMessage {
            role: ChatRole::User,
            content: "hello".to_string(),
        }];
        let system_msg = vec![ChatMessage {
            role: ChatRole::System,
            content: "hello".to_string(),
        }];

        cache.insert(&user_msg, make_response("user response"));
        cache.insert(&system_msg, make_response("system response"));

        assert_eq!(cache.len(), 2);

        let user_resp = cache.get(&user_msg).unwrap();
        assert_eq!(user_resp.text, "user response");

        let sys_resp = cache.get(&system_msg).unwrap();
        assert_eq!(sys_resp.text, "system response");
    }
}
