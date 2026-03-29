#![deny(unsafe_code)]

//! Multi-Tenant Isolation for Messaging Gateway
//!
//! Bridges the team/role/permission system to messaging resources, isolating
//! sessions, rate limits, and channels per tenant (where each tenant maps to
//! a team).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::messaging::types::{CommandCategory, PermissionSet, Platform, RateLimitConfig};

// ---------------------------------------------------------------------------
// TenantId
// ---------------------------------------------------------------------------

/// Unique identifier for a messaging tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

impl TenantId {
    /// Create a [`TenantId`] from a UUID string.
    #[must_use]
    pub fn new(uuid: impl fmt::Display) -> Self {
        Self(uuid.to_string())
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for TenantId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for TenantId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ---------------------------------------------------------------------------
// TenantConfig
// ---------------------------------------------------------------------------

/// Per-tenant messaging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub tenant_id: TenantId,
    pub enabled: bool,
    pub allowed_platforms: Vec<Platform>,
    pub rate_limit_overrides: HashMap<Platform, RateLimitConfig>,
    pub max_sessions_per_user: u32,
    pub command_whitelist: Option<Vec<CommandCategory>>,
    pub default_permissions: PermissionSet,
    pub llm_model_override: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl TenantConfig {
    /// Create a new config with sensible defaults.
    #[must_use]
    pub fn new(tenant_id: TenantId) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            tenant_id,
            enabled: true,
            allowed_platforms: Vec::new(),
            rate_limit_overrides: HashMap::new(),
            max_sessions_per_user: 100,
            command_whitelist: None,
            default_permissions: PermissionSet::new(),
            llm_model_override: None,
            created_at: now,
            updated_at: now,
        }
    }
}

// ---------------------------------------------------------------------------
// TenantManager
// ---------------------------------------------------------------------------

/// Manages tenant lifecycle with optional SQLite persistence.
pub struct TenantManager {
    store: TenantStore,
}

enum TenantStore {
    Memory(RwLock<HashMap<String, TenantConfig>>),
    Sqlite {
        conn: Mutex<Connection>,
        cache: RwLock<HashMap<String, TenantConfig>>,
    },
}

