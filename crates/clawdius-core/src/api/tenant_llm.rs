use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::llm::{create_provider, LlmConfig, LlmProvider};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantLlmConfig {
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: usize,
    pub enabled: bool,
}

#[derive(Debug, Default)]
pub struct TenantTokenUsage {
    pub input_tokens: AtomicU64,
    pub output_tokens: AtomicU64,
    pub total_requests: AtomicU64,
}

impl Clone for TenantTokenUsage {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl TenantTokenUsage {
    fn snapshot(&self) -> Self {
        Self {
            input_tokens: AtomicU64::new(self.input_tokens.load(Ordering::Relaxed)),
            output_tokens: AtomicU64::new(self.output_tokens.load(Ordering::Relaxed)),
            total_requests: AtomicU64::new(self.total_requests.load(Ordering::Relaxed)),
        }
    }
}

pub struct TenantLlmRegistry {
    configs: RwLock<HashMap<String, TenantLlmConfig>>,
    providers: RwLock<HashMap<String, Arc<LlmProvider>>>,
    default_provider: RwLock<Option<Arc<LlmProvider>>>,
    token_usage: RwLock<HashMap<String, TenantTokenUsage>>,
}

impl TenantLlmRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: RwLock::new(HashMap::new()),
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
            token_usage: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_default_provider(&self, provider: LlmProvider) {
        let mut default = self.default_provider.write().unwrap();
        *default = Some(Arc::new(provider));
    }

    pub fn register_tenant_config(&self, tenant_id: &str, config: TenantLlmConfig) {
        let mut configs = self.configs.write().unwrap();
        let mut providers = self.providers.write().unwrap();
        providers.remove(tenant_id);
        configs.insert(tenant_id.to_string(), config);
    }

    pub fn remove_tenant_config(&self, tenant_id: &str) {
        let mut configs = self.configs.write().unwrap();
        let mut providers = self.providers.write().unwrap();
        configs.remove(tenant_id);
        providers.remove(tenant_id);
    }

    #[must_use]
    pub fn get_tenant_config(&self, tenant_id: &str) -> Option<TenantLlmConfig> {
        let configs = self.configs.read().unwrap();
        configs.get(tenant_id).cloned()
    }

    pub fn resolve_provider(&self, tenant_id: Option<&str>) -> Result<Arc<LlmProvider>> {
        if tenant_id.is_none() {
            let default = self.default_provider.read().unwrap();
            return default
                .clone()
                .ok_or_else(|| Error::Config("No default LLM provider configured".to_string()));
        }

        let tid = tenant_id.unwrap();

        {
            let providers = self.providers.read().unwrap();
            if let Some(provider) = providers.get(tid) {
                return Ok(Arc::clone(provider));
            }
        }

        {
            let configs = self.configs.read().unwrap();
            if let Some(config) = configs.get(tid) {
                if !config.enabled {
                    let default = self.default_provider.read().unwrap();
                    return default.clone().ok_or_else(|| {
                        Error::Config("No default LLM provider configured".to_string())
                    });
                }

                let llm_config = LlmConfig {
                    provider: config.provider.clone(),
                    model: config.model.clone(),
                    api_key: config.api_key.clone(),
                    base_url: config.base_url.clone(),
                    max_tokens: config.max_tokens,
                };

                let provider = create_provider(&llm_config)?;
                let arc_provider = Arc::new(provider);

                let mut providers = self.providers.write().unwrap();
                providers.insert(tid.to_string(), Arc::clone(&arc_provider));
                return Ok(arc_provider);
            }
        }

        let default = self.default_provider.read().unwrap();
        default
            .clone()
            .ok_or_else(|| Error::Config("No default LLM provider configured".to_string()))
    }

