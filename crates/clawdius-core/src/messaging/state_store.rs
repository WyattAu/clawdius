//! Abstract State Store for Horizontal Scaling
//!
//! Provides a trait-based abstraction over state storage backends, enabling
//! the messaging system to swap between in-memory, SQLite, and future backends
//! (Redis, PostgreSQL, etc.) for horizontal scaling support.

#![deny(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use parking_lot::RwLock;
use rusqlite::params;
use tracing::{debug, warn};

use super::types::MessagingError;
use super::types::Result;

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, table: &str, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()>;
    async fn delete(&self, table: &str, key: &str) -> Result<bool>;
    async fn exists(&self, table: &str, key: &str) -> Result<bool>;

    async fn get_multi(&self, table: &str, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>>;
    async fn set_multi(
        &self,
        table: &str,
        entries: &[(String, Vec<u8>)],
        ttl: Option<u64>,
    ) -> Result<()>;

    async fn keys(&self, table: &str, pattern: &str) -> Result<Vec<String>>;
    async fn count(&self, table: &str) -> Result<usize>;

    async fn create_table(&self, table: &str) -> Result<()>;
    async fn drop_table(&self, table: &str) -> Result<()>;
    async fn table_exists(&self, table: &str) -> Result<bool>;

    async fn health_check(&self) -> Result<bool>;

    fn store_type(&self) -> &'static str;
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_expired(expires_at: Option<u64>) -> bool {
    expires_at.is_some_and(|t| t <= now_unix())
}

fn validate_table_name(table: &str) -> Result<()> {
    if table.is_empty() {
        return Err(MessagingError::InvalidConfig(
            "table name must not be empty".into(),
        ));
    }
    if table.len() > 64 {
        return Err(MessagingError::InvalidConfig(
            "table name too long (max 64 characters)".into(),
        ));
    }
    if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(MessagingError::InvalidConfig(format!(
            "invalid table name: {table}"
        )));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(MessagingError::InvalidConfig(
            "key must not be empty".into(),
        ));
    }
    Ok(())
}

fn db_err(msg: impl Into<String>) -> MessagingError {
    MessagingError::ParseError(msg.into())
}

fn internal_err(msg: impl Into<String>) -> MessagingError {
    MessagingError::ParseError(msg.into())
}

pub struct InMemoryStateStore {
    data: Arc<RwLock<HashMap<String, HashMap<String, (Vec<u8>, Option<u64>)>>>>,
}

impl InMemoryStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>> {
        validate_table_name(table)?;
        validate_key(key)?;

        let tables = self.data.read();
        let entry = tables
            .get(table)
            .and_then(|t| t.get(key))
            .map(|(data, expires_at)| (data.clone(), *expires_at));