impl TenantManager {
    /// Create an in-memory tenant manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: TenantStore::Memory(RwLock::new(HashMap::new())),
        }
    }

    /// Create a SQLite-backed tenant manager.
    ///
    /// The database file (and parent directories) are created automatically.
    pub fn with_persistence(db_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(TENANTS_DDL)?;
        conn.execute_batch(API_KEYS_DDL)?;
        Ok(Self {
            store: TenantStore::Sqlite {
                conn: Mutex::new(conn),
                cache: RwLock::new(HashMap::new()),
            },
        })
    }

    /// Register a tenant.
    pub fn create_tenant(
        &self,
        tenant_id: TenantId,
        config: TenantConfig,
    ) -> anyhow::Result<TenantConfig> {
        let key = tenant_id.0.clone();
        match &self.store {
            TenantStore::Memory(map) => {
                let mut map = map
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                if map.contains_key(&key) {
                    anyhow::bail!("tenant already exists: {key}");
                }
                map.insert(key.clone(), config.clone());
                debug!(tenant_id = %key, "tenant created (memory)");
                Ok(config)
            }
            TenantStore::Sqlite { conn, cache } => {
                let conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                conn.execute(
                    "INSERT INTO tenants (tenant_id, enabled, allowed_platforms, \
                     rate_limit_overrides, max_sessions_per_user, command_whitelist, \
                     default_permissions, llm_model_override, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        config.tenant_id.0,
                        config.enabled,
                        serde_json::to_string(&config.allowed_platforms)?,
                        serde_json::to_string(&config.rate_limit_overrides)?,
                        config.max_sessions_per_user,
                        config
                            .command_whitelist
                            .as_ref()
                            .map(|w| serde_json::to_string(w).ok()),
                        serde_json::to_string(&config.default_permissions)?,
                        config.llm_model_override,
                        config.created_at as i64,
                        config.updated_at as i64,
                    ],
                )?;
                let mut cache = cache
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                cache.insert(key.clone(), config.clone());
                debug!(tenant_id = %key, "tenant created (sqlite)");
                Ok(config)
            }
        }
    }

    /// Retrieve a tenant configuration.
    pub fn get_tenant(&self, tenant_id: &TenantId) -> anyhow::Result<Option<TenantConfig>> {
        let key = &tenant_id.0;
        match &self.store {
            TenantStore::Memory(map) => {
                let map = map
                    .read()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                Ok(map.get(key).cloned())
            }
            TenantStore::Sqlite { conn, cache } => {
                let cache = cache
                    .read()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                if let Some(cfg) = cache.get(key) {
                    return Ok(Some(cfg.clone()));
                }
                drop(cache);
                let conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                let mut stmt = conn.prepare(
                    "SELECT enabled, allowed_platforms, rate_limit_overrides, \
                     max_sessions_per_user, command_whitelist, default_permissions, \
                     llm_model_override, created_at, updated_at \
                     FROM tenants WHERE tenant_id = ?1",
                )?;
                let result = stmt.query_row(rusqlite::params![key], |row| {
                    let allowed: String = row.get(1)?;
                    let overrides: String = row.get(2)?;
                    let whitelist_str: Option<String> = row.get(4)?;
                    let perms_str: String = row.get(5)?;
                    Ok(TenantConfig {
                        tenant_id: tenant_id.clone(),
                        enabled: row.get(0)?,
                        allowed_platforms: serde_json::from_str(&allowed).unwrap_or_default(),
                        rate_limit_overrides: serde_json::from_str(&overrides).unwrap_or_default(),
                        max_sessions_per_user: row.get(3)?,
                        command_whitelist: whitelist_str
                            .and_then(|s| serde_json::from_str(&s).ok()),
                        default_permissions: serde_json::from_str(&perms_str).unwrap_or_default(),
                        llm_model_override: row.get(6)?,
                        created_at: row.get::<_, i64>(7)? as u64,
                        updated_at: row.get::<_, i64>(8)? as u64,
                    })
                });
                match result {
                    Ok(cfg) => {
                        let mut cache = cache_cache(&self.store)?;
                        cache.insert(key.clone(), cfg.clone());
                        Ok(Some(cfg))
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("db error: {e}")),
                }
            }
        }
    }

    /// Update an existing tenant's configuration.
    pub fn update_tenant(
        &self,
        tenant_id: &TenantId,
        mut config: TenantConfig,
    ) -> anyhow::Result<TenantConfig> {
        let key = &tenant_id.0;
        config.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match &self.store {
            TenantStore::Memory(map) => {
                let mut map = map
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                if !map.contains_key(key) {
                    anyhow::bail!("tenant not found: {key}");
                }
                map.insert(key.clone(), config.clone());
                debug!(tenant_id = %key, "tenant updated (memory)");
                Ok(config)
            }
            TenantStore::Sqlite { conn, cache } => {
                let conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                let rows = conn.execute(
                    "UPDATE tenants SET enabled=?1, allowed_platforms=?2, \
                     rate_limit_overrides=?3, max_sessions_per_user=?4, \
                     command_whitelist=?5, default_permissions=?6, \
                     llm_model_override=?7, updated_at=?8 \
                     WHERE tenant_id=?9",
                    rusqlite::params![
                        config.enabled,
                        serde_json::to_string(&config.allowed_platforms)?,
                        serde_json::to_string(&config.rate_limit_overrides)?,
                        config.max_sessions_per_user,
                        config
                            .command_whitelist
                            .as_ref()
                            .map(|w| serde_json::to_string(w).ok()),
                        serde_json::to_string(&config.default_permissions)?,
                        config.llm_model_override,
                        config.updated_at as i64,
                        key,
                    ],
                )?;
                if rows == 0 {
                    anyhow::bail!("tenant not found: {key}");
                }
                let mut cache = cache
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                cache.insert(key.clone(), config.clone());
                debug!(tenant_id = %key, "tenant updated (sqlite)");
                Ok(config)
            }
        }
    }

    /// Remove a tenant entirely.
    pub fn delete_tenant(&self, tenant_id: &TenantId) -> anyhow::Result<()> {
        let key = &tenant_id.0;
        match &self.store {
            TenantStore::Memory(map) => {
                let mut map = map
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                if map.remove(key).is_none() {
                    anyhow::bail!("tenant not found: {key}");
                }
                debug!(tenant_id = %key, "tenant deleted (memory)");
                Ok(())
            }
            TenantStore::Sqlite { conn, cache } => {
                let conn = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                let rows = conn.execute(
                    "DELETE FROM tenant_api_keys WHERE tenant_id = ?1",
                    rusqlite::params![key],
                )?;
                debug!(rows, "removed api key mappings");
                let rows = conn.execute(
                    "DELETE FROM tenants WHERE tenant_id = ?1",
                    rusqlite::params![key],
                )?;
                if rows == 0 {
                    anyhow::bail!("tenant not found: {key}");
                }
                let mut cache = cache
                    .write()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                cache.remove(key);
                debug!(tenant_id = %key, "tenant deleted (sqlite)");
                Ok(())
            }
        }
    }

    /// List all tenant configurations.
    #[must_use]
    pub fn list_tenants(&self) -> Vec<TenantConfig> {
        match &self.store {
            TenantStore::Memory(map) => match map.read() {
                Ok(m) => m.values().cloned().collect(),
                Err(_) => Vec::new(),
            },
            TenantStore::Sqlite { conn, .. } => match conn.lock() {
                Ok(conn) => {
                    let mut stmt = match conn.prepare(
                        "SELECT tenant_id, enabled, allowed_platforms, \
                             rate_limit_overrides, max_sessions_per_user, \
                             command_whitelist, default_permissions, \
                             llm_model_override, created_at, updated_at \
                             FROM tenants",
                    ) {
                        Ok(s) => s,
                        Err(_) => return Vec::new(),
                    };
                    let rows = match stmt.query_map([], |row| {
                        let id: String = row.get(0)?;
                        let allowed: String = row.get(2)?;
                        let overrides: String = row.get(3)?;
                        let whitelist_str: Option<String> = row.get(5)?;
                        let perms_str: String = row.get(6)?;
                        Ok(TenantConfig {
                            tenant_id: TenantId(id),
                            enabled: row.get(1)?,
                            allowed_platforms: serde_json::from_str(&allowed).unwrap_or_default(),
                            rate_limit_overrides: serde_json::from_str(&overrides)
                                .unwrap_or_default(),
                            max_sessions_per_user: row.get(4)?,
                            command_whitelist: whitelist_str
                                .and_then(|s| serde_json::from_str(&s).ok()),
                            default_permissions: serde_json::from_str(&perms_str)
                                .unwrap_or_default(),
                            llm_model_override: row.get(7)?,
                            created_at: row.get::<_, i64>(8)? as u64,
                            updated_at: row.get::<_, i64>(9)? as u64,
                        })
                    }) {
                        Ok(r) => r,
                        Err(_) => return Vec::new(),
                    };
                    rows.filter_map(|r| r.ok()).collect()
                }
                Err(_) => Vec::new(),
            },
        }
    }

    /// Check whether a tenant is enabled.
    #[must_use]
    pub fn is_tenant_enabled(&self, tenant_id: &TenantId) -> bool {
        self.get_tenant(tenant_id)
            .ok()
            .flatten()
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Return allowed platforms for a tenant. Empty vec means all platforms.
    #[must_use]
    pub fn get_allowed_platforms(&self, tenant_id: &TenantId) -> Vec<Platform> {
        match self.get_tenant(tenant_id) {
            Ok(Some(cfg)) if cfg.allowed_platforms.is_empty() => Platform::all().to_vec(),
            Ok(Some(cfg)) => cfg.allowed_platforms,
            _ => Vec::new(),
        }
    }

    /// Get the effective rate limit for a platform within a tenant.
    ///
    /// Returns the tenant override if set, otherwise `None` (caller should
    /// fall back to the global default).
    #[must_use]
    pub fn get_rate_limit(
        &self,
        tenant_id: &TenantId,
        platform: Platform,
    ) -> Option<RateLimitConfig> {
        self.get_tenant(tenant_id)
            .ok()
            .flatten()
            .and_then(|cfg| cfg.rate_limit_overrides.get(&platform).cloned())
    }

    /// Number of registered tenants.
    #[must_use]
    pub fn tenant_count(&self) -> usize {
        match &self.store {
            TenantStore::Memory(map) => map.read().map(|m| m.len()).unwrap_or(0),
            TenantStore::Sqlite { cache, .. } => cache.read().map(|c| c.len()).unwrap_or(0),
        }
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: acquire a write-lock on the cache inside the Sqlite variant.
fn cache_cache(
    store: &TenantStore,
) -> anyhow::Result<std::sync::RwLockWriteGuard<'_, HashMap<String, TenantConfig>>> {
    match store {
        TenantStore::Sqlite { cache, .. } => cache
            .write()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}")),
        _ => anyhow::bail!("not a sqlite store"),
    }
}