    pub fn record_usage(&self, tenant_id: &str, input_tokens: u32, output_tokens: u32) {
        let mut usage_map = self.token_usage.write().unwrap();
        let usage = usage_map
            .entry(tenant_id.to_string())
            .or_default();
        usage
            .input_tokens
            .fetch_add(u64::from(input_tokens), Ordering::Relaxed);
        usage
            .output_tokens
            .fetch_add(u64::from(output_tokens), Ordering::Relaxed);
        usage.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_usage(&self, tenant_id: &str) -> TenantTokenUsage {
        let usage_map = self.token_usage.read().unwrap();
        usage_map
            .get(tenant_id)
            .map(TenantTokenUsage::snapshot)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn list_tenant_configs(&self) -> Vec<(String, TenantLlmConfig)> {
        let configs = self.configs.read().unwrap();
        configs
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl Default for TenantLlmRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ollama_config(model: &str) -> TenantLlmConfig {
        TenantLlmConfig {
            provider: "ollama".to_string(),
            model: model.to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
            enabled: true,
        }
    }

    #[test]
    fn test_new_registry() {
        let registry = TenantLlmRegistry::new();
        assert!(registry.list_tenant_configs().is_empty());
        assert_eq!(registry.get_tenant_config("t1"), None);
    }

    #[test]
    fn test_set_default_provider() {
        let registry = TenantLlmRegistry::new();
        let config = LlmConfig {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
        };
        let provider = create_provider(&config).unwrap();
        registry.set_default_provider(provider);
        let resolved = registry.resolve_provider(None).unwrap();
        assert!(Arc::strong_count(&resolved) > 0);
    }

    #[test]
    fn test_register_tenant_config() {
        let registry = TenantLlmRegistry::new();
        let config = ollama_config("llama3.2");
        registry.register_tenant_config("t1", config.clone());
        let retrieved = registry.get_tenant_config("t1").unwrap();
        assert_eq!(retrieved.provider, "ollama");
        assert_eq!(retrieved.model, "llama3.2");
        assert!(retrieved.enabled);
    }

    #[test]
    fn test_remove_tenant_config() {
        let registry = TenantLlmRegistry::new();
        registry.register_tenant_config("t1", ollama_config("llama3.2"));
        assert!(registry.get_tenant_config("t1").is_some());
        registry.remove_tenant_config("t1");
        assert!(registry.get_tenant_config("t1").is_none());
    }

    #[test]
    fn test_resolve_default_when_no_tenant() {
        let registry = TenantLlmRegistry::new();
        let config = LlmConfig {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
        };
        registry.set_default_provider(create_provider(&config).unwrap());
        let provider = registry.resolve_provider(None).unwrap();
        assert!(Arc::strong_count(&provider) > 0);
    }

    #[test]
    fn test_resolve_default_when_no_config() {
        let registry = TenantLlmRegistry::new();
        let config = LlmConfig {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
        };
        registry.set_default_provider(create_provider(&config).unwrap());
        let provider = registry.resolve_provider(Some("unknown-tenant")).unwrap();
        assert!(Arc::strong_count(&provider) > 0);
    }

    #[test]
    fn test_resolve_tenant_provider() {
        let registry = TenantLlmRegistry::new();
        registry.register_tenant_config("t1", ollama_config("llama3.2"));
        let provider = registry.resolve_provider(Some("t1")).unwrap();
        assert!(Arc::strong_count(&provider) > 0);
    }

    #[test]
    fn test_resolve_caches_provider() {
        let registry = TenantLlmRegistry::new();
        registry.register_tenant_config("t1", ollama_config("llama3.2"));
        let p1 = registry.resolve_provider(Some("t1")).unwrap();
        let p2 = registry.resolve_provider(Some("t1")).unwrap();
        assert!(Arc::ptr_eq(&p1, &p2));
    }

    #[test]
    fn test_record_and_get_usage() {
        let registry = TenantLlmRegistry::new();
        registry.record_usage("t1", 100, 50);
        registry.record_usage("t1", 200, 75);
        let usage = registry.get_usage("t1");
        assert_eq!(usage.input_tokens.load(Ordering::Relaxed), 300);
        assert_eq!(usage.output_tokens.load(Ordering::Relaxed), 125);
        assert_eq!(usage.total_requests.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_resolve_error_when_nothing_configured() {
        let registry = TenantLlmRegistry::new();
        let result = registry.resolve_provider(None);
        assert!(result.is_err());
        let result = registry.resolve_provider(Some("t1"));
        assert!(result.is_err());
    }

    #[test]
    fn test_register_tenant_config_evicts_cache() {
        let registry = TenantLlmRegistry::new();
        registry.register_tenant_config("t1", ollama_config("llama3.2"));
        let p1 = registry.resolve_provider(Some("t1")).unwrap();
        registry.register_tenant_config("t1", ollama_config("mistral"));
        let p2 = registry.resolve_provider(Some("t1")).unwrap();
        assert!(!Arc::ptr_eq(&p1, &p2));
    }
}