        match entry {
            None => Ok(None),
            Some((data, expires_at)) => {
                if is_expired(expires_at) {
                    drop(tables);
                    let mut tables = self.data.write();
                    if let Some(tbl) = tables.get_mut(table) {
                        tbl.remove(key);
                    }
                    Ok(None)
                } else {
                    Ok(Some(data))
                }
            },
        }
    }

    async fn set(&self, table: &str, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        validate_table_name(table)?;
        validate_key(key)?;

        let expires_at = ttl.map(|d| now_unix() + d);
        let mut tables = self.data.write();
        let tbl = tables.entry(table.to_string()).or_default();
        tbl.insert(key.to_string(), (value.to_vec(), expires_at));
        debug!(table, key, "in-memory set");
        Ok(())
    }

    async fn delete(&self, table: &str, key: &str) -> Result<bool> {
        validate_table_name(table)?;
        validate_key(key)?;

        let mut tables = self.data.write();
        let removed = tables
            .get_mut(table)
            .and_then(|tbl| tbl.remove(key))
            .is_some();
        debug!(table, key, removed, "in-memory delete");
        Ok(removed)
    }

    async fn exists(&self, table: &str, key: &str) -> Result<bool> {
        validate_table_name(table)?;
        validate_key(key)?;

        let tables = self.data.read();
        let entry = tables
            .get(table)
            .and_then(|t| t.get(key))
            .map(|(_, expires_at)| *expires_at);

        match entry {
            None => Ok(false),
            Some(expires_at) => {
                if is_expired(expires_at) {
                    Ok(false)
                } else {
                    Ok(true)
                }
            },
        }
    }

    async fn get_multi(&self, table: &str, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        validate_table_name(table)?;

        let tables = self.data.read();
        let tbl = match tables.get(table) {
            Some(t) => t,
            None => return Ok(HashMap::new()),
        };

        let now = now_unix();
        let result: HashMap<String, Vec<u8>> = keys
            .iter()
            .filter_map(|key| {
                tbl.get(*key).and_then(|(data, expires_at)| {
                    if expires_at.is_some_and(|t| t <= now) {
                        None
                    } else {
                        Some(((*key).to_string(), data.clone()))
                    }
                })
            })
            .collect();

        Ok(result)
    }

    async fn set_multi(
        &self,
        table: &str,
        entries: &[(String, Vec<u8>)],
        ttl: Option<u64>,
    ) -> Result<()> {
        validate_table_name(table)?;

        if entries.is_empty() {
            return Ok(());
        }

        let expires_at = ttl.map(|d| now_unix() + d);
        let mut tables = self.data.write();
        let tbl = tables.entry(table.to_string()).or_default();

        for (key, value) in entries {
            validate_key(key)?;
            tbl.insert(key.clone(), (value.clone(), expires_at));
        }

        debug!(table, count = entries.len(), "in-memory set_multi");
        Ok(())
    }

    async fn keys(&self, table: &str, pattern: &str) -> Result<Vec<String>> {
        validate_table_name(table)?;

        let tables = self.data.read();
        let tbl = match tables.get(table) {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        let now = now_unix();
        let result: Vec<String> = tbl
            .iter()
            .filter(|(_, (_, expires_at))| expires_at.is_none_or(|t| t > now))
            .filter(|(key, _)| match_keys(key, pattern))
            .map(|(key, _)| key.clone())
            .collect();

        Ok(result)
    }

    async fn count(&self, table: &str) -> Result<usize> {
        validate_table_name(table)?;

        let tables = self.data.read();
        let now = now_unix();
        let count = tables
            .get(table)
            .map(|tbl| {
                tbl.iter()
                    .filter(|(_, (_, expires_at))| expires_at.is_none_or(|t| t > now))
                    .count()
            })
            .unwrap_or(0);

        Ok(count)
    }

    async fn create_table(&self, table: &str) -> Result<()> {
        validate_table_name(table)?;

        let mut tables = self.data.write();
        if !tables.contains_key(table) {
            tables.insert(table.to_string(), HashMap::new());
            debug!(table, "in-memory create_table");
        }
        Ok(())
    }

    async fn drop_table(&self, table: &str) -> Result<()> {
        validate_table_name(table)?;

        let mut tables = self.data.write();
        let removed = tables.remove(table).is_some();
        debug!(table, removed, "in-memory drop_table");
        Ok(())
    }

    async fn table_exists(&self, table: &str) -> Result<bool> {
        validate_table_name(table)?;

        let tables = self.data.read();
        Ok(tables.contains_key(table))
    }

    async fn health_check(&self) -> Result<bool> {
        let tables = self.data.read();
        Ok(!tables.is_empty())
    }

    fn store_type(&self) -> &'static str {
        "memory"
    }
}

fn match_keys(key: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return key == pattern;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 2 {
        let prefix = parts[0];
        let suffix = parts[1];
        return key.starts_with(prefix) && key.ends_with(suffix);
    }
    key == pattern
}

pub struct SqliteStateStore {
    path: String,
}

impl SqliteStateStore {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    fn table_sql(table: &str) -> String {
        format!(
            "CREATE TABLE IF NOT EXISTS \"{table}\" (key TEXT PRIMARY KEY, value BLOB NOT NULL, expires_at INTEGER)"
        )
    }