impl Platform {
    /// Return every known platform variant.
    #[must_use]
    fn all() -> [Self; 8] {
        [
            Self::Telegram,
            Self::Discord,
            Self::Matrix,
            Self::Signal,
            Self::RocketChat,
            Self::WhatsApp,
            Self::Slack,
            Self::Webhook,
        ]
    }
}

const TENANTS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS tenants (
    tenant_id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    allowed_platforms TEXT,
    rate_limit_overrides TEXT,
    max_sessions_per_user INTEGER NOT NULL DEFAULT 100,
    command_whitelist TEXT,
    default_permissions TEXT,
    llm_model_override TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);";

const API_KEYS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS tenant_api_keys (
    api_key TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id)
);";

// ---------------------------------------------------------------------------
// TenantContext
// ---------------------------------------------------------------------------

/// Per-request tenant context providing isolation checks.
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub config: TenantConfig,
}

impl TenantContext {
    /// Load a [`TenantContext`] from the manager.
    pub fn new(tenant_id: TenantId, manager: &TenantManager) -> anyhow::Result<Self> {
        let config = manager
            .get_tenant(&tenant_id)?
            .ok_or_else(|| anyhow::anyhow!("tenant not found: {tenant_id}"))?;
        Ok(Self { tenant_id, config })
    }

