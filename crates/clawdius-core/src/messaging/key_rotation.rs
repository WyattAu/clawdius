#![deny(unsafe_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub key_hash: String,
    pub label: String,
    pub platform: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub grace_period_secs: u64,
    pub active: bool,
}

impl ApiKeyEntry {
    pub fn new(key_hash: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key_hash: key_hash.into(),
            label: label.into(),
            platform: None,
            created_at: Utc::now().timestamp(),
            expires_at: None,
            grace_period_secs: 0,
            active: true,
        }
    }

    pub fn with_platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = Some(platform.into());
        self
    }

    pub fn with_expiry(mut self, expires_at: i64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn with_grace_period(mut self, grace_period_secs: u64) -> Self {
        self.grace_period_secs = grace_period_secs;
        self
    }

    pub fn is_valid_at(&self, now: i64) -> bool {
        if !self.active {
            return false;
        }
        if let Some(expires) = self.expires_at {
            let effective_expiry = expires + self.grace_period_secs as i64;
            if now > effective_expiry {
                return false;
            }
        }
        true
    }

    pub fn is_expired(&self, now: i64) -> bool {
        if let Some(expires) = self.expires_at {
            now > expires + self.grace_period_secs as i64
        } else {
            false
        }
    }

    pub fn time_until_expiry(&self, now: i64) -> Option<i64> {
        self.expires_at.map(|e| {
            let effective = e + self.grace_period_secs as i64;
            (effective - now).max(0)
        })
    }
}

pub struct ApiKeyStore {
    keys: RwLock<Vec<ApiKeyEntry>>,
    index: RwLock<HashMap<String, usize>>,
}

impl std::fmt::Debug for ApiKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyStore").finish_non_exhaustive()
    }
}

impl ApiKeyStore {
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(Vec::new()),
            index: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_key(&self, entry: ApiKeyEntry) {
        let hash = entry.key_hash.clone();
        let mut keys = self.keys.write().await;
        let idx = keys.len();
        keys.push(entry);
        drop(keys);
        self.index.write().await.insert(hash, idx);
        info!("API key added");
    }

    pub async fn validate_key(&self, key_hash: &str) -> Option<ApiKeyEntry> {
        let now = Utc::now().timestamp();
        let keys = self.keys.read().await;
        let idx = *self.index.read().await.get(key_hash)?;
        keys.get(idx).filter(|e| e.is_valid_at(now)).cloned()
    }

    pub async fn deactivate_key(&self, key_hash: &str) -> bool {
        let mut keys = self.keys.write().await;
        let idx = match self.index.read().await.get(key_hash) {
            Some(&idx) => idx,
            None => return false,
        };
        if let Some(entry) = keys.get_mut(idx) {
            entry.active = false;
            info!("API key deactivated");
            true
        } else {
            false
        }
    }

    pub async fn list_keys(&self) -> Vec<ApiKeyEntry> {
        self.keys.read().await.clone()
    }

    pub async fn list_expired_keys(&self) -> Vec<ApiKeyEntry> {
        let now = Utc::now().timestamp();
        self.keys
            .read()
            .await
            .iter()
            .filter(|e| e.is_expired(now))
            .cloned()
            .collect()
    }

    pub async fn active_key_count(&self) -> usize {
        let now = Utc::now().timestamp();
        self.keys
            .read()
            .await
            .iter()
            .filter(|e| e.is_valid_at(now))
            .count()
    }

    pub async fn purge_expired(&self) -> usize {
        let expired_pairs: Vec<(String, usize)> = {
            let keys = self.keys.read().await;
            let now = Utc::now().timestamp();
            keys.iter()
                .enumerate()
                .filter(|(_, e)| e.is_expired(now))
                .map(|(i, e)| (e.key_hash.clone(), i))
                .collect()
        };

        if expired_pairs.is_empty() {
            return 0;
        }

        let mut keys = self.keys.write().await;
        let mut index = self.index.write().await;
        let mut indices: Vec<usize> = Vec::with_capacity(expired_pairs.len());
        for (hash, idx) in &expired_pairs {
            if index.remove(hash).is_some() {
                indices.push(*idx);
            }
        }
        indices.sort();
        indices.reverse();

        let removed = indices.len();
        for idx in indices {
            keys.remove(idx);
        }
        info!(removed, "Expired API keys purged");
        removed
    }
}

/// Non-cryptographic hash of an API key for demo/testing only.
/// For production, use SHA-256 with a random salt.
pub fn hash_api_key(key: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_ts() -> i64 {
        Utc::now().timestamp()
    }

    #[tokio::test]
    async fn test_add_and_validate_key() {
        let store = ApiKeyStore::new();
        let entry = ApiKeyEntry::new("test-key-hash", "test-key");
        store.add_key(entry).await;

        assert!(store.validate_key("test-key-hash").await.is_some());
        assert!(store.validate_key("unknown").await.is_none());
    }

    #[tokio::test]
    async fn test_deactivated_key_invalid() {
        let store = ApiKeyStore::new();
        let entry = ApiKeyEntry::new("key-hash", "my-key");
        store.add_key(entry).await;
        store.deactivate_key("key-hash").await;

        assert!(store.validate_key("key-hash").await.is_none());
    }

    #[tokio::test]
    async fn test_expired_key_invalid() {
        let store = ApiKeyStore::new();
        let past = now_ts() - 100;
        let entry = ApiKeyEntry::new("expired-hash", "expired-key").with_expiry(past);
        store.add_key(entry).await;

        assert!(store.validate_key("expired-hash").await.is_none());
    }

    #[tokio::test]
    async fn test_grace_period() {
        let store = ApiKeyStore::new();
        let past = now_ts() - 100;
        let entry = ApiKeyEntry::new("grace-hash", "grace-key")
            .with_expiry(past)
            .with_grace_period(200);
        store.add_key(entry).await;

        assert!(store.validate_key("grace-hash").await.is_some());
    }

    #[tokio::test]
    async fn test_platform_scoped_key() {
        let store = ApiKeyStore::new();
        let entry = ApiKeyEntry::new("tg-key", "telegram-key").with_platform("telegram");
        store.add_key(entry).await;

        assert!(store.validate_key("tg-key").await.is_some());
    }

    #[tokio::test]
    async fn test_purge_expired() {
        let store = ApiKeyStore::new();
        let past = now_ts() - 100;

        store.add_key(ApiKeyEntry::new("valid", "valid-key")).await;
        store
            .add_key(ApiKeyEntry::new("exp1", "expired-1").with_expiry(past))
            .await;
        store
            .add_key(ApiKeyEntry::new("exp2", "expired-2").with_expiry(past))
            .await;

        let purged = store.purge_expired().await;
        assert_eq!(purged, 2);
        assert_eq!(store.active_key_count().await, 1);
    }

    #[tokio::test]
    async fn test_list_keys() {
        let store = ApiKeyStore::new();
        store.add_key(ApiKeyEntry::new("a", "key-a")).await;
        store.add_key(ApiKeyEntry::new("b", "key-b")).await;

        assert_eq!(store.list_keys().await.len(), 2);
    }

    #[tokio::test]
    async fn test_no_grace_period() {
        let store = ApiKeyStore::new();
        let past = now_ts() - 100;
        let entry = ApiKeyEntry::new("no-grace", "no-grace-key").with_expiry(past);
        store.add_key(entry).await;

        assert!(store.validate_key("no-grace").await.is_none());
    }
}
