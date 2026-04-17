//! Tenant database isolation tiers.
//!
//! Clawdius supports three isolation tiers for multi-tenant deployments:
//!
//! - **Shared**: All tenants share one database, filtered by `tenant_id` column.
//! - **Schema**: Each tenant gets a dedicated `SQLite` database file (separate pool).
//! - **Process**: Each tenant gets a separate database file with a dedicated connection pool.
//!
//! The `TenantIsolationManager` acts as a routing layer that selects the correct
//! `PooledSessionStore` based on a tenant's isolation tier.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use super::pool::{create_pool, ConnectionPoolConfig, PooledSessionStore, SessionPool};
use crate::error::Result;

/// Isolation tier for a tenant's data.
///
/// Higher tiers provide stronger isolation at the cost of more resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum IsolationTier {
    /// All tenants share one database. Queries are filtered by `tenant_id` column.
    /// This is the most resource-efficient tier.
    #[default]
    Shared,
    /// Each tenant gets a dedicated `SQLite` database file.
    /// Provides file-level isolation while sharing a process.
    DedicatedDb,
}

impl IsolationTier {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::DedicatedDb => "dedicated_db",
        }
    }

    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "dedicated_db" | "dedicated" | "schema" | "process" => Self::DedicatedDb,
            _ => Self::Shared,
        }
    }
}


impl std::fmt::Display for IsolationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for tenant database isolation.
#[derive(Debug, Clone)]
pub struct IsolationConfig {
    /// Default isolation tier for new tenants.
    pub default_tier: IsolationTier,
    /// Base directory for per-tenant database files (used by `DedicatedDb` tier).
    pub dedicated_db_base_path: PathBuf,
    /// Connection pool config for shared database.
    pub shared_pool_config: ConnectionPoolConfig,
    /// Connection pool config for dedicated tenant databases.
    pub dedicated_pool_config: ConnectionPoolConfig,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            default_tier: IsolationTier::Shared,
            dedicated_db_base_path: PathBuf::from(".clawdius/tenants"),
            shared_pool_config: ConnectionPoolConfig::default(),
            dedicated_pool_config: ConnectionPoolConfig {
                max_size: 8,
                min_idle: 1,
                connection_timeout_secs: 5,
            },
        }
    }
}

/// Manages database isolation for multiple tenants.
///
/// Routes session operations to the correct database based on each tenant's
/// isolation tier. For `Shared` tier tenants, all data lives in one database
/// with a `tenant_id` column for filtering. For `DedicatedDb` tier tenants,
/// each gets its own `SQLite` file with a dedicated connection pool.
pub struct TenantIsolationManager {
    config: IsolationConfig,
    /// Shared database pool (used by all Shared-tier tenants).
    shared_store: Arc<PooledSessionStore>,
    /// Per-tenant dedicated stores (`DedicatedDb` tier).
    dedicated_stores: RwLock<HashMap<String, Arc<PooledSessionStore>>>,
    /// Per-tenant isolation tier overrides.
    tenant_tiers: RwLock<HashMap<String, IsolationTier>>,
}

impl TenantIsolationManager {
    /// Create a new isolation manager.
    ///
    /// Opens (or creates) the shared database at `shared_db_path`.
    pub fn new(shared_db_path: &Path, config: IsolationConfig) -> Result<Self> {
        let shared_pool = create_pool(shared_db_path, &config.shared_pool_config)?;
        let shared_store = Arc::new(PooledSessionStore::new(shared_pool));
        Ok(Self {
            config,
            shared_store,
            dedicated_stores: RwLock::new(HashMap::new()),
            tenant_tiers: RwLock::new(HashMap::new()),
        })
    }

    /// Register a tenant's isolation tier.
    ///
    /// Call this during tenant onboarding. If the tenant uses `DedicatedDb`,
    /// a new database file and connection pool will be created on first access.
    pub fn register_tenant(&self, tenant_id: &str, tier: IsolationTier) -> Result<()> {
        self.tenant_tiers
            .write()
            .insert(tenant_id.to_string(), tier);

        if tier == IsolationTier::DedicatedDb {
            self.ensure_dedicated_store(tenant_id)?;
        }

        Ok(())
    }

    /// Get the isolation tier for a tenant.
    #[must_use]
    pub fn get_tier(&self, tenant_id: &str) -> IsolationTier {
        self.tenant_tiers
            .read()
            .get(tenant_id)
            .copied()
            .unwrap_or(self.config.default_tier)
    }

    /// Set the isolation tier for an existing tenant.
    ///
    /// Changing from `Shared` to `DedicatedDb` will create a new database.
    /// Data migration from shared to dedicated is NOT automatic — the caller
    /// must handle data migration if needed.
    pub fn set_tier(&self, tenant_id: &str, tier: IsolationTier) -> Result<()> {
        self.register_tenant(tenant_id, tier)
    }

