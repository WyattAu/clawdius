use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantPoolConfig {
    pub max_connections_per_tenant: usize,
    pub max_global_connections: usize,
    pub idle_timeout: Duration,
    pub max_tenants: usize,
}

impl Default for TenantPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_tenant: 4,
            max_global_connections: 100,
            idle_timeout: Duration::from_secs(300),
            max_tenants: 1000,
        }
    }
}

#[derive(Debug)]
struct TenantConnectionPoolInner {
    tenant_connections: HashMap<String, usize>,
    global_connections: usize,
    tenant_activity: HashMap<String, Instant>,
}

pub struct TenantConnectionPool {
    inner: Arc<Mutex<TenantConnectionPoolInner>>,
    config: TenantPoolConfig,
}

#[derive(Debug)]
pub struct ConnectionPermit {
    tenant_id: String,
    pool: Arc<Mutex<TenantConnectionPoolInner>>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.pool.lock() {
            let count = inner
                .tenant_connections
                .get(&self.tenant_id)
                .copied()
                .unwrap_or(0);
            if count <= 1 {
                inner.tenant_connections.remove(&self.tenant_id);
            } else {
                *inner.tenant_connections.get_mut(&self.tenant_id).unwrap() = count - 1;
                inner
                    .tenant_activity
                    .insert(self.tenant_id.clone(), Instant::now());
            }
            inner.global_connections = inner.global_connections.saturating_sub(1);
        }
    }
}

