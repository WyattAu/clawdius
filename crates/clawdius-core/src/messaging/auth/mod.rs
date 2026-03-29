use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::key_rotation::{hash_api_key, ApiKeyStore};
use super::types::Platform;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult {
    Authenticated { platform: Platform, user_id: String },
    InvalidKey,
    MissingKey,
}

#[derive(Debug, Clone)]
pub struct ApiAuthenticator {
    keys: HashMap<Platform, HashSet<String>>,
    global_keys: HashSet<String>,
    pub key_store: Option<Arc<ApiKeyStore>>,
}

impl Default for ApiAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiAuthenticator {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            global_keys: HashSet::new(),
            key_store: None,
        }
    }

    pub fn with_key_store(mut self, store: Arc<ApiKeyStore>) -> Self {
        self.key_store = Some(store);
        self
    }

    pub fn add_platform_key(&mut self, platform: Platform, key: String) {
        self.keys.entry(platform).or_default().insert(key);
    }

    pub fn add_global_key(&mut self, key: String) {
        self.global_keys.insert(key);
    }

    pub fn remove_key(&mut self, platform: Option<Platform>, key: &str) -> bool {
        if let Some(p) = platform {
            if let Some(set) = self.keys.get_mut(&p) {
                return set.remove(key);
            }
            false
        } else {
            self.global_keys.remove(key)
        }
    }

    pub fn validate(&self, platform: Platform, api_key: Option<&str>) -> AuthResult {
        let key = match api_key {
            Some(k) if !k.is_empty() => k,
            _ => return AuthResult::MissingKey,
        };

        if self.global_keys.contains(key) {
            return AuthResult::Authenticated {
                platform,
                user_id: key.to_string(),
            };
        }

        if let Some(platform_keys) = self.keys.get(&platform) {
            if platform_keys.contains(key) {
                return AuthResult::Authenticated {
                    platform,
                    user_id: key.to_string(),
                };
            }
        }

        AuthResult::InvalidKey
    }

    pub async fn validate_with_store(
        &self,
        platform: Platform,
        api_key: Option<&str>,
    ) -> AuthResult {
        let result = self.validate(platform, api_key);
        match result {
            AuthResult::Authenticated { .. } => result,
            AuthResult::InvalidKey | AuthResult::MissingKey => {
                if let Some(key_str) = api_key {
                    if key_str.is_empty() {
                        return result;
                    }
                    if let Some(store) = &self.key_store {
                        let key_hash = hash_api_key(key_str);
                        if store.validate_key(&key_hash).await.is_some() {
                            return AuthResult::Authenticated {
                                platform,
                                user_id: key_str.to_string(),
                            };
                        }
                    }
                }
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let auth = ApiAuthenticator::new();
        assert!(auth.keys.is_empty());
        assert!(auth.global_keys.is_empty());
    }

    #[test]
    fn default_is_empty() {
        let auth = ApiAuthenticator::default();
        assert!(auth.keys.is_empty());
        assert!(auth.global_keys.is_empty());
    }

    #[test]
    fn add_and_validate_platform_key() {
        let mut auth = ApiAuthenticator::new();
        auth.add_platform_key(Platform::Telegram, "tg_key_1".into());
        assert_eq!(
            auth.validate(Platform::Telegram, Some("tg_key_1")),
            AuthResult::Authenticated {
                platform: Platform::Telegram,
                user_id: "tg_key_1".to_string(),
            }
        );
    }

    #[test]
    fn platform_key_not_valid_for_other_platform() {
        let mut auth = ApiAuthenticator::new();
        auth.add_platform_key(Platform::Telegram, "tg_key_1".into());
        assert_eq!(
            auth.validate(Platform::Discord, Some("tg_key_1")),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn global_key_valid_for_all_platforms() {
        let mut auth = ApiAuthenticator::new();
        auth.add_global_key("master_key".into());
        assert_eq!(
            auth.validate(Platform::Telegram, Some("master_key")),
            AuthResult::Authenticated {
                platform: Platform::Telegram,
                user_id: "master_key".to_string(),
            }
        );
        assert_eq!(
            auth.validate(Platform::Discord, Some("master_key")),
            AuthResult::Authenticated {
                platform: Platform::Discord,
                user_id: "master_key".to_string(),
            }
        );
    }

    #[test]
    fn missing_key_returns_missing_key() {
        let auth = ApiAuthenticator::new();
        assert_eq!(
            auth.validate(Platform::Telegram, None),
            AuthResult::MissingKey
        );
    }

    #[test]
    fn empty_key_returns_missing_key() {
        let auth = ApiAuthenticator::new();
        assert_eq!(
            auth.validate(Platform::Telegram, Some("")),
            AuthResult::MissingKey
        );
    }

    #[test]
    fn unknown_key_returns_invalid() {
        let auth = ApiAuthenticator::new();
        assert_eq!(
            auth.validate(Platform::Telegram, Some("nope")),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn remove_platform_key() {
        let mut auth = ApiAuthenticator::new();
        auth.add_platform_key(Platform::Telegram, "tg_key".into());
        assert_eq!(
            auth.validate(Platform::Telegram, Some("tg_key")),
            AuthResult::Authenticated {
                platform: Platform::Telegram,
                user_id: "tg_key".into()
            }
        );
        assert!(auth.remove_key(Some(Platform::Telegram), "tg_key"));
        assert_eq!(
            auth.validate(Platform::Telegram, Some("tg_key")),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn remove_global_key() {
        let mut auth = ApiAuthenticator::new();
        auth.add_global_key("gk".into());
        assert!(auth.remove_key(None, "gk"));
        assert_eq!(
            auth.validate(Platform::Telegram, Some("gk")),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn remove_nonexistent_key_returns_false() {
        let mut auth = ApiAuthenticator::new();
        assert!(!auth.remove_key(Some(Platform::Telegram), "nope"));
        assert!(!auth.remove_key(None, "nope"));
    }

    #[test]
    fn multiple_keys_per_platform() {
        let mut auth = ApiAuthenticator::new();
        auth.add_platform_key(Platform::Slack, "key_a".into());
        auth.add_platform_key(Platform::Slack, "key_b".into());
        assert_eq!(
            auth.validate(Platform::Slack, Some("key_a")),
            AuthResult::Authenticated {
                platform: Platform::Slack,
                user_id: "key_a".into()
            }
        );
        assert_eq!(
            auth.validate(Platform::Slack, Some("key_b")),
            AuthResult::Authenticated {
                platform: Platform::Slack,
                user_id: "key_b".into()
            }
        );
        assert_eq!(
            auth.validate(Platform::Slack, Some("key_c")),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn platform_key_preferred_over_global() {
        let mut auth = ApiAuthenticator::new();
        auth.add_global_key("shared".into());
        auth.add_platform_key(Platform::Discord, "shared".into());
        assert!(matches!(
            auth.validate(Platform::Discord, Some("shared")),
            AuthResult::Authenticated { .. }
        ));
    }
}