    pub async fn cleanup_expired(&self, table: &str) -> Result<usize> {
        validate_table_name(table)?;

        let sql =
            format!("DELETE FROM \"{table}\" WHERE expires_at IS NOT NULL AND expires_at <= ?");
        let now = now_unix() as i64;
        let path = self.path.clone();

        let deleted = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;
            let count = stmt.execute(params![now])?;
            Ok::<usize, rusqlite::Error>(count)
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("cleanup expired failed: {e}")))?;

        debug!(table, deleted, "sqlite cleanup_expired");
        Ok(deleted)
    }
}

#[async_trait]
impl StateStore for SqliteStateStore {
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>> {
        validate_table_name(table)?;
        validate_key(key)?;

        let select_sql = format!("SELECT value, expires_at FROM \"{table}\" WHERE key = ?");
        let delete_sql = format!("DELETE FROM \"{table}\" WHERE key = ?");
        let key = key.to_string();
        let path = self.path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;

            let (value, expired) = {
                let mut stmt = conn.prepare(&select_sql)?;
                let mut rows = stmt.query(params![key])?;

                let row = rows.next()?;
                let Some(row) = row else {
                    return Ok::<Option<Vec<u8>>, rusqlite::Error>(None);
                };

                let value: Vec<u8> = row.get(0)?;
                let expires_at: Option<i64> = row.get(1)?;
                let expired = expires_at.is_some_and(|t| t <= now_unix() as i64);
                (value, expired)
            };

            if expired {
                let mut del_stmt = conn.prepare(&delete_sql)?;
                del_stmt.execute(params![key])?;
                Ok(None)
            } else {
                Ok(Some(value))
            }
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("get failed: {e}")))?;

        Ok(result)
    }

    async fn set(&self, table: &str, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        validate_table_name(table)?;
        validate_key(key)?;

        let sql =
            format!("INSERT OR REPLACE INTO \"{table}\" (key, value, expires_at) VALUES (?, ?, ?)");
        let key_owned = key.to_string();
        let value = value.to_vec();
        let expires_at: Option<i64> = ttl.map(|d| (now_unix() + d) as i64);
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute(params![key_owned, value, expires_at])?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("set failed: {e}")))?;

        debug!(table, key, "sqlite set");
        Ok(())
    }

    async fn delete(&self, table: &str, key: &str) -> Result<bool> {
        validate_table_name(table)?;
        validate_key(key)?;

        let sql = format!("DELETE FROM \"{table}\" WHERE key = ?");
        let key_owned = key.to_string();
        let path = self.path.clone();

        let deleted = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;
            let count = stmt.execute(params![key_owned])?;
            Ok::<usize, rusqlite::Error>(count)
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("delete failed: {e}")))?;

        let removed = deleted > 0;
        debug!(table, key, removed, "sqlite delete");
        Ok(removed)
    }

    async fn exists(&self, table: &str, key: &str) -> Result<bool> {
        validate_table_name(table)?;
        validate_key(key)?;

        let sql = format!(
            "SELECT 1 FROM \"{table}\" WHERE key = ? AND (expires_at IS NULL OR expires_at > ?) LIMIT 1"
        );
        let key = key.to_string();
        let now = now_unix() as i64;
        let path = self.path.clone();

        let found = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(params![key, now])?;
            Ok::<bool, rusqlite::Error>(rows.next()?.is_some())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("exists failed: {e}")))?;

        Ok(found)
    }

    async fn get_multi(&self, table: &str, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        validate_table_name(table)?;

        if keys.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = keys.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT key, value, expires_at FROM \"{table}\" WHERE key IN ({})",
            placeholders.join(", ")
        );
        let keys_owned: Vec<String> = keys.iter().map(|k| (*k).to_string()).collect();
        let path = self.path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;

            let param_values: Vec<&dyn rusqlite::types::ToSql> = keys_owned
                .iter()
                .map(|k| k as &dyn rusqlite::types::ToSql)
                .collect();

            let mut rows = stmt.query(param_values.as_slice())?;
            let now = now_unix() as i64;
            let mut result = HashMap::new();

            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let value: Vec<u8> = row.get(1)?;
                let expires_at: Option<i64> = row.get(2)?;

                if expires_at.is_none_or(|t| t > now) {
                    result.insert(key, value);
                }
            }

            Ok::<HashMap<String, Vec<u8>>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("get_multi failed: {e}")))?;

        Ok(result)
    }

    async fn set_multi(
        &self,
        table: &str,
        entries: &[(String, Vec<u8>)],
        ttl: Option<u64>,
    ) -> Result<()> {
        validate_table_name(table)?;

        if entries.is_empty() {
            return Ok(());
        }

        for (key, _) in entries {
            validate_key(key)?;
        }

        let entry_count = entries.len();
        let sql =
            format!("INSERT OR REPLACE INTO \"{table}\" (key, value, expires_at) VALUES (?, ?, ?)");
        let entries = entries.to_vec();
        let expires_at: Option<i64> = ttl.map(|d| (now_unix() + d) as i64);
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let tx = conn.unchecked_transaction()?;

            for (key, value) in &entries {
                let mut stmt = tx.prepare(&sql)?;
                stmt.execute(params![key, value, expires_at])?;
            }

            tx.commit()?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("set_multi failed: {e}")))?;

        debug!(table, count = entry_count, "sqlite set_multi");
        Ok(())
    }

    async fn keys(&self, table: &str, pattern: &str) -> Result<Vec<String>> {
        validate_table_name(table)?;

        let now = now_unix() as i64;
        let sql =
            format!("SELECT key FROM \"{table}\" WHERE (expires_at IS NULL OR expires_at > ?)");
        let pattern = pattern.to_string();
        let path = self.path.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(params![now])?;

            let mut result = Vec::new();
            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                if match_keys(&key, &pattern) {
                    result.push(key);
                }
            }

            Ok::<Vec<String>, rusqlite::Error>(result)
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("keys failed: {e}")))?;

        Ok(result)
    }

    async fn count(&self, table: &str) -> Result<usize> {
        validate_table_name(table)?;

        let sql =
            format!("SELECT COUNT(*) FROM \"{table}\" WHERE expires_at IS NULL OR expires_at > ?");
        let now = now_unix() as i64;
        let path = self.path.clone();

        let count = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let count: i64 = conn.query_row(&sql, params![now], |row| row.get(0))?;
            Ok::<i64, rusqlite::Error>(count)
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("count failed: {e}")))?;

        Ok(count as usize)
    }

    async fn create_table(&self, table: &str) -> Result<()> {
        validate_table_name(table)?;

        let sql = Self::table_sql(table);
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute_batch(&sql)?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("create_table failed: {e}")))?;

        debug!(table, "sqlite create_table");
        Ok(())
    }

    async fn drop_table(&self, table: &str) -> Result<()> {
        validate_table_name(table)?;

        let sql = format!("DROP TABLE IF EXISTS \"{table}\"");
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute_batch(&sql)?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("drop_table failed: {e}")))?;

        debug!(table, "sqlite drop_table");
        Ok(())
    }

    async fn table_exists(&self, table: &str) -> Result<bool> {
        validate_table_name(table)?;

        let path = self.path.clone();
        let table = table.to_string();

        let exists = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query(params![table])?;
            Ok::<bool, rusqlite::Error>(rows.next()?.is_some())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .map_err(|e| db_err(format!("table_exists failed: {e}")))?;

        Ok(exists)
    }

    async fn health_check(&self) -> Result<bool> {
        let path = self.path.clone();

        let ok = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute_batch("SELECT 1")?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| internal_err(format!("spawn_blocking failed: {e}")))?
        .is_ok();

        if !ok {
            warn!("sqlite health_check failed");
        }
        Ok(ok)
    }

    fn store_type(&self) -> &'static str {
        "sqlite"
    }
}

