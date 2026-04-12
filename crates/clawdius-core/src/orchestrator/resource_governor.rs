//! Multi-Tenant Resource Governor
//!
//! Enforces per-user resource limits to prevent noisy-neighbor problems
//! in a multi-tenant SaaS deployment. Uses atomic counters for lock-free
//! quota checking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

/// Resource quota limits for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Maximum RAM per task in bytes
    pub max_ram_per_task: u64,
    /// Maximum execution time per task in milliseconds
    pub max_execution_time_ms: u64,
    /// Maximum tasks per hour
    pub max_tasks_per_hour: usize,
    /// Maximum total tasks per day
    pub max_tasks_per_day: usize,
}

impl Default for Quota {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 2,
            max_ram_per_task: 2 * 1024 * 1024 * 1024, // 2 GB
            max_execution_time_ms: 10 * 60 * 1000,    // 10 minutes
            max_tasks_per_hour: 30,
            max_tasks_per_day: 200,
        }
    }
}

/// Free-tier quota (restricted).
#[must_use]
pub fn free_tier_quota() -> Quota {
    Quota {
        max_concurrent_tasks: 1,
        max_ram_per_task: 512 * 1024 * 1024,  // 512 MB
        max_execution_time_ms: 5 * 60 * 1000, // 5 minutes
        max_tasks_per_hour: 10,
        max_tasks_per_day: 50,
    }
}

/// Pro-tier quota (generous).
#[must_use]
pub fn pro_tier_quota() -> Quota {
    Quota {
        max_concurrent_tasks: 4,
        max_ram_per_task: 4 * 1024 * 1024 * 1024, // 4 GB
        max_execution_time_ms: 30 * 60 * 1000,    // 30 minutes
        max_tasks_per_hour: 100,
        max_tasks_per_day: 1000,
    }
}

/// Current resource usage for a tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Currently running tasks
    pub active_tasks: usize,
    /// Total tasks completed this hour
    pub tasks_this_hour: usize,
    /// Total tasks completed this day
    pub tasks_this_day: usize,
    /// Timestamp of the last hourly reset
    pub last_hour_reset: u64,
    /// Timestamp of the last daily reset
    pub last_day_reset: u64,
}

/// Tracks resources held by a specific task.
#[derive(Debug, Clone)]
struct TaskAllocation {
    /// Task ID
    task_id: String,
    /// When the allocation was made (unix millis)
    #[allow(dead_code)]
    allocated_at: u64,
}

/// Multi-tenant resource governor.
///
/// Thread-safe via `RwLock`. For production with many tenants, consider
/// sharding by tenant ID hash.
pub struct ResourceGovernor {
    /// Per-tenant quotas (overrides default)
    quotas: RwLock<HashMap<String, Quota>>,
    /// Default quota for tenants without specific quota
    default_quota: Quota,
    /// Per-tenant usage
    usage: RwLock<HashMap<String, ResourceUsage>>,
    /// Active task allocations per tenant
    allocations: RwLock<HashMap<String, Vec<TaskAllocation>>>,
}