impl TenantConnectionPool {
    #[must_use] 
    pub fn new(config: TenantPoolConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TenantConnectionPoolInner {
                tenant_connections: HashMap::new(),
                global_connections: 0,
                tenant_activity: HashMap::new(),
            })),
            config,
        }
    }

    pub fn acquire(&self, tenant_id: &str) -> Result<ConnectionPermit> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| crate::Error::Config(format!("pool lock poisoned: {e}")))?;

        let tenant_count = inner
            .tenant_connections
            .get(tenant_id)
            .copied()
            .unwrap_or(0);

        if tenant_count >= self.config.max_connections_per_tenant {
            return Err(crate::Error::RateLimited { retry_after_ms: 0 });
        }

        if inner.global_connections >= self.config.max_global_connections {
            return Err(crate::Error::Config(format!(
                "global connection limit reached ({}/{})",
                inner.global_connections, self.config.max_global_connections
            )));
        }

        if !inner.tenant_connections.contains_key(tenant_id)
            && inner.tenant_connections.len() >= self.config.max_tenants
        {
            return Err(crate::Error::Config(format!(
                "max tenant limit reached ({})",
                self.config.max_tenants
            )));
        }

        *inner
            .tenant_connections
            .entry(tenant_id.to_string())
            .or_insert(0) += 1;
        inner.global_connections += 1;
        inner
            .tenant_activity
            .insert(tenant_id.to_string(), Instant::now());

        Ok(ConnectionPermit {
            tenant_id: tenant_id.to_string(),
            pool: Arc::clone(&self.inner),
        })
    }

    #[must_use] 
    pub fn available_slots(&self, tenant_id: &str) -> (usize, usize) {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let tenant_used = inner
            .tenant_connections
            .get(tenant_id)
            .copied()
            .unwrap_or(0);
        let tenant_available = self
            .config
            .max_connections_per_tenant
            .saturating_sub(tenant_used);
        let global_available = self
            .config
            .max_global_connections
            .saturating_sub(inner.global_connections);
        (tenant_available, global_available)
    }

    #[must_use] 
    pub fn tenant_usage(&self, tenant_id: &str) -> usize {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner
            .tenant_connections
            .get(tenant_id)
            .copied()
            .unwrap_or(0)
    }

    #[must_use] 
    pub fn global_usage(&self) -> usize {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.global_connections
    }

    #[must_use] 
    pub fn active_tenants(&self) -> usize {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.tenant_connections.len()
    }

    pub fn evict_idle_tenants(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let idle_tenants: Vec<String> = inner
            .tenant_activity
            .iter()
            .filter(|(_, &last_active)| now.duration_since(last_active) > self.config.idle_timeout)
            .map(|(id, _)| id.clone())
            .collect();

        for tenant_id in idle_tenants {
            if inner
                .tenant_connections
                .get(&tenant_id)
                .copied()
                .unwrap_or(0)
                == 0
            {
                inner.tenant_connections.remove(&tenant_id);
                inner.tenant_activity.remove(&tenant_id);
            }
        }
    }

    #[must_use] 
    pub fn tenant_count(&self) -> usize {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.tenant_activity.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TenantPoolConfig {
        TenantPoolConfig {
            max_connections_per_tenant: 2,
            max_global_connections: 5,
            idle_timeout: Duration::from_millis(100),
            max_tenants: 10,
        }
    }

    #[test]
    fn test_new_pool() {
        let pool = TenantConnectionPool::new(TenantPoolConfig::default());
        assert_eq!(pool.global_usage(), 0);
        assert_eq!(pool.active_tenants(), 0);
        assert_eq!(pool.tenant_count(), 0);
        let (tenant_avail, global_avail) = pool.available_slots("t1");
        assert_eq!(tenant_avail, 4);
        assert_eq!(global_avail, 100);
    }

    #[test]
    fn test_acquire_and_release() {
        let pool = TenantConnectionPool::new(test_config());
        let permit = pool.acquire("t1").expect("should acquire");
        assert_eq!(pool.tenant_usage("t1"), 1);
        assert_eq!(pool.global_usage(), 1);
        drop(permit);
        assert_eq!(pool.tenant_usage("t1"), 0);
        assert_eq!(pool.global_usage(), 0);
    }

    #[test]
    fn test_tenant_limit_enforced() {
        let pool = TenantConnectionPool::new(test_config());
        let _p1 = pool.acquire("t1").unwrap();
        let _p2 = pool.acquire("t1").unwrap();
        let result = pool.acquire("t1");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::Error::RateLimited { .. } => {},
            other => panic!("expected RateLimited, got {other}"),
        }
    }

    #[test]
    fn test_global_limit_enforced() {
        let config = TenantPoolConfig {
            max_connections_per_tenant: 10,
            max_global_connections: 2,
            idle_timeout: Duration::from_secs(300),
            max_tenants: 100,
        };
        let pool = TenantConnectionPool::new(config);
        let _p1 = pool.acquire("t1").unwrap();
        let _p2 = pool.acquire("t2").unwrap();
        let result = pool.acquire("t3");
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::Error::Config(msg) => assert!(msg.contains("global")),
            other => panic!("expected Config, got {other}"),
        }
    }

    #[test]
    fn test_available_slots() {
        let pool = TenantConnectionPool::new(test_config());
        let _p1 = pool.acquire("t1").unwrap();
        let (tenant_avail, global_avail) = pool.available_slots("t1");
        assert_eq!(tenant_avail, 1);
        assert_eq!(global_avail, 4);
        let (tenant_avail_new, _) = pool.available_slots("t2");
        assert_eq!(tenant_avail_new, 2);
    }

    #[test]
    fn test_multiple_tenants_independent() {
        let pool = TenantConnectionPool::new(test_config());
        let _p1 = pool.acquire("t1").unwrap();
        let _p2 = pool.acquire("t1").unwrap();
        let _p3 = pool.acquire("t2").unwrap();
        assert_eq!(pool.tenant_usage("t1"), 2);
        assert_eq!(pool.tenant_usage("t2"), 1);
        assert_eq!(pool.global_usage(), 3);
        let result = pool.acquire("t1");
        assert!(result.is_err());
        let result = pool.acquire("t2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_evict_idle_tenants() {
        let pool = TenantConnectionPool::new(test_config());
        let p1 = pool.acquire("t1").unwrap();
        let p2 = pool.acquire("t2").unwrap();
        assert_eq!(pool.tenant_count(), 2);
        drop(p1);
        drop(p2);
        assert_eq!(pool.tenant_count(), 2);
        std::thread::sleep(Duration::from_millis(150));
        pool.evict_idle_tenants();
        assert_eq!(pool.tenant_count(), 0);
        assert_eq!(pool.active_tenants(), 0);
    }

    #[test]
    fn test_tenant_usage_tracking() {
        let pool = TenantConnectionPool::new(test_config());
        assert_eq!(pool.tenant_usage("t1"), 0);
        let p1 = pool.acquire("t1").unwrap();
        assert_eq!(pool.tenant_usage("t1"), 1);
        let p2 = pool.acquire("t1").unwrap();
        assert_eq!(pool.tenant_usage("t1"), 2);
        drop(p1);
        assert_eq!(pool.tenant_usage("t1"), 1);
        drop(p2);
        assert_eq!(pool.tenant_usage("t1"), 0);
        assert_eq!(pool.global_usage(), 0);
    }
}
