use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TenantTier {
    Free,
    Pro,
}

impl TenantTier {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            TenantTier::Free => "free",
            TenantTier::Pro => "pro",
        }
    }

    #[must_use]
    pub fn tasks_hour_limit(&self) -> u64 {
        match self {
            TenantTier::Free => 10,
            TenantTier::Pro => 100,
        }
    }

    #[must_use]
    pub fn tasks_day_limit(&self) -> u64 {
        match self {
            TenantTier::Free => 50,
            TenantTier::Pro => 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub tier: TenantTier,
    pub api_keys: Vec<String>,
    pub created_at: DateTime<Utc>,
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

    pub fn add_tenant(&mut self, tenant: Tenant) {
        self.tenants.insert(tenant.id.clone(), tenant);
    }

    #[must_use]
    pub fn get_tenant(&self, id: &str) -> Option<&Tenant> {
        self.tenants.get(id)
    }

    #[must_use]
    pub fn get_tenant_by_api_key(&self, key: &str) -> Option<&Tenant> {
        self.tenants
            .values()
            .find(|t| t.api_keys.iter().any(|k| k == key))
    }

    #[must_use]
    pub fn list_tenants(&self) -> Vec<&Tenant> {
        self.tenants.values().collect()
    }
}

impl Default for TenantStore {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn default_tenants() -> TenantStore {
    let mut store = TenantStore::new();
    store.add_tenant(Tenant {
        id: "default".to_string(),
        name: "Default".to_string(),
        tier: TenantTier::Free,
        api_keys: vec!["default-key".to_string()],
        created_at: Utc::now(),
    });
    store.add_tenant(Tenant {
        id: "demo".to_string(),
        name: "Demo".to_string(),
        tier: TenantTier::Free,
        api_keys: vec!["demo-key".to_string()],
        created_at: Utc::now(),
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
            api_keys: vec!["key-abc".to_string()],
            created_at: Utc::now(),
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
            name: "Alpha".to_string(),
            tier: TenantTier::Free,
            api_keys: vec!["key-1".to_string(), "key-2".to_string()],
            created_at: Utc::now(),
        });
        store.add_tenant(Tenant {
            id: "t2".to_string(),
            name: "Beta".to_string(),
            tier: TenantTier::Pro,
            api_keys: vec!["key-3".to_string()],
            created_at: Utc::now(),
        });

        assert_eq!(store.get_tenant_by_api_key("key-1").unwrap().id, "t1");
        assert_eq!(store.get_tenant_by_api_key("key-2").unwrap().id, "t1");
        assert_eq!(store.get_tenant_by_api_key("key-3").unwrap().id, "t2");
        assert!(store.get_tenant_by_api_key("unknown").is_none());
    }

    #[test]
    fn test_default_tenants_exist() {
        let store = default_tenants();
        assert!(store.get_tenant("default").is_some());
        assert!(store.get_tenant("demo").is_some());
        assert_eq!(store.list_tenants().len(), 2);

        let default = store.get_tenant("default").unwrap();
        assert_eq!(default.tier, TenantTier::Free);
        assert!(default.api_keys.contains(&"default-key".to_string()));

        let demo = store.get_tenant("demo").unwrap();
        assert_eq!(demo.name, "Demo");
    }

    #[test]
    fn test_tenant_tier_limits() {
        assert_eq!(TenantTier::Free.tasks_hour_limit(), 10);
        assert_eq!(TenantTier::Free.tasks_day_limit(), 50);
        assert_eq!(TenantTier::Pro.tasks_hour_limit(), 100);
        assert_eq!(TenantTier::Pro.tasks_day_limit(), 1000);
    }
}