pub enum StateStoreConfig {
    Memory,
    SQLite { path: String },
}

impl StateStoreConfig {
    pub fn create(&self) -> Result<Arc<dyn StateStore>> {
        match self {
            StateStoreConfig::Memory => Ok(Arc::new(InMemoryStateStore::new())),
            StateStoreConfig::SQLite { path } => Ok(Arc::new(SqliteStateStore::new(path))),
        }
    }
}

impl Default for StateStoreConfig {
    fn default() -> Self {
        Self::Memory
    }
}

const TABLE_SESSIONS: &str = "messaging_sessions";
const TABLE_RATE_LIMITS: &str = "messaging_rate_limits";
const TABLE_RETRY_QUEUE: &str = "messaging_retry_queue";
const TABLE_OAUTH_TOKENS: &str = "messaging_oauth_tokens";
const TABLE_TENANTS: &str = "messaging_tenants";

const TABLE_MARKETPLACE_PLUGINS: &str = "marketplace_plugins";

const ALL_TABLES: &[&str] = &[
    TABLE_SESSIONS,
    TABLE_RATE_LIMITS,
    TABLE_RETRY_QUEUE,
    TABLE_OAUTH_TOKENS,
    TABLE_TENANTS,
    TABLE_MARKETPLACE_PLUGINS,
];

