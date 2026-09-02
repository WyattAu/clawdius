use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenantTier {
    Free,
    Pro,
    Enterprise,
}

impl TenantTier {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            TenantTier::Free => "free",
            TenantTier::Pro => "pro",
            TenantTier::Enterprise => "enterprise",
        }
    }

    #[must_use]
    pub fn tasks_hour_limit(&self) -> u64 {
        match self {
            TenantTier::Free => 10,
            TenantTier::Pro => 100,
            TenantTier::Enterprise => 10_000,
        }
    }

    #[must_use]
    pub fn tasks_day_limit(&self) -> u64 {
        match self {
            TenantTier::Free => 50,
            TenantTier::Pro => 1_000,
            TenantTier::Enterprise => 100_000,
        }
    }

    #[must_use]
    pub fn max_workspaces(&self) -> u64 {
        match self {
            TenantTier::Free => 1,
            TenantTier::Pro => 10,
            TenantTier::Enterprise => 1_000,
        }
    }

    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "free" => Some(TenantTier::Free),
            "pro" => Some(TenantTier::Pro),
            "enterprise" => Some(TenantTier::Enterprise),
            _ => None,
        }
    }
}

impl fmt::Display for TenantTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantUsage {
    pub tasks_total: u64,
    pub tasks_hour: u64,
    pub tasks_day: u64,
    pub tokens_total: u64,
    pub sessions_total: u64,
    pub sessions_active: u64,
    pub files_modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub tier: TenantTier,
    pub api_keys: Vec<ApiKeyEntry>,
    pub email: Option<String>,
    pub workspace_root: Option<String>,
    pub usage: TenantUsage,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub key: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub active: bool,
}

impl ApiKeyEntry {
    #[must_use]
    pub fn new(label: &str) -> Self {
        Self {
            key: generate_api_key(),
            label: label.to_string(),
            created_at: Utc::now(),
            last_used_at: Utc::now(),
            active: true,
        }
    }

    #[must_use]
    pub fn masked(&self) -> String {
        if self.key.len() <= 8 {
            return "*".repeat(self.key.len());
        }
        format!("{}...{}", &self.key[..6], &self.key[self.key.len() - 4..])
    }
}

#[must_use]
pub fn generate_api_key() -> String {
    let id = uuid::Uuid::new_v4();
    format!("ck_{}", id.simple())
}

pub struct TenantStore {
    tenants: HashMap<String, Tenant>,
}