    /// Whether the given platform is allowed for this tenant.
    #[must_use]
    pub fn is_platform_allowed(&self, platform: Platform) -> bool {
        if self.config.allowed_platforms.is_empty() {
            return true;
        }
        self.config.allowed_platforms.contains(&platform)
    }

    /// Whether a command category is permitted.
    ///
    /// When `command_whitelist` is `None` all categories are allowed.
    #[must_use]
    pub fn is_command_allowed(&self, category: CommandCategory) -> bool {
        match &self.config.command_whitelist {
            Some(list) => list.contains(&category),
            None => true,
        }
    }

    /// Borrow the default permission set for this tenant.
    #[must_use]
    pub fn get_permissions(&self) -> &PermissionSet {
        &self.config.default_permissions
    }

    /// Effective rate limit for a platform (override first, then global default).
    #[must_use]
    pub fn get_rate_limit(&self, platform: Platform) -> Option<RateLimitConfig> {
        self.config.rate_limit_overrides.get(&platform).cloned()
    }
}

// ---------------------------------------------------------------------------
// TenantResolver
// ---------------------------------------------------------------------------

/// Resolves incoming requests to their owning tenant via API keys or
/// platform-user mappings.
pub struct TenantResolver {
    manager: Arc<TenantManager>,
    api_keys: RwLock<HashMap<String, String>>,
}