    /// Resolve the session store for a given tenant.
    pub fn resolve_store(&self, tenant_id: &str) -> Arc<PooledSessionStore> {
        match self.get_tier(tenant_id) {
            IsolationTier::Shared => Arc::clone(&self.shared_store),
            IsolationTier::DedicatedDb => {
                // Try to get existing; if not found, create on demand.
                // In production, register_tenant should be called first.
                if let Some(store) = self.dedicated_stores.read().get(tenant_id) {
                    return Arc::clone(store);
                }
                // Fallback: create on demand (best-effort)
                if let Ok(store) = self.create_dedicated_store(tenant_id) {
                    let mut stores = self.dedicated_stores.write();
                    stores
                        .entry(tenant_id.to_string())
                        .or_insert_with(|| Arc::clone(&store));
                    drop(stores);
                    if let Some(s) = self.dedicated_stores.read().get(tenant_id) {
                        return Arc::clone(s);
                    }
                }
                // Ultimate fallback to shared store
                Arc::clone(&self.shared_store)
            },
        }
    }

    /// Get a reference to the shared session store.
    #[must_use]
    pub fn shared_store(&self) -> Arc<PooledSessionStore> {
        Arc::clone(&self.shared_store)
    }

    /// List all registered tenant IDs.
    #[must_use]
    pub fn list_tenants(&self) -> Vec<String> {
        self.tenant_tiers.read().keys().cloned().collect()
    }

    /// Remove a tenant's dedicated database (`DedicatedDb` tier only).
    ///
    /// For Shared tier tenants, this is a no-op (data remains in shared DB).
    /// The database file is NOT deleted from disk — only the pool is dropped.
    pub fn remove_tenant(&self, tenant_id: &str) {
        if self.get_tier(tenant_id) == IsolationTier::DedicatedDb {
            self.dedicated_stores.write().remove(tenant_id);
        }
        self.tenant_tiers.write().remove(tenant_id);
    }

    /// Get the database path for a dedicated tenant.
    #[must_use]
    pub fn dedicated_db_path(&self, tenant_id: &str) -> PathBuf {
        self.config
            .dedicated_db_base_path
            .join(format!("{tenant_id}.db"))
    }

    // --- Internal helpers ---

    fn ensure_dedicated_store(&self, tenant_id: &str) -> Result<()> {
        let stores = self.dedicated_stores.read();
        if stores.contains_key(tenant_id) {
            return Ok(());
        }
        drop(stores);
        self.create_dedicated_store(tenant_id)?;
        Ok(())
    }

    fn create_dedicated_store(&self, tenant_id: &str) -> Result<Arc<PooledSessionStore>> {
        let db_path = self.dedicated_db_path(tenant_id);
        let pool = create_pool(&db_path, &self.config.dedicated_pool_config)?;
        Ok(Arc::new(PooledSessionStore::new(pool)))
    }
}

// --- Tenant-aware query helpers ---
//
// These are extension methods that add tenant_id filtering to session queries
// when operating in Shared isolation mode.