pub struct StateStoreFactory {
    store: Arc<dyn StateStore>,
}

impl StateStoreFactory {
    pub fn new(config: StateStoreConfig) -> Result<Self> {
        let store = config.create()?;
        debug!(store_type = store.store_type(), "StateStoreFactory created");
        Ok(Self { store })
    }

    #[must_use]
    pub fn store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    #[must_use]
    pub fn sessions_store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    #[must_use]
    pub fn rate_limits_store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    #[must_use]
    pub fn retry_queue_store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    #[must_use]
    pub fn oauth_tokens_store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    #[must_use]
    pub fn tenant_store(&self) -> Arc<dyn StateStore> {
        Arc::clone(&self.store)
    }

    pub async fn init_tables(&self) -> Result<()> {
        for table in ALL_TABLES {
            self.store.create_table(table).await?;
        }
        debug!(
            store_type = self.store.store_type(),
            "initialized all messaging tables"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn temp_sqlite_path() -> String {
        let tmp = NamedTempFile::new().expect("temp file");
        tmp.path().to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn test_inmemory_get_set_delete_exists() {
        let store = InMemoryStateStore::new();

        assert!(store
            .get("test_table", "key1")
            .await
            .expect("get ok")
            .is_none());
        assert!(!store.exists("test_table", "key1").await.expect("exists ok"));

        store
            .set("test_table", "key1", b"value1", None)
            .await
            .expect("set ok");

        let val = store
            .get("test_table", "key1")
            .await
            .expect("get ok")
            .expect("value");
        assert_eq!(val, b"value1");
        assert!(store.exists("test_table", "key1").await.expect("exists ok"));

        let deleted = store.delete("test_table", "key1").await.expect("delete ok");
        assert!(deleted);
        assert!(!store.exists("test_table", "key1").await.expect("exists ok"));

        let deleted_again = store.delete("test_table", "key1").await.expect("delete ok");
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_inmemory_ttl_expiration() {
        let store = InMemoryStateStore::new();

        store
            .set("ttl_table", "expiring", b"data", Some(0))
            .await
            .expect("set ok");

        let val = store.get("ttl_table", "expiring").await.expect("get ok");
        assert!(val.is_none(), "TTL=0 should expire immediately on get");

        store
            .set("ttl_table", "persistent", b"data", None)
            .await
            .expect("set ok");

        let val = store.get("ttl_table", "persistent").await.expect("get ok");
        assert!(val.is_some(), "no TTL should persist");
    }

    #[tokio::test]
    async fn test_inmemory_get_multi_set_multi() {
        let store = InMemoryStateStore::new();

        let entries = vec![
            ("k1".to_string(), b"v1".to_vec()),
            ("k2".to_string(), b"v2".to_vec()),
            ("k3".to_string(), b"v3".to_vec()),
        ];

        store
            .set_multi("batch_table", &entries, None)
            .await
            .expect("set_multi ok");

        let result = store
            .get_multi("batch_table", &["k1", "k2", "k4"])
            .await
            .expect("get_multi ok");

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("k1"), Some(&b"v1".to_vec()));
        assert_eq!(result.get("k2"), Some(&b"v2".to_vec()));
        assert!(result.get("k4").is_none());
    }

    #[tokio::test]
    async fn test_inmemory_keys_and_count() {
        let store = InMemoryStateStore::new();

        store
            .set("scan_table", "user:1", b"a", None)
            .await
            .expect("set ok");
        store
            .set("scan_table", "user:2", b"b", None)
            .await
            .expect("set ok");
        store
            .set("scan_table", "admin:1", b"c", None)
            .await
            .expect("set ok");

        let count = store.count("scan_table").await.expect("count ok");
        assert_eq!(count, 3);

        let all_keys = store.keys("scan_table", "*").await.expect("keys ok");
        assert_eq!(all_keys.len(), 3);

        let user_keys = store.keys("scan_table", "user:*").await.expect("keys ok");
        assert_eq!(user_keys.len(), 2);
    }

    #[tokio::test]
    async fn test_inmemory_table_management() {
        let store = InMemoryStateStore::new();

        assert!(!store
            .table_exists("new_table")
            .await
            .expect("table_exists ok"));

        store
            .create_table("new_table")
            .await
            .expect("create_table ok");

        assert!(store
            .table_exists("new_table")
            .await
            .expect("table_exists ok"));

        store.drop_table("new_table").await.expect("drop_table ok");

        assert!(!store
            .table_exists("new_table")
            .await
            .expect("table_exists ok"));
    }

    #[tokio::test]
    async fn test_inmemory_empty_count() {
        let store = InMemoryStateStore::new();
        let count = store.count("nonexistent").await.expect("count ok");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_sqlite_get_set_delete_exists() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);

        store
            .create_table("test_tbl")
            .await
            .expect("create_table ok");

        assert!(store
            .get("test_tbl", "key1")
            .await
            .expect("get ok")
            .is_none());
        assert!(!store.exists("test_tbl", "key1").await.expect("exists ok"));

        store
            .set("test_tbl", "key1", b"value1", None)
            .await
            .expect("set ok");

        let val = store
            .get("test_tbl", "key1")
            .await
            .expect("get ok")
            .expect("value");
        assert_eq!(val, b"value1");
        assert!(store.exists("test_tbl", "key1").await.expect("exists ok"));

        let deleted = store.delete("test_tbl", "key1").await.expect("delete ok");
        assert!(deleted);
        assert!(!store.exists("test_tbl", "key1").await.expect("exists ok"));

        let deleted_again = store.delete("test_tbl", "key1").await.expect("delete ok");
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_sqlite_ttl_expiration() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);

        store
            .create_table("ttl_tbl")
            .await
            .expect("create_table ok");

        store
            .set("ttl_tbl", "expiring", b"data", Some(0))
            .await
            .expect("set ok");

        let val = store.get("ttl_tbl", "expiring").await.expect("get ok");
        assert!(val.is_none(), "TTL=0 should expire on read");

        store
            .set("ttl_tbl", "persistent", b"data", None)
            .await
            .expect("set ok");

        let val = store.get("ttl_tbl", "persistent").await.expect("get ok");
        assert!(val.is_some(), "no TTL should persist");
    }

    #[tokio::test]
    async fn test_sqlite_batch_operations() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);

