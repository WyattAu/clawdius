use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantResourceBudget {
    pub max_sessions: usize,
    pub max_messages_per_session: usize,
    pub max_message_size_bytes: usize,
    pub max_storage_bytes: usize,
    pub max_concurrent_agents: usize,
    pub max_agent_wall_time_secs: u64,
    pub max_agent_total_tokens: usize,
}

impl Default for TenantResourceBudget {
    fn default() -> Self {
        Self {
            max_sessions: 100,
            max_messages_per_session: 10_000,
            max_message_size_bytes: 1_048_576,
            max_storage_bytes: 104_857_600,
            max_concurrent_agents: 3,
            max_agent_wall_time_secs: 300,
            max_agent_total_tokens: 200_000,
        }
    }
}

impl TenantResourceBudget {
    #[must_use]
    pub const fn free_tier() -> Self {
        Self {
            max_sessions: 5,
            max_messages_per_session: 1_000,
            max_message_size_bytes: 512_000,
            max_storage_bytes: 10_485_760,
            max_concurrent_agents: 1,
            max_agent_wall_time_secs: 60,
            max_agent_total_tokens: 50_000,
        }
    }

    #[must_use]
    pub const fn pro_tier() -> Self {
        Self {
            max_sessions: 50,
            max_messages_per_session: 5_000,
            max_message_size_bytes: 1_048_576,
            max_storage_bytes: 104_857_600,
            max_concurrent_agents: 3,
            max_agent_wall_time_secs: 300,
            max_agent_total_tokens: 200_000,
        }
    }

    #[must_use]
    pub const fn enterprise_tier() -> Self {
        Self {
            max_sessions: 1_000,
            max_messages_per_session: 100_000,
            max_message_size_bytes: 10_485_760,
            max_storage_bytes: 10_737_418_240,
            max_concurrent_agents: 10,
            max_agent_wall_time_secs: 3_600,
            max_agent_total_tokens: 1_000_000,
        }
    }
}

pub struct TenantResourceUsage {
    session_count: AtomicU64,
    total_messages: AtomicU64,
    total_storage_bytes: AtomicU64,
    active_agents: AtomicU64,
    total_tokens_used: AtomicU64,
}

