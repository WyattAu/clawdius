//! Per-model resource caps via the orchestrator's [`ResourceGovernor`].
//!
//! The evolution module's cost-cap intent ("cost caps come from the
//! resource governor", [`crate::quota`] design notes in `evolution`) applied
//! to the chat gateway: every LLM call is keyed by its model and gated
//! against a per-model [`Quota`] — concurrent requests, tasks/hour,
//! tasks/day. Quotas come from the `CLAWDIUS_MODEL_QUOTAS` env var (JSON
//! map of model key → quota); unset means no gating. Over-quota calls map
//! to [`GatewayError::RateLimited`] with a 60-second retry hint.

use std::collections::HashMap;
use std::sync::Arc;

use crate::adapter::Platform;
use crate::error::GatewayError;
use clawdius_core::error::Error as CoreError;
use clawdius_core::orchestrator::{Quota, ResourceGovernor};

/// How long an over-quota sender should wait before retrying.
const QUOTA_RETRY_AFTER_MS: u64 = 60_000;

/// Parse the `CLAWDIUS_MODEL_QUOTAS` JSON map.
///
/// Shape: `{"<model key>": {full Quota object}}` — every field of
/// [`Quota`] must be present (no partial merges, no surprises).
///
/// # Errors
///
/// A string reason when the payload is not a JSON object of full quotas.
pub fn parse_model_quotas(json: &str) -> Result<HashMap<String, Quota>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = value
        .as_object()
        .ok_or("expected a JSON object keyed by model")?;
    let mut quotas = HashMap::with_capacity(obj.len());
    for (model, quota) in obj {
        let parsed: Quota = serde_json::from_value(quota.clone())
            .map_err(|e| format!("quota for '{model}' is not a full Quota: {e}"))?;
        quotas.insert(model.clone(), parsed);
    }
    Ok(quotas)
}

/// Build the governor from `CLAWDIUS_MODEL_QUOTAS`.
///
/// `None` when the env var is unset, empty, or invalid — the gateway runs
/// ungated rather than guessing caps.
#[must_use]
pub fn governor_from_env() -> Option<Arc<ResourceGovernor>> {
    let raw = std::env::var("CLAWDIUS_MODEL_QUOTAS").ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    match parse_model_quotas(&raw) {
        Ok(quotas) if !quotas.is_empty() => Some(Arc::new(ResourceGovernor::new(quotas))),
        Ok(_) => None,
        Err(reason) => {
            tracing::warn!(
                %reason,
                "CLAWDIUS_MODEL_QUOTAS ignored — per-model caps disabled"
            );
            None
        },
    }
}

/// Reserve one model slot for a chat request.
///
/// # Errors
///
/// [`GatewayError::RateLimited`] when the model's quota is exhausted
/// (retry after 60s); [`GatewayError::Agent`] for other governor errors.
pub async fn acquire_model_slot(
    governor: &ResourceGovernor,
    model_key: &str,
    task_id: &str,
    user_id: &str,
    platform: Platform,
) -> Result<(), GatewayError> {
    match governor.acquire(model_key, task_id).await {
        Ok(()) => Ok(()),
        Err(CoreError::QuotaExceeded(_)) => Err(GatewayError::RateLimited {
            user_id: user_id.to_string(),
            platform: platform.as_str().to_string(),
            retry_after_ms: QUOTA_RETRY_AFTER_MS,
        }),
        Err(e) => Err(GatewayError::Agent(format!("resource governor: {e}"))),
    }
}

/// Release a model slot after the chat call finishes (either outcome).
pub async fn release_model_slot(governor: &ResourceGovernor, model_key: &str, task_id: &str) {
    governor.release(model_key, task_id).await;
}