        store
            .create_table("batch_tbl")
            .await
            .expect("create_table ok");

        let entries = vec![
            ("k1".to_string(), b"v1".to_vec()),
            ("k2".to_string(), b"v2".to_vec()),
            ("k3".to_string(), b"v3".to_vec()),
        ];

        store
            .set_multi("batch_tbl", &entries, None)
            .await
            .expect("set_multi ok");

        let result = store
            .get_multi("batch_tbl", &["k1", "k2", "k4"])
            .await
            .expect("get_multi ok");

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("k1"), Some(&b"v1".to_vec()));
        assert_eq!(result.get("k2"), Some(&b"v2".to_vec()));
    }

    #[tokio::test]
    async fn test_sqlite_table_management() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);

        assert!(!store
            .table_exists("my_table")
            .await
            .expect("table_exists ok"));

        store
            .create_table("my_table")
            .await
            .expect("create_table ok");

        assert!(store
            .table_exists("my_table")
            .await
            .expect("table_exists ok"));

        store.drop_table("my_table").await.expect("drop_table ok");

        assert!(!store
            .table_exists("my_table")
            .await
            .expect("table_exists ok"));
    }

    #[tokio::test]
    async fn test_sqlite_count_and_keys() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);

        store
            .create_table("count_tbl")
            .await
            .expect("create_table ok");

        store
            .set("count_tbl", "a", b"1", None)
            .await
            .expect("set ok");
        store
            .set("count_tbl", "b", b"2", None)
            .await
            .expect("set ok");

        let count = store.count("count_tbl").await.expect("count ok");
        assert_eq!(count, 2);

        let all_keys = store.keys("count_tbl", "*").await.expect("keys ok");
        assert_eq!(all_keys.len(), 2);
    }

    #[tokio::test]
    async fn test_config_create_memory() {
        let config = StateStoreConfig::Memory;
        let store = config.create().expect("create ok");
        assert_eq!(store.store_type(), "memory");
    }

    #[tokio::test]
    async fn test_config_create_sqlite() {
        let path = temp_sqlite_path();
        let config = StateStoreConfig::SQLite { path };
        let store = config.create().expect("create ok");
        assert_eq!(store.store_type(), "sqlite");
    }

    #[tokio::test]
    async fn test_factory_init_tables() {
        let path = temp_sqlite_path();
        let factory =
            StateStoreFactory::new(StateStoreConfig::SQLite { path }).expect("factory ok");

        factory.init_tables().await.expect("init_tables ok");

        assert!(factory
            .store()
            .table_exists("messaging_sessions")
            .await
            .expect("exists ok"));
        assert!(factory
            .store()
            .table_exists("messaging_rate_limits")
            .await
            .expect("exists ok"));
        assert!(factory
            .store()
            .table_exists("messaging_retry_queue")
            .await
            .expect("exists ok"));
        assert!(factory
            .store()
            .table_exists("messaging_oauth_tokens")
            .await
            .expect("exists ok"));
        assert!(factory
            .store()
            .table_exists("messaging_tenants")
            .await
            .expect("exists ok"));
    }

    #[tokio::test]
    async fn test_health_check_memory() {
        let store = InMemoryStateStore::new();
        assert!(store.health_check().await.expect("health ok"));
    }

    #[tokio::test]
    async fn test_health_check_sqlite() {
        let path = temp_sqlite_path();
        let store = SqliteStateStore::new(&path);
        assert!(store.health_check().await.expect("health ok"));
    }

    #[tokio::test]
    async fn test_store_type_identifiers() {
        let mem = InMemoryStateStore::new();
        assert_eq!(mem.store_type(), "memory");

        let path = temp_sqlite_path();
        let sql = SqliteStateStore::new(&path);
        assert_eq!(sql.store_type(), "sqlite");
    }

    #[tokio::test]
    async fn test_invalid_table_name() {
        let store = InMemoryStateStore::new();
        let result = store.get("bad table!", "key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_key_rejected() {
        let store = InMemoryStateStore::new();
        let result = store.set("valid_table", "", b"val", None).await;
        assert!(result.is_err());
    }
}