impl TenantResourceUsage {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            session_count: AtomicU64::new(0),
            total_messages: AtomicU64::new(0),
            total_storage_bytes: AtomicU64::new(0),
            active_agents: AtomicU64::new(0),
            total_tokens_used: AtomicU64::new(0),
        }
    }

    pub fn check_session_limit(
        &self,
        budget: &TenantResourceBudget,
    ) -> Result<(), ResourceExceededError> {
        let current = self.session_count.load(Ordering::Relaxed) as usize;
        if current >= budget.max_sessions {
            Err(ResourceExceededError::SessionLimit {
                current,
                max: budget.max_sessions,
            })
        } else {
            Ok(())
        }
    }

    pub fn check_message_limit(
        &self,
        session_messages: usize,
        message_size: usize,
        budget: &TenantResourceBudget,
    ) -> Result<(), ResourceExceededError> {
        if message_size > budget.max_message_size_bytes {
            return Err(ResourceExceededError::MessageTooLarge {
                size: message_size,
                max: budget.max_message_size_bytes,
            });
        }
        if session_messages >= budget.max_messages_per_session {
            return Err(ResourceExceededError::MessageLimit {
                current: session_messages,
                max: budget.max_messages_per_session,
            });
        }
        let current_storage = self.total_storage_bytes.load(Ordering::Relaxed) as usize;
        if current_storage.saturating_add(message_size) > budget.max_storage_bytes {
            return Err(ResourceExceededError::StorageLimit {
                current: current_storage,
                max: budget.max_storage_bytes,
            });
        }
        Ok(())
    }

    pub fn check_agent_limit(
        &self,
        budget: &TenantResourceBudget,
    ) -> Result<(), ResourceExceededError> {
        let current = self.active_agents.load(Ordering::Relaxed) as usize;
        if current >= budget.max_concurrent_agents {
            Err(ResourceExceededError::AgentLimit {
                current,
                max: budget.max_concurrent_agents,
            })
        } else {
            Ok(())
        }
    }

    pub fn record_session_created(&self) {
        self.session_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_session_deleted(&self) {
        self.session_count.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_message_added(&self, size: usize) {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.total_storage_bytes
            .fetch_add(size as u64, Ordering::Relaxed);
    }

    pub fn record_tokens_used(&self, tokens: u32) {
        self.total_tokens_used
            .fetch_add(u64::from(tokens), Ordering::Relaxed);
    }

    pub fn record_agent_started(&self) {
        self.active_agents.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_agent_stopped(&self) {
        self.active_agents.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Default for TenantResourceUsage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceExceededError {
    SessionLimit { current: usize, max: usize },
    MessageLimit { current: usize, max: usize },
    MessageTooLarge { size: usize, max: usize },
    StorageLimit { current: usize, max: usize },
    AgentLimit { current: usize, max: usize },
}

impl std::fmt::Display for ResourceExceededError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionLimit { current, max } => {
                write!(f, "session limit exceeded: {current}/{max}")
            },
            Self::MessageLimit { current, max } => {
                write!(f, "message limit exceeded: {current}/{max}")
            },
            Self::MessageTooLarge { size, max } => {
                write!(f, "message too large: {size} bytes (max {max})")
            },
            Self::StorageLimit { current, max } => {
                write!(f, "storage limit exceeded: {current}/{max} bytes")
            },
            Self::AgentLimit { current, max } => {
                write!(f, "agent limit exceeded: {current}/{max}")
            },
        }
    }
}

impl std::error::Error for ResourceExceededError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_defaults_free_lower_than_pro() {
        let free = TenantResourceBudget::free_tier();
        let pro = TenantResourceBudget::pro_tier();

        assert!(free.max_sessions < pro.max_sessions);
        assert!(free.max_messages_per_session < pro.max_messages_per_session);
        assert!(free.max_message_size_bytes < pro.max_message_size_bytes);
        assert!(free.max_storage_bytes < pro.max_storage_bytes);
        assert!(free.max_concurrent_agents < pro.max_concurrent_agents);
        assert!(free.max_agent_wall_time_secs < pro.max_agent_wall_time_secs);
        assert!(free.max_agent_total_tokens < pro.max_agent_total_tokens);
    }

    #[test]
    fn test_tier_defaults_pro_lower_than_enterprise() {
        let pro = TenantResourceBudget::pro_tier();
        let ent = TenantResourceBudget::enterprise_tier();

        assert!(pro.max_sessions < ent.max_sessions);
        assert!(pro.max_messages_per_session < ent.max_messages_per_session);
        assert!(pro.max_message_size_bytes < ent.max_message_size_bytes);
        assert!(pro.max_storage_bytes < ent.max_storage_bytes);
        assert!(pro.max_concurrent_agents < ent.max_concurrent_agents);
        assert!(pro.max_agent_wall_time_secs < ent.max_agent_wall_time_secs);
        assert!(pro.max_agent_total_tokens < ent.max_agent_total_tokens);
    }

    #[test]
    fn test_default_budget_values() {
        let budget = TenantResourceBudget::default();
        assert_eq!(budget.max_sessions, 100);
        assert_eq!(budget.max_messages_per_session, 10_000);
        assert_eq!(budget.max_message_size_bytes, 1_048_576);
        assert_eq!(budget.max_storage_bytes, 104_857_600);
        assert_eq!(budget.max_concurrent_agents, 3);
        assert_eq!(budget.max_agent_wall_time_secs, 300);
        assert_eq!(budget.max_agent_total_tokens, 200_000);
    }

    #[test]
    fn test_session_limit_enforcement() {
        let usage = TenantResourceUsage::new();
        let budget = TenantResourceBudget::free_tier();

        for _ in 0..budget.max_sessions {
            usage.record_session_created();
        }

        assert!(usage.check_session_limit(&budget).is_err());
        let err = usage.check_session_limit(&budget).unwrap_err();
        assert_eq!(
            err,
            ResourceExceededError::SessionLimit {
                current: budget.max_sessions,
                max: budget.max_sessions,
            }
        );

        usage.record_session_deleted();
        assert!(usage.check_session_limit(&budget).is_ok());
    }

    #[test]
    fn test_message_limit_enforcement() {
        let usage = TenantResourceUsage::new();
        let budget = TenantResourceBudget::free_tier();
        let session_messages = budget.max_messages_per_session;

        let result = usage.check_message_limit(session_messages, 100, &budget);
        assert!(matches!(
            result,
            Err(ResourceExceededError::MessageLimit { .. })
        ));

        let result = usage.check_message_limit(session_messages - 1, 100, &budget);
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_size_validation() {
        let usage = TenantResourceUsage::new();
        let budget = TenantResourceBudget::free_tier();

        let result = usage.check_message_limit(0, budget.max_message_size_bytes + 1, &budget);
        assert!(matches!(
            result,
            Err(ResourceExceededError::MessageTooLarge { .. })
        ));

        let result = usage.check_message_limit(0, budget.max_message_size_bytes, &budget);
        assert!(result.is_ok());
    }

    #[test]
    fn test_storage_limit_enforcement() {
        let usage = TenantResourceUsage::new();
        let budget = TenantResourceBudget {
            max_storage_bytes: 1000,
            ..TenantResourceBudget::free_tier()
        };

        usage.record_message_added(800);
        let result = usage.check_message_limit(0, 300, &budget);
        assert!(matches!(
            result,
            Err(ResourceExceededError::StorageLimit { .. })
        ));

        let result = usage.check_message_limit(0, 200, &budget);
        assert!(result.is_ok());
    }

    #[test]
    fn test_concurrent_agent_limit() {
        let usage = TenantResourceUsage::new();
        let budget = TenantResourceBudget::free_tier();

        for _ in 0..budget.max_concurrent_agents {
            usage.record_agent_started();
        }

        assert!(usage.check_agent_limit(&budget).is_err());
        let err = usage.check_agent_limit(&budget).unwrap_err();
        assert_eq!(
            err,
            ResourceExceededError::AgentLimit {
                current: budget.max_concurrent_agents,
                max: budget.max_concurrent_agents,
            }
        );

        usage.record_agent_stopped();
        assert!(usage.check_agent_limit(&budget).is_ok());
    }

    #[test]
    fn test_atomic_counter_increments() {
        let usage = TenantResourceUsage::new();

        usage.record_session_created();
        usage.record_session_created();
        usage.record_session_created();
        usage.record_session_deleted();
        assert_eq!(usage.session_count.load(Ordering::Relaxed), 2);

        usage.record_message_added(256);
        usage.record_message_added(512);
        assert_eq!(usage.total_messages.load(Ordering::Relaxed), 2);
        assert_eq!(usage.total_storage_bytes.load(Ordering::Relaxed), 768);

        usage.record_tokens_used(1500);
        usage.record_tokens_used(500);
        assert_eq!(usage.total_tokens_used.load(Ordering::Relaxed), 2000);

        usage.record_agent_started();
        usage.record_agent_stopped();
        assert_eq!(usage.active_agents.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_resource_exceeded_error_display() {
        let err = ResourceExceededError::SessionLimit { current: 5, max: 5 };
        assert_eq!(err.to_string(), "session limit exceeded: 5/5");

        let err = ResourceExceededError::MessageTooLarge {
            size: 2_000_000,
            max: 1_048_576,
        };
        assert_eq!(
            err.to_string(),
            "message too large: 2000000 bytes (max 1048576)"
        );
    }
}