impl TenantStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }

    /// Create a SQLite-backed tenant store.
    pub fn with_sqlite(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = rusqlite::Connection::open(db_path)?;

        // Create tables if they don't exist
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tenants (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                tier TEXT NOT NULL DEFAULT 'free',
                email TEXT,
                workspace_root TEXT,
                created_at TEXT NOT NULL,
                last_active_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS api_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                key TEXT NOT NULL UNIQUE,
                label TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                FOREIGN KEY (tenant_id) REFERENCES tenants(id)
            );

            CREATE TABLE IF NOT EXISTS tenant_usage (
                tenant_id TEXT PRIMARY KEY,
                tasks_total INTEGER NOT NULL DEFAULT 0,
                tasks_hour INTEGER NOT NULL DEFAULT 0,
                tasks_day INTEGER NOT NULL DEFAULT 0,
                tokens_total INTEGER NOT NULL DEFAULT 0,
                sessions_total INTEGER NOT NULL DEFAULT 0,
                sessions_active INTEGER NOT NULL DEFAULT 0,
                files_modified INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (tenant_id) REFERENCES tenants(id)
            );"
        )?;

        let mut store = Self::new();

        // Load tenants from database
        let mut stmt = conn.prepare("SELECT id, name, tier, email, workspace_root, created_at, last_active_at FROM tenants")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;

        for row in rows {
            let (id, name, tier_str, email, workspace_root, created_at, last_active_at) = row?;
            let tier = TenantTier::from_str_opt(&tier_str).unwrap_or(TenantTier::Free);

            // Load API keys for this tenant
            let mut key_stmt = conn.prepare("SELECT key, label, created_at, last_used_at, active FROM api_keys WHERE tenant_id = ?")?;
            let keys = key_stmt.query_map([&id], |row| {
                Ok(ApiKeyEntry {
                    key: row.get(0)?,
                    label: row.get(1)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_used_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    active: row.get::<_, i32>(4)? != 0,
                })
            })?.collect::<Result<Vec<_>, _>>()?;

            // Load usage
            let mut usage_stmt = conn.prepare("SELECT tasks_total, tasks_hour, tasks_day, tokens_total, sessions_total, sessions_active, files_modified FROM tenant_usage WHERE tenant_id = ?")?;
            let usage = usage_stmt.query_map([&id], |row| {
                Ok(TenantUsage {
                    tasks_total: row.get(0)?,
                    tasks_hour: row.get(1)?,
                    tasks_day: row.get(2)?,
                    tokens_total: row.get(3)?,
                    sessions_total: row.get(4)?,
                    sessions_active: row.get(5)?,
                    files_modified: row.get(6)?,
                })
            })?.next().transpose()?.unwrap_or_default();

            store.add_tenant(Tenant {
                id,
                name,
                tier,
                api_keys: keys,
                email,
                workspace_root,
                usage,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_active_at: chrono::DateTime::parse_from_rfc3339(&last_active_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            });
        }

        Ok(store)
    }

    pub fn add_tenant(&mut self, tenant: Tenant) {
        self.tenants.insert(tenant.id.clone(), tenant);
    }

    #[must_use]
    pub fn get_tenant(&self, id: &str) -> Option<&Tenant> {
        self.tenants.get(id)
    }

    #[must_use]
    pub fn get_tenant_mut(&mut self, id: &str) -> Option<&mut Tenant> {
        self.tenants.get_mut(id)
    }

    #[must_use]
    pub fn get_tenant_by_api_key(&self, key: &str) -> Option<&Tenant> {
        self.tenants
            .values()
            .find(|t| t.api_keys.iter().any(|k| k.key == key && k.active))
    }

    #[must_use]
    pub fn get_tenant_id_by_api_key(&self, key: &str) -> Option<String> {
        self.get_tenant_by_api_key(key).map(|t| t.id.clone())
    }

    #[must_use]
    pub fn list_tenants(&self) -> Vec<&Tenant> {
        self.tenants.values().collect()
    }

    pub fn update_tenant(
        &mut self,
        id: &str,
        name: Option<&str>,
        tier: Option<TenantTier>,
        email: Option<&str>,
        workspace_root: Option<&str>,
    ) -> Option<&Tenant> {
        let tenant = self.tenants.get_mut(id)?;
        if let Some(name) = name {
            tenant.name = name.to_string();
        }
        if let Some(tier) = tier {
            tenant.tier = tier;
        }
        if let Some(email) = email {
            tenant.email = Some(email.to_string());
        }
        if let Some(workspace_root) = workspace_root {
            tenant.workspace_root = Some(workspace_root.to_string());
        }
        Some(
            self.tenants
                .get(id)
                .expect("tenant exists — verified by get_mut above"),
        )
    }

    pub fn add_api_key(&mut self, tenant_id: &str, label: &str) -> Option<ApiKeyEntry> {
        let tenant = self.tenants.get_mut(tenant_id)?;
        let entry = ApiKeyEntry::new(label);
        let key_entry = entry.clone();
        tenant.api_keys.push(entry);
        Some(key_entry)
    }

    pub fn revoke_api_key(&mut self, tenant_id: &str, key: &str) -> bool {
        if let Some(tenant) = self.tenants.get_mut(tenant_id) {
            if let Some(entry) = tenant.api_keys.iter_mut().find(|k| k.key == key) {
                entry.active = false;
                return true;
            }
        }
        false
    }

    pub fn delete_tenant(&mut self, id: &str) -> bool {
        self.tenants.remove(id).is_some()
    }

    pub fn record_task(&mut self, tenant_id: &str, _tokens: usize) -> bool {
        if let Some(tenant) = self.tenants.get_mut(tenant_id) {
            let hour_limit = tenant.tier.tasks_hour_limit();
            let day_limit = tenant.tier.tasks_day_limit();

            if tenant.usage.tasks_hour >= hour_limit || tenant.usage.tasks_day >= day_limit {
                return false;
            }

            tenant.usage.tasks_total += 1;
            tenant.usage.tasks_hour += 1;
            tenant.usage.tasks_day += 1;
            tenant.last_active_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn record_session_start(&mut self, tenant_id: &str) -> bool {
        if let Some(tenant) = self.tenants.get_mut(tenant_id) {
            tenant.usage.sessions_total += 1;
            tenant.usage.sessions_active += 1;
            tenant.last_active_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn record_session_end(&mut self, tenant_id: &str) -> bool {
        if let Some(tenant) = self.tenants.get_mut(tenant_id) {
            if tenant.usage.sessions_active > 0 {
                tenant.usage.sessions_active -= 1;
            }
            tenant.last_active_at = Utc::now();
            true
        } else {
            false
        }
    }

    pub fn reset_hourly_counters(&mut self) {
        for tenant in self.tenants.values_mut() {
            tenant.usage.tasks_hour = 0;
        }
    }

    pub fn reset_daily_counters(&mut self) {
        for tenant in self.tenants.values_mut() {
            tenant.usage.tasks_day = 0;
        }
    }
}

impl Default for TenantStore {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn default_tenants() -> TenantStore {
    let now = Utc::now();
    let mut store = TenantStore::new();
    store.add_tenant(Tenant {
        id: "default".to_string(),
        name: "Default".to_string(),
        tier: TenantTier::Free,
        api_keys: vec![ApiKeyEntry {
            key: "default-key".to_string(),
            label: "default".to_string(),
            created_at: now,
            last_used_at: now,
            active: true,
        }],
        email: None,
        workspace_root: None,
        usage: TenantUsage::default(),
        created_at: now,
        last_active_at: now,
    });
    store.add_tenant(Tenant {
        id: "demo".to_string(),
        name: "Demo".to_string(),
        tier: TenantTier::Free,
        api_keys: vec![ApiKeyEntry {
            key: "demo-key".to_string(),
            label: "demo".to_string(),
            created_at: now,
            last_used_at: now,
            active: true,
        }],
        email: None,
        workspace_root: None,
        usage: TenantUsage::default(),
        created_at: now,
        last_active_at: now,
    });
    store
}

#[derive(Debug, Clone)]
pub struct AuthenticatedApiKey(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_retrieve_tenant() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "Test Tenant".to_string(),
            tier: TenantTier::Pro,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        let tenant = store.get_tenant("t1").unwrap();
        assert_eq!(tenant.name, "Test Tenant");
        assert_eq!(tenant.tier, TenantTier::Pro);
        assert!(store.get_tenant("nonexistent").is_none());
    }

    #[test]
    fn test_lookup_tenant_by_api_key() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "Test".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![ApiKeyEntry {
                key: "my-secret-key".to_string(),
                label: "prod".to_string(),
                created_at: Utc::now(),
                last_used_at: Utc::now(),
                active: true,
            }],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        assert!(store.get_tenant_by_api_key("my-secret-key").is_some());
        assert!(store.get_tenant_by_api_key("wrong-key").is_none());
        assert_eq!(
            store.get_tenant_id_by_api_key("my-secret-key"),
            Some("t1".to_string())
        );
    }

    #[test]
    fn test_default_tenants_exist() {
        let store = default_tenants();
        assert!(store.get_tenant("default").is_some());
        assert!(store.get_tenant("demo").is_some());
        assert_eq!(store.list_tenants().len(), 2);

        let default = store.get_tenant("default").unwrap();
        assert_eq!(default.tier, TenantTier::Free);
        assert!(default
            .api_keys
            .iter()
            .any(|k| k.key == "default-key" && k.active));

        let demo = store.get_tenant("demo").unwrap();
        assert_eq!(demo.name, "Demo");
    }

    #[test]
    fn test_tenant_tier_limits() {
        assert_eq!(TenantTier::Free.tasks_hour_limit(), 10);
        assert_eq!(TenantTier::Free.tasks_day_limit(), 50);
        assert_eq!(TenantTier::Free.max_workspaces(), 1);

        assert_eq!(TenantTier::Pro.tasks_hour_limit(), 100);
        assert_eq!(TenantTier::Pro.tasks_day_limit(), 1_000);
        assert_eq!(TenantTier::Pro.max_workspaces(), 10);

        assert_eq!(TenantTier::Enterprise.tasks_hour_limit(), 10_000);
        assert_eq!(TenantTier::Enterprise.tasks_day_limit(), 100_000);
        assert_eq!(TenantTier::Enterprise.max_workspaces(), 1_000);
    }

    #[test]
    fn test_api_key_generation() {
        let key = generate_api_key();
        assert!(
            key.starts_with("ck_"),
            "key should start with ck_ prefix: {key}"
        );
        assert!(key.len() > 10, "key should be long enough: {key}");

        let key2 = generate_api_key();
        assert_ne!(key, key2, "each generated key should be unique");
    }

    #[test]
    fn test_api_key_masking() {
        let entry = ApiKeyEntry {
            key: "ck_abcdef1234567890".to_string(),
            label: "test".to_string(),
            created_at: Utc::now(),
            last_used_at: Utc::now(),
            active: true,
        };
        let masked = entry.masked();
        assert!(masked.contains("..."), "masked key should contain '...'");
        assert!(
            !masked.contains("ck_abcdef1234567890"),
            "masked key should not contain full key"
        );
    }

    #[test]
    fn test_rate_limiting() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "free-tenant".to_string(),
            name: "Free".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        for _ in 0..10 {
            assert!(store.record_task("free-tenant", 100));
        }
        assert!(
            !store.record_task("free-tenant", 100),
            "11th task should be rate limited"
        );

        assert_eq!(
            store.get_tenant("free-tenant").unwrap().usage.tasks_total,
            10
        );
        assert_eq!(
            store.get_tenant("free-tenant").unwrap().usage.tasks_hour,
            10
        );
    }

    #[test]
    fn test_update_tenant() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "Old Name".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        store.update_tenant(
            "t1",
            Some("New Name"),
            Some(TenantTier::Pro),
            Some("test@example.com"),
            Some("/workspace"),
        );

        let tenant = store.get_tenant("t1").unwrap();
        assert_eq!(tenant.name, "New Name");
        assert_eq!(tenant.tier, TenantTier::Pro);
        assert_eq!(tenant.email.as_deref(), Some("test@example.com"));
        assert_eq!(tenant.workspace_root.as_deref(), Some("/workspace"));
    }

    #[test]
    fn test_revoke_api_key() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "Test".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: chrono::Utc::now(),
            last_active_at: chrono::Utc::now(),
        });
        let key_entry = store.add_api_key("t1", "production").unwrap();
        assert!(store.get_tenant_by_api_key(&key_entry.key).is_some());

        store.revoke_api_key("t1", &key_entry.key);
        assert!(store.get_tenant_by_api_key(&key_entry.key).is_none());
    }

    #[test]
    fn test_delete_tenant() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "To Delete".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        assert!(store.get_tenant("t1").is_some());
        assert!(store.delete_tenant("t1"));
        assert!(store.get_tenant("t1").is_none());
        assert!(
            !store.delete_tenant("t1"),
            "deleting non-existent should return false"
        );
    }

    #[test]
    fn test_session_tracking() {
        let mut store = TenantStore::new();
        store.add_tenant(Tenant {
            id: "t1".to_string(),
            name: "Session Test".to_string(),
            tier: TenantTier::Free,
            api_keys: vec![],
            email: None,
            workspace_root: None,
            usage: TenantUsage::default(),
            created_at: Utc::now(),
            last_active_at: Utc::now(),
        });

        assert!(store.record_session_start("t1"));
        assert!(store.record_session_start("t1"));
        assert_eq!(store.get_tenant("t1").unwrap().usage.sessions_active, 2);
        assert_eq!(store.get_tenant("t1").unwrap().usage.sessions_total, 2);

        store.record_session_end("t1");
        assert_eq!(store.get_tenant("t1").unwrap().usage.sessions_active, 1);

        assert!(!store.record_session_start("nonexistent"));
        assert!(!store.record_session_end("nonexistent"));
    }

    #[test]
    fn test_tenant_tier_from_str() {
        assert_eq!(TenantTier::from_str_opt("free"), Some(TenantTier::Free));
        assert_eq!(TenantTier::from_str_opt("Free"), Some(TenantTier::Free));
        assert_eq!(TenantTier::from_str_opt("FREE"), Some(TenantTier::Free));
        assert_eq!(TenantTier::from_str_opt("pro"), Some(TenantTier::Pro));
        assert_eq!(TenantTier::from_str_opt("Pro"), Some(TenantTier::Pro));
        assert_eq!(
            TenantTier::from_str_opt("enterprise"),
            Some(TenantTier::Enterprise)
        );
        assert_eq!(
            TenantTier::from_str_opt("Enterprise"),
            Some(TenantTier::Enterprise)
        );
        assert_eq!(TenantTier::from_str_opt("unknown"), None);
        assert_eq!(TenantTier::from_str_opt(""), None);

        assert_eq!(TenantTier::Free.as_str(), "free");
        assert_eq!(TenantTier::Pro.as_str(), "pro");
        assert_eq!(TenantTier::Enterprise.as_str(), "enterprise");

        assert_eq!(format!("{}", TenantTier::Free), "free");
        assert_eq!(format!("{}", TenantTier::Pro), "pro");
        assert_eq!(format!("{}", TenantTier::Enterprise), "enterprise");
    }
}