impl TenantResolver {
    /// Create a new resolver backed by the given tenant manager.
    ///
    /// If the manager is SQLite-backed, existing API key mappings are loaded
    /// from the database automatically.
    #[must_use]
    pub fn new(manager: Arc<TenantManager>) -> Self {
        let api_keys = RwLock::new(HashMap::new());
        let resolver = Self { manager, api_keys };

        if let TenantStore::Sqlite { conn, .. } = &resolver.manager.store {
            if let Ok(conn) = conn.lock() {
                if let Ok(mut stmt) = conn.prepare("SELECT api_key, tenant_id FROM tenant_api_keys")
                {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    }) {
                        if let Ok(mut keys) = resolver.api_keys.write() {
                            for row in rows.flatten() {
                                keys.insert(row.0, row.1);
                            }
                        }
                    }
                }
            }
        }

        resolver
    }

    /// Resolve an API key to its tenant.
    pub fn resolve_by_api_key(&self, api_key: &str) -> anyhow::Result<Option<TenantId>> {
        let keys = self
            .api_keys
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        match keys.get(api_key) {
            Some(tid) => {
                let enabled = self.manager.is_tenant_enabled(&TenantId(tid.clone()));
                if enabled {
                    Ok(Some(TenantId(tid.clone())))
                } else {
                    warn!(api_key = api_key, tenant_id = %tid, "resolved tenant is disabled");
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Resolve a platform + user_id pair to a tenant.
    pub fn resolve_by_platform_user(
        &self,
        _platform: Platform,
        _user_id: &str,
    ) -> anyhow::Result<Option<TenantId>> {
        let _composite = format!("{}:{}", _platform.as_str(), _user_id);
        let keys = self
            .api_keys
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        if let Some(tid) = keys.get(&_composite) {
            return Ok(Some(TenantId(tid.clone())));
        }
        Ok(None)
    }

    /// Associate an API key with a tenant.
    pub fn register_api_key(&self, tenant_id: &TenantId, api_key: &str) -> anyhow::Result<()> {
        let exists = self.manager.get_tenant(tenant_id)?.is_some();
        if !exists {
            anyhow::bail!("tenant not found: {tenant_id}");
        }
        let mut keys = self
            .api_keys
            .write()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        keys.insert(api_key.to_string(), tenant_id.0.clone());

        if let TenantStore::Sqlite { conn, .. } = &self.manager.store {
            let conn = conn
                .lock()
                .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO tenant_api_keys (api_key, tenant_id, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![api_key, tenant_id.0, now as i64],
            ) {
                warn!("failed to persist api key mapping: {e}");
            }
        }
        debug!(api_key, tenant_id = %tenant_id.0, "api key registered");
        Ok(())
    }

    /// Remove an API key mapping.
    pub fn unregister_api_key(&self, api_key: &str) -> anyhow::Result<()> {
        let mut keys = self
            .api_keys
            .write()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        if keys.remove(api_key).is_none() {
            anyhow::bail!("api key not found: {api_key}");
        }

        if let TenantStore::Sqlite { conn, .. } = &self.manager.store {
            let conn = conn
                .lock()
                .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
            if let Err(e) = conn.execute(
                "DELETE FROM tenant_api_keys WHERE api_key = ?1",
                rusqlite::params![api_key],
            ) {
                warn!("failed to remove persisted api key: {e}");
            }
        }
        debug!(api_key, "api key unregistered");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> TenantManager {
        TenantManager::new()
    }

    fn make_tenant_id(s: &str) -> TenantId {
        TenantId(s.to_string())
    }

    fn make_default_config(id: TenantId) -> TenantConfig {
        TenantConfig::new(id)
    }

    // -- CRUD ----------------------------------------------------------------

    #[test]
    fn test_create_and_get_tenant() {
        let mgr = make_manager();
        let id = make_tenant_id("t1");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let fetched = mgr.get_tenant(&id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().tenant_id.0, "t1");
    }

    #[test]
    fn test_create_duplicate_tenant_fails() {
        let mgr = make_manager();
        let id = make_tenant_id("dup");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg.clone()).unwrap();
        let result = mgr.create_tenant(id, cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_nonexistent_tenant() {
        let mgr = make_manager();
        let result = mgr.get_tenant(&make_tenant_id("nope")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_tenant() {
        let mgr = make_manager();
        let id = make_tenant_id("upd");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let mut updated = mgr.get_tenant(&id).unwrap().unwrap();
        updated.enabled = false;
        updated.max_sessions_per_user = 5;
        let result = mgr.update_tenant(&id, updated.clone()).unwrap();
        assert!(!result.enabled);
        assert_eq!(result.max_sessions_per_user, 5);
        let fetched = mgr.get_tenant(&id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_update_nonexistent_tenant_fails() {
        let mgr = make_manager();
        let id = make_tenant_id("ghost");
        let cfg = make_default_config(id.clone());
        let result = mgr.update_tenant(&id, cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_tenant() {
        let mgr = make_manager();
        let id = make_tenant_id("del");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        mgr.delete_tenant(&id).unwrap();
        assert!(mgr.get_tenant(&id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_tenant_fails() {
        let mgr = make_manager();
        let result = mgr.delete_tenant(&make_tenant_id("ghost"));
        assert!(result.is_err());
    }

    // -- List & enabled -------------------------------------------------------

    #[test]
    fn test_list_tenants() {
        let mgr = make_manager();
        for i in 0..3 {
            let id = make_tenant_id(&format!("list-{i}"));
            mgr.create_tenant(
                id,
                make_default_config(make_tenant_id(&format!("list-{i}"))),
            )
            .unwrap();
        }
        assert_eq!(mgr.list_tenants().len(), 3);
    }

    #[test]
    fn test_is_tenant_enabled() {
        let mgr = make_manager();
        let id = make_tenant_id("en");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        assert!(mgr.is_tenant_enabled(&id));
        let mut updated = mgr.get_tenant(&id).unwrap().unwrap();
        updated.enabled = false;
        mgr.update_tenant(&id, updated).unwrap();
        assert!(!mgr.is_tenant_enabled(&id));
    }

    #[test]
    fn test_tenant_count() {
        let mgr = make_manager();
        assert_eq!(mgr.tenant_count(), 0);
        let id = make_tenant_id("cnt");
        mgr.create_tenant(id, make_default_config(make_tenant_id("cnt")))
            .unwrap();
        assert_eq!(mgr.tenant_count(), 1);
    }

    // -- Platform filtering ---------------------------------------------------

    #[test]
    fn test_allowed_platforms_empty_means_all() {
        let mgr = make_manager();
        let id = make_tenant_id("plat-all");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let platforms = mgr.get_allowed_platforms(&id);
        assert_eq!(platforms.len(), 8);
    }

    #[test]
    fn test_allowed_platforms_filtered() {
        let mgr = make_manager();
        let id = make_tenant_id("plat-filt");
        let mut cfg = make_default_config(id.clone());
        cfg.allowed_platforms = vec![Platform::Telegram, Platform::Discord];
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let platforms = mgr.get_allowed_platforms(&id);
        assert_eq!(platforms.len(), 2);
    }

    // -- Rate-limit overrides -------------------------------------------------

    #[test]
    fn test_rate_limit_override() {
        let mgr = make_manager();
        let id = make_tenant_id("rl");
        let mut cfg = make_default_config(id.clone());
        let override_rl = RateLimitConfig {
            requests_per_minute: 5,
            burst_capacity: 2,
            tokens_per_refill: 1,
            refill_interval_ms: 1000,
        };
        cfg.rate_limit_overrides
            .insert(Platform::Telegram, override_rl.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let got = mgr.get_rate_limit(&id, Platform::Telegram).unwrap();
        assert_eq!(got.requests_per_minute, 5);
        let missing = mgr.get_rate_limit(&id, Platform::Discord);
        assert!(missing.is_none());
    }

    // -- TenantContext --------------------------------------------------------

    #[test]
    fn test_tenant_context_platform_filtering() {
        let mgr = make_manager();
        let id = make_tenant_id("ctx-plat");
        let mut cfg = make_default_config(id.clone());
        cfg.allowed_platforms = vec![Platform::Slack];
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let ctx = TenantContext::new(id, &mgr).unwrap();
        assert!(ctx.is_platform_allowed(Platform::Slack));
        assert!(!ctx.is_platform_allowed(Platform::Telegram));
    }

    #[test]
    fn test_tenant_context_command_whitelist() {
        let mgr = make_manager();
        let id = make_tenant_id("ctx-cmd");
        let mut cfg = make_default_config(id.clone());
        cfg.command_whitelist = Some(vec![CommandCategory::Status, CommandCategory::Help]);
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let ctx = TenantContext::new(id, &mgr).unwrap();
        assert!(ctx.is_command_allowed(CommandCategory::Status));
        assert!(!ctx.is_command_allowed(CommandCategory::Generate));
    }

    #[test]
    fn test_tenant_context_no_whitelist_allows_all() {
        let mgr = make_manager();
        let id = make_tenant_id("ctx-nocmd");
        let cfg = make_default_config(id.clone());
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let ctx = TenantContext::new(id, &mgr).unwrap();
        assert!(ctx.is_command_allowed(CommandCategory::Admin));
    }

    // -- TenantResolver -------------------------------------------------------

    #[test]
    fn test_resolver_api_key_mapping() {
        let mgr = Arc::new(make_manager());
        let id = make_tenant_id("res");
        mgr.create_tenant(id.clone(), make_default_config(id.clone()))
            .unwrap();
        let resolver = TenantResolver::new(mgr);
        resolver.register_api_key(&id, "key-abc").unwrap();
        let resolved = resolver.resolve_by_api_key("key-abc").unwrap();
        assert_eq!(resolved.unwrap().0, "res");
    }

    #[test]
    fn test_resolver_unknown_api_key() {
        let mgr = Arc::new(make_manager());
        let resolver = TenantResolver::new(mgr);
        let resolved = resolver.resolve_by_api_key("nope").unwrap();
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolver_disabled_tenant_returns_none() {
        let mgr = Arc::new(make_manager());
        let id = make_tenant_id("dis");
        let mut cfg = make_default_config(id.clone());
        cfg.enabled = false;
        mgr.create_tenant(id.clone(), cfg).unwrap();
        let resolver = TenantResolver::new(mgr);
        resolver.register_api_key(&id, "key-dis").unwrap();
        let resolved = resolver.resolve_by_api_key("key-dis").unwrap();
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolver_unregister_api_key() {
        let mgr = Arc::new(make_manager());
        let id = make_tenant_id("unreg");
        mgr.create_tenant(id.clone(), make_default_config(id.clone()))
            .unwrap();
        let resolver = TenantResolver::new(mgr);
        resolver.register_api_key(&id, "key-unreg").unwrap();
        resolver.unregister_api_key("key-unreg").unwrap();
        let resolved = resolver.resolve_by_api_key("key-unreg").unwrap();
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolver_register_for_nonexistent_tenant_fails() {
        let mgr = Arc::new(make_manager());
        let resolver = TenantResolver::new(mgr);
        let result = resolver.register_api_key(&make_tenant_id("ghost"), "key-ghost");
        assert!(result.is_err());
    }

    // -- SQLite persistence ---------------------------------------------------

    #[test]
    fn test_sqlite_persistence_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tenants.db");
        let id = make_tenant_id("sqlite-t");
        let mut cfg = make_default_config(id.clone());
        cfg.allowed_platforms = vec![Platform::Telegram];
        cfg.max_sessions_per_user = 42;

        {
            let mgr = TenantManager::with_persistence(&db_path).unwrap();
            mgr.create_tenant(id.clone(), cfg.clone()).unwrap();
            let fetched = mgr.get_tenant(&id).unwrap().unwrap();
            assert_eq!(fetched.max_sessions_per_user, 42);
            assert_eq!(fetched.allowed_platforms.len(), 1);
        }

        let mgr2 = TenantManager::with_persistence(&db_path).unwrap();
        let fetched = mgr2.get_tenant(&id).unwrap().unwrap();
        assert_eq!(fetched.max_sessions_per_user, 42);
        assert_eq!(fetched.allowed_platforms[0], Platform::Telegram);
    }

    #[test]
    fn test_sqlite_api_key_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tenants2.db");
        let id = make_tenant_id("sqlite-key");

        let mgr = Arc::new(TenantManager::with_persistence(&db_path).unwrap());
        mgr.create_tenant(id.clone(), make_default_config(id.clone()))
            .unwrap();
        let resolver = TenantResolver::new(Arc::clone(&mgr));
        resolver.register_api_key(&id, "persisted-key").unwrap();

        drop(resolver);
        let resolver2 = TenantResolver::new(Arc::clone(&mgr));
        let resolved = resolver2.resolve_by_api_key("persisted-key").unwrap();
        assert_eq!(resolved.unwrap().0, "sqlite-key");
    }

    // -- Default config -------------------------------------------------------

    #[test]
    fn test_default_tenant_config_values() {
        let id = make_tenant_id("defaults");
        let cfg = TenantConfig::new(id);
        assert!(cfg.enabled);
        assert!(cfg.allowed_platforms.is_empty());
        assert!(cfg.rate_limit_overrides.is_empty());
        assert_eq!(cfg.max_sessions_per_user, 100);
        assert!(cfg.command_whitelist.is_none());
        assert!(!cfg.default_permissions.can_admin);
        assert!(cfg.default_permissions.can_generate);
        assert!(cfg.llm_model_override.is_none());
        assert_eq!(cfg.created_at, cfg.updated_at);
    }

    // -- TenantId conversions -------------------------------------------------

    #[test]
    fn test_tenant_id_from_str_and_string() {
        let from_str: TenantId = "abc".into();
        assert_eq!(from_str.0, "abc");
        let from_string: TenantId = String::from("def").into();
        assert_eq!(from_string.0, "def");
    }

    #[test]
    fn test_tenant_id_display() {
        let id = TenantId::new("display-test");
        assert_eq!(format!("{id}"), "display-test");
    }

    // -- Integration: multi-tenant isolation -----------------------------------

    #[tokio::test]
    async fn test_tenant_context_blocks_unauthorized_command() {
        let manager = Arc::new(make_manager());
        let tenant_id = TenantId::new("corp-a");

        let mut config = make_default_config(tenant_id.clone());
        config.command_whitelist = Some(vec![CommandCategory::Status, CommandCategory::Help]);
        manager.create_tenant(tenant_id.clone(), config).unwrap();

        let ctx = TenantContext::new(tenant_id, &manager).unwrap();

        assert!(ctx.is_command_allowed(CommandCategory::Status));
        assert!(ctx.is_command_allowed(CommandCategory::Help));

        assert!(!ctx.is_command_allowed(CommandCategory::Admin));
        assert!(!ctx.is_command_allowed(CommandCategory::Generate));
        assert!(!ctx.is_command_allowed(CommandCategory::Analyze));
        assert!(!ctx.is_command_allowed(CommandCategory::Config));
    }

    #[tokio::test]
    async fn test_tenant_resolver_isolates_api_keys() {
        let manager = Arc::new(make_manager());

        let t1 = TenantId::new("tenant-alpha");
        let t2 = TenantId::new("tenant-beta");
        manager
            .create_tenant(t1.clone(), make_default_config(t1.clone()))
            .unwrap();
        manager
            .create_tenant(t2.clone(), make_default_config(t2.clone()))
            .unwrap();

        let resolver = TenantResolver::new(manager.clone());
        resolver.register_api_key(&t1, "key-alpha-1").unwrap();
        resolver.register_api_key(&t2, "key-beta-1").unwrap();

        assert_eq!(
            resolver
                .resolve_by_api_key("key-alpha-1")
                .unwrap()
                .map(|id| id.to_string()),
            Some("tenant-alpha".to_string())
        );
        assert_eq!(
            resolver
                .resolve_by_api_key("key-beta-1")
                .unwrap()
                .map(|id| id.to_string()),
            Some("tenant-beta".to_string())
        );
        assert!(resolver.resolve_by_api_key("key-gamma").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_tenant_platform_filtering_blocks_requests() {
        let manager = Arc::new(make_manager());

        let tenant_id = TenantId::new("slack-only-tenant");
        let mut config = make_default_config(tenant_id.clone());
        config.allowed_platforms = vec![Platform::Slack];
        manager.create_tenant(tenant_id.clone(), config).unwrap();

        let ctx = TenantContext::new(tenant_id, &manager).unwrap();

        assert!(ctx.is_platform_allowed(Platform::Slack));
        assert!(!ctx.is_platform_allowed(Platform::Telegram));
        assert!(!ctx.is_platform_allowed(Platform::Discord));
        assert!(!ctx.is_platform_allowed(Platform::Matrix));
    }

    #[tokio::test]
    async fn test_disabled_tenant_blocks_access() {
        let manager = Arc::new(make_manager());

        let tenant_id = TenantId::new("disabled-tenant");
        let mut config = make_default_config(tenant_id.clone());
        config.enabled = false;
        manager.create_tenant(tenant_id.clone(), config).unwrap();

        let resolver = TenantResolver::new(manager.clone());
        resolver
            .register_api_key(&tenant_id, "disabled-key")
            .unwrap();

        let resolved = resolver.resolve_by_api_key("disabled-key").unwrap();
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn test_tenant_rate_limit_override() {
        let manager = Arc::new(make_manager());

        let tenant_id = TenantId::new("rate-limited");
        let mut config = make_default_config(tenant_id.clone());
        config.rate_limit_overrides.insert(
            Platform::Slack,
            RateLimitConfig {
                requests_per_minute: 5,
                burst_capacity: 2,
                tokens_per_refill: 1,
                refill_interval_ms: 1000,
            },
        );
        manager.create_tenant(tenant_id.clone(), config).unwrap();

        let ctx = TenantContext::new(tenant_id, &manager).unwrap();

        let rate = ctx.get_rate_limit(Platform::Slack);
        assert!(rate.is_some());
        assert_eq!(rate.unwrap().requests_per_minute, 5);

        let rate = ctx.get_rate_limit(Platform::Telegram);
        assert!(rate.is_none());
    }
}