/// Add `tenant_id` column to shared database if it doesn't exist.
///
/// This is a lightweight migration that should be called once during startup
/// when using Shared isolation tier.
pub fn ensure_tenant_columns(pool: &SessionPool) -> Result<()> {
    let conn = pool
        .get()
        .map_err(|e| crate::Error::Session(e.to_string()))?;

    // Add tenant_id column to sessions table (if missing)
    let has_tenant_id: bool = conn
        .prepare("SELECT tenant_id FROM sessions LIMIT 0")
        .is_ok();

    if !has_tenant_id {
        conn.execute_batch("ALTER TABLE sessions ADD COLUMN tenant_id TEXT DEFAULT NULL;")?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_sessions_tenant ON sessions(tenant_id);",
        )?;
    }

    // Add tenant_id column to messages table (if missing)
    let has_msg_tenant: bool = conn
        .prepare("SELECT tenant_id FROM messages LIMIT 0")
        .is_ok();

    if !has_msg_tenant {
        conn.execute_batch("ALTER TABLE messages ADD COLUMN tenant_id TEXT DEFAULT NULL;")?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_messages_tenant ON messages(tenant_id);",
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_isolation_tier_default() {
        assert_eq!(IsolationTier::default(), IsolationTier::Shared);
    }

    #[test]
    fn test_isolation_tier_display() {
        assert_eq!(IsolationTier::Shared.to_string(), "shared");
        assert_eq!(IsolationTier::DedicatedDb.to_string(), "dedicated_db");
    }

    #[test]
    fn test_isolation_tier_from_str() {
        assert_eq!(
            IsolationTier::from_str_lossy("shared"),
            IsolationTier::Shared
        );
        assert_eq!(
            IsolationTier::from_str_lossy("dedicated_db"),
            IsolationTier::DedicatedDb
        );
        assert_eq!(
            IsolationTier::from_str_lossy("dedicated"),
            IsolationTier::DedicatedDb
        );
        assert_eq!(
            IsolationTier::from_str_lossy("schema"),
            IsolationTier::DedicatedDb
        );
        assert_eq!(
            IsolationTier::from_str_lossy("process"),
            IsolationTier::DedicatedDb
        );
        assert_eq!(
            IsolationTier::from_str_lossy("unknown"),
            IsolationTier::Shared
        );
    }

    #[test]
    fn test_isolation_tier_serde_roundtrip() {
        let tier = IsolationTier::DedicatedDb;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"dedicated_db\"");
        let parsed: IsolationTier = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, IsolationTier::DedicatedDb);
    }

    #[test]
    fn test_isolation_config_default() {
        let config = IsolationConfig::default();
        assert_eq!(config.default_tier, IsolationTier::Shared);
        assert_eq!(
            config.dedicated_db_base_path,
            PathBuf::from(".clawdius/tenants")
        );
        assert_eq!(config.shared_pool_config.max_size, 32);
        assert_eq!(config.dedicated_pool_config.max_size, 8);
    }

    #[test]
    fn test_manager_shared_tenant() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let manager = TenantIsolationManager::new(&shared_db, IsolationConfig::default()).unwrap();

        manager
            .register_tenant("t1", IsolationTier::Shared)
            .unwrap();
        assert_eq!(manager.get_tier("t1"), IsolationTier::Shared);

        let store = manager.resolve_store("t1");
        // Should be the same Arc as shared store
        assert!(Arc::ptr_eq(&store, &manager.shared_store()));
    }

    #[test]
    fn test_manager_dedicated_tenant() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let config = IsolationConfig {
            dedicated_db_base_path: tmp.path().join("tenants").to_path_buf(),
            ..Default::default()
        };
        let manager = TenantIsolationManager::new(&shared_db, config).unwrap();

        manager
            .register_tenant("t1", IsolationTier::DedicatedDb)
            .unwrap();
        assert_eq!(manager.get_tier("t1"), IsolationTier::DedicatedDb);

        let store = manager.resolve_store("t1");
        // Should NOT be the shared store
        assert!(!Arc::ptr_eq(&store, &manager.shared_store()));
    }

    #[test]
    fn test_manager_unregistered_tenant_defaults_to_shared() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let manager = TenantIsolationManager::new(&shared_db, IsolationConfig::default()).unwrap();

        assert_eq!(manager.get_tier("unknown"), IsolationTier::Shared);
        let store = manager.resolve_store("unknown");
        assert!(Arc::ptr_eq(&store, &manager.shared_store()));
    }

    #[test]
    fn test_manager_list_and_remove_tenants() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let manager = TenantIsolationManager::new(&shared_db, IsolationConfig::default()).unwrap();

        manager
            .register_tenant("t1", IsolationTier::Shared)
            .unwrap();
        manager
            .register_tenant("t2", IsolationTier::DedicatedDb)
            .unwrap();

        let mut tenants = manager.list_tenants();
        tenants.sort();
        assert_eq!(tenants, vec!["t1", "t2"]);

        manager.remove_tenant("t1");
        assert_eq!(manager.get_tier("t1"), IsolationTier::Shared); // defaults back
        assert_eq!(manager.list_tenants().len(), 1);
    }

    #[test]
    fn test_manager_set_tier_migration() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let config = IsolationConfig {
            dedicated_db_base_path: tmp.path().join("tenants").to_path_buf(),
            ..Default::default()
        };
        let manager = TenantIsolationManager::new(&shared_db, config).unwrap();

        manager
            .register_tenant("t1", IsolationTier::Shared)
            .unwrap();
        assert!(Arc::ptr_eq(
            &manager.resolve_store("t1"),
            &manager.shared_store()
        ));

        // Migrate to dedicated
        manager.set_tier("t1", IsolationTier::DedicatedDb).unwrap();
        assert_eq!(manager.get_tier("t1"), IsolationTier::DedicatedDb);
        assert!(!Arc::ptr_eq(
            &manager.resolve_store("t1"),
            &manager.shared_store()
        ));
    }

    #[test]
    fn test_dedicated_db_path() {
        let tmp = TempDir::new().unwrap();
        let shared_db = tmp.path().join("shared.db");
        let config = IsolationConfig {
            dedicated_db_base_path: tmp.path().join("tenants").to_path_buf(),
            ..Default::default()
        };
        let manager = TenantIsolationManager::new(&shared_db, config).unwrap();

        let path = manager.dedicated_db_path("acme-corp");
        assert!(path.ends_with("acme-corp.db"));
        assert!(path.starts_with(tmp.path()));
    }

    #[test]
    fn test_ensure_tenant_columns() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let pool = create_pool(&db_path, &ConnectionPoolConfig::default()).unwrap();

        // Should succeed even on fresh database (adds columns)
        ensure_tenant_columns(&pool).unwrap();

        // Should be idempotent (running again is fine)
        ensure_tenant_columns(&pool).unwrap();
    }
}