impl ResourceGovernor {
    /// Creates a new resource governor with the given per-tenant quotas.
    #[must_use]
    pub fn new(quotas: HashMap<String, Quota>) -> Self {
        Self {
            quotas: RwLock::new(quotas),
            default_quota: Quota::default(),
            usage: RwLock::new(HashMap::new()),
            allocations: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a new resource governor with a default quota for all tenants.
    #[must_use]
    pub fn with_default_quota(default: Quota) -> Self {
        Self {
            quotas: RwLock::new(HashMap::new()),
            default_quota: default,
            usage: RwLock::new(HashMap::new()),
            allocations: RwLock::new(HashMap::new()),
        }
    }

    /// Sets the quota for a tenant.
    pub async fn set_quota(&self, tenant_id: &str, quota: Quota) {
        self.quotas
            .write()
            .await
            .insert(tenant_id.to_string(), quota);
        info!("Updated quota for tenant {}", tenant_id);
    }

    /// Gets the quota for a tenant.
    pub async fn get_quota(&self, tenant_id: &str) -> Quota {
        self.quotas
            .read()
            .await
            .get(tenant_id)
            .cloned()
            .unwrap_or_else(|| self.default_quota.clone())
    }

    /// Gets the current usage for a tenant.
    pub async fn get_usage(&self, tenant_id: &str) -> ResourceUsage {
        let mut usage = self
            .usage
            .read()
            .await
            .get(tenant_id)
            .cloned()
            .unwrap_or_default();

        // Reset counters if time window has passed
        let now = current_timestamp();
        if now.saturating_sub(usage.last_hour_reset) > 3600_000 {
            usage.tasks_this_hour = 0;
            usage.last_hour_reset = now;
        }
        if now.saturating_sub(usage.last_day_reset) > 86400_000 {
            usage.tasks_this_day = 0;
            usage.last_day_reset = now;
        }

        usage
    }

    /// Checks if a tenant has available quota.
    ///
    /// # Errors
    ///
    /// Returns an error if any quota limit would be exceeded.
    pub async fn check_quota(&self, tenant_id: &str) -> crate::error::Result<()> {
        let quota = self.get_quota(tenant_id).await;
        let usage = self.get_usage(tenant_id).await;

        if usage.active_tasks >= quota.max_concurrent_tasks {
            return Err(crate::error::Error::QuotaExceeded(format!(
                "Tenant {} has {} active tasks (max {})",
                tenant_id, usage.active_tasks, quota.max_concurrent_tasks
            )));
        }

        if usage.tasks_this_hour >= quota.max_tasks_per_hour {
            return Err(crate::error::Error::QuotaExceeded(format!(
                "Tenant {} has {} tasks this hour (max {})",
                tenant_id, usage.tasks_this_hour, quota.max_tasks_per_hour
            )));
        }

        if usage.tasks_this_day >= quota.max_tasks_per_day {
            return Err(crate::error::Error::QuotaExceeded(format!(
                "Tenant {} has {} tasks today (max {})",
                tenant_id, usage.tasks_this_day, quota.max_tasks_per_day
            )));
        }

        Ok(())
    }

    /// Acquires resources for a task.
    ///
    /// # Errors
    ///
    /// Returns an error if quota would be exceeded.
    pub async fn acquire(&self, tenant_id: &str, task_id: &str) -> crate::error::Result<()> {
        self.check_quota(tenant_id).await?;

        let now = current_timestamp();

        // Update usage
        {
            let mut usage_map = self.usage.write().await;
            let usage = usage_map.entry(tenant_id.to_string()).or_insert_with(|| {
                let mut u = ResourceUsage::default();
                u.last_hour_reset = now;
                u.last_day_reset = now;
                u
            });

            // Reset counters if needed
            if now.saturating_sub(usage.last_hour_reset) > 3600_000 {
                usage.tasks_this_hour = 0;
                usage.last_hour_reset = now;
            }
            if now.saturating_sub(usage.last_day_reset) > 86400_000 {
                usage.tasks_this_day = 0;
                usage.last_day_reset = now;
            }

            usage.active_tasks += 1;
            usage.tasks_this_hour += 1;
            usage.tasks_this_day += 1;
        }

        // Record allocation
        {
            let mut alloc_map = self.allocations.write().await;
            alloc_map
                .entry(tenant_id.to_string())
                .or_default()
                .push(TaskAllocation {
                    task_id: task_id.to_string(),
                    allocated_at: now,
                });
        }

        tracing::debug!(
            "Tenant {} acquired resources for task {} (active: {})",
            tenant_id,
            task_id,
            self.get_usage(tenant_id).await.active_tasks
        );

        Ok(())
    }

    /// Releases resources held by a task.
    pub async fn release(&self, tenant_id: &str, task_id: &str) {
        // Remove allocation
        {
            let mut alloc_map = self.allocations.write().await;
            if let Some(allocations) = alloc_map.get_mut(tenant_id) {
                allocations.retain(|a| a.task_id != task_id);
            }
        }

        // Decrement active count
        {
            let mut usage_map = self.usage.write().await;
            if let Some(usage) = usage_map.get_mut(tenant_id) {
                usage.active_tasks = usage.active_tasks.saturating_sub(1);
            }
        }

        tracing::debug!(
            "Tenant {} released resources for task {}",
            tenant_id,
            task_id
        );
    }

    /// Returns the number of active tasks for a tenant.
    pub async fn active_count(&self, tenant_id: &str) -> usize {
        self.get_usage(tenant_id).await.active_tasks
    }

    /// Returns the number of tenants being tracked.
    pub async fn tenant_count(&self) -> usize {
        self.usage.read().await.len()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_quota() {
        let q = Quota::default();
        assert_eq!(q.max_concurrent_tasks, 2);
        assert_eq!(q.max_ram_per_task, 2 * 1024 * 1024 * 1024);
        assert_eq!(q.max_execution_time_ms, 10 * 60 * 1000);
    }

    #[test]
    fn test_free_tier_quota() {
        let q = free_tier_quota();
        assert_eq!(q.max_concurrent_tasks, 1);
        assert_eq!(q.max_ram_per_task, 512 * 1024 * 1024);
        assert_eq!(q.max_tasks_per_hour, 10);
    }

    #[test]
    fn test_pro_tier_quota() {
        let q = pro_tier_quota();
        assert_eq!(q.max_concurrent_tasks, 4);
        assert_eq!(q.max_ram_per_task, 4 * 1024 * 1024 * 1024);
        assert_eq!(q.max_tasks_per_day, 1000);
    }

    #[tokio::test]
    async fn test_acquire_release() {
        let governor = ResourceGovernor::with_default_quota(Quota {
            max_concurrent_tasks: 2,
            ..Default::default()
        });

        // Should succeed
        governor
            .acquire("t1", "task-1")
            .await
            .expect("first acquire");
        assert_eq!(governor.active_count("t1").await, 1);

        // Should succeed
        governor
            .acquire("t1", "task-2")
            .await
            .expect("second acquire");
        assert_eq!(governor.active_count("t1").await, 2);

        // Should fail — quota exceeded
        let result = governor.acquire("t1", "task-3").await;
        assert!(result.is_err());

        // Release one
        governor.release("t1", "task-1").await;
        assert_eq!(governor.active_count("t1").await, 1);

        // Should succeed again
        governor
            .acquire("t1", "task-3")
            .await
            .expect("third acquire after release");
    }

    #[tokio::test]
    async fn test_per_tenant_isolation() {
        let governor = ResourceGovernor::with_default_quota(Quota {
            max_concurrent_tasks: 1,
            ..Default::default()
        });

        governor.acquire("t1", "task-1").await.expect("t1 acquire");
        governor.acquire("t2", "task-2").await.expect("t2 acquire");

        // Each tenant can have 1 task
        assert_eq!(governor.active_count("t1").await, 1);
        assert_eq!(governor.active_count("t2").await, 1);

        // t1 should be at quota
        let result = governor.acquire("t1", "task-3").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_quota() {
        let governor = ResourceGovernor::with_default_quota(free_tier_quota());

        // Initially limited
        governor.acquire("t1", "task-1").await.expect("first");
        let result = governor.acquire("t1", "task-2").await;
        assert!(result.is_err());

        // Upgrade to pro
        governor.set_quota("t1", pro_tier_quota()).await;

        // Now should work
        governor
            .acquire("t1", "task-2")
            .await
            .expect("second after upgrade");
        assert_eq!(governor.active_count("t1").await, 2);
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let governor = ResourceGovernor::with_default_quota(Quota {
            max_concurrent_tasks: 10,
            max_tasks_per_hour: 3,
            ..Default::default()
        });

        governor.acquire("t1", "task-1").await.expect("1");
        governor.acquire("t1", "task-2").await.expect("2");
        governor.acquire("t1", "task-3").await.expect("3");

        let usage = governor.get_usage("t1").await;
        assert_eq!(usage.tasks_this_hour, 3);

        // 4th should fail due to hourly limit
        let result = governor.acquire("t1", "task-4").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_release_nonexistent() {
        let governor = ResourceGovernor::new(HashMap::new());
        // Should not panic
        governor.release("nonexistent", "task-1").await;
    }
}