/// The governor key for one chat request.
#[must_use]
pub fn model_key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Env mutations are process-global — serialize the tests that touch
    /// them (the ENV_LOCK lesson from the aileron flake).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn full_quota(concurrent: usize) -> Quota {
        Quota {
            max_concurrent_tasks: concurrent,
            max_ram_per_task: 1_024,
            max_execution_time_ms: 60_000,
            max_tasks_per_hour: 100,
            max_tasks_per_day: 1_000,
        }
    }

    #[test]
    fn parse_full_quota_map() {
        let json = r#"{
            "openai/gpt-5": {
                "max_concurrent_tasks": 2,
                "max_ram_per_task": 1073741824,
                "max_execution_time_ms": 300000,
                "max_tasks_per_hour": 30,
                "max_tasks_per_day": 200
            },
            "zai/glm-5.3-flash": {
                "max_concurrent_tasks": 4,
                "max_ram_per_task": 1073741824,
                "max_execution_time_ms": 600000,
                "max_tasks_per_hour": 100,
                "max_tasks_per_day": 1000
            }
        }"#;
        let quotas = parse_model_quotas(json).expect("valid map");
        assert_eq!(quotas.len(), 2);
        assert_eq!(quotas["openai/gpt-5"].max_concurrent_tasks, 2);
        assert_eq!(quotas["zai/glm-5.3-flash"].max_tasks_per_day, 1000);
    }

    #[test]
    fn parse_rejects_invalid_shapes() {
        assert!(parse_model_quotas("not json").is_err());
        assert!(parse_model_quotas("[1,2]").is_err());
        // A quota missing fields must fail closed, not default silently.
        assert!(parse_model_quotas(r#"{"m": {"max_concurrent_tasks": 2}}"#).is_err());
    }

    #[tokio::test]
    async fn env_gating_is_opt_in() {
        {
            let _guard = ENV_LOCK.lock().expect("env lock");
            std::env::remove_var("CLAWDIUS_MODEL_QUOTAS");
            assert!(governor_from_env().is_none(), "unset = ungated");

            std::env::set_var("CLAWDIUS_MODEL_QUOTAS", "   ");
            assert!(governor_from_env().is_none(), "blank = ungated");

            std::env::set_var("CLAWDIUS_MODEL_QUOTAS", "{broken");
            assert!(
                governor_from_env().is_none(),
                "invalid = ungated (with a warning)"
            );

            std::env::set_var(
                "CLAWDIUS_MODEL_QUOTAS",
                r#"{"m/x": {"max_concurrent_tasks": 1, "max_ram_per_task": 1, "max_execution_time_ms": 1000, "max_tasks_per_hour": 2, "max_tasks_per_day": 5}}"#,
            );
        }
        let gov = governor_from_env().expect("valid env");
        // The quota is live: one slot, second acquire must bounce.
        assert!(gov.acquire("m/x", "t1").await.is_ok());
        assert!(gov.acquire("m/x", "t2").await.is_err(), "over quota");
        gov.release("m/x", "t1").await;
        assert!(gov.acquire("m/x", "t2").await.is_ok(), "slot freed");

        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("CLAWDIUS_MODEL_QUOTAS");
    }

    #[tokio::test]
    async fn over_quota_maps_to_rate_limited() {
        let mut quotas = HashMap::new();
        quotas.insert("m/tight".to_string(), full_quota(1));
        let gov = ResourceGovernor::new(quotas);

        acquire_model_slot(&gov, "m/tight", "a", "42", Platform::Telegram)
            .await
            .expect("first slot");

        let err = acquire_model_slot(&gov, "m/tight", "b", "42", Platform::Telegram)
            .await
            .expect_err("second slot over quota");
        assert!(
            matches!(err, GatewayError::RateLimited { ref retry_after_ms, .. } if *retry_after_ms == QUOTA_RETRY_AFTER_MS),
            "expected RateLimited(60s), got {err}"
        );

        release_model_slot(&gov, "m/tight", "a").await;
        acquire_model_slot(&gov, "m/tight", "b", "42", Platform::Telegram)
            .await
            .expect("slot freed after release");
    }

    #[tokio::test]
    async fn different_models_gate_independently() {
        let mut quotas = HashMap::new();
        quotas.insert("m/a".to_string(), full_quota(1));
        let gov = ResourceGovernor::new(quotas);

        acquire_model_slot(&gov, "m/a", "a", "42", Platform::Telegram)
            .await
            .expect("m/a slot");
        acquire_model_slot(&gov, "m/other", "b", "777", Platform::Telegram)
            .await
            .expect("unlisted model uses the default quota, independent");
        release_model_slot(&gov, "m/a", "a").await;
    }

    #[test]
    fn model_key_format() {
        assert_eq!(model_key("openai", "gpt-5"), "openai/gpt-5");
    }
}
