//! Secret Resolution
//!
//! Centralized secret management with a clear priority chain:
//!
//! 1. **Environment variable** (highest priority, recommended for production)
//! 2. **Config file value** (fallback, warns if the field looks like a real secret)
//! 3. **Empty / disabled** (if neither is set)
//!
//! # Environment Variable Convention
//!
//! All secrets follow the `CLAWDIUS_` prefix pattern:
//!
//! | Secret                    | Environment Variable            | Config Field             |
//! |---------------------------|--------------------------------|--------------------------|
//! | JWT HMAC secret           | `CLAWDIUS_JWT_SECRET`         | `messaging.jwt_secret`   |
//! | State store encryption key| `CLAWDIUS_ENCRYPTION_KEY`     | `messaging.state_store.encryption_key` |
//! | Telegram bot token        | `CLAWDIUS_TELEGRAM_BOT_TOKEN` | `platforms.telegram.bot_token` |
//! | Discord bot token         | `CLAWDIUS_DISCORD_BOT_TOKEN`  | `platforms.discord.discord_bot_token` |
//! | Slack bot token           | `CLAWDIUS_SLACK_BOT_TOKEN`    | `platforms.slack.slack_bot_token` |
//! | Matrix access token       | `CLAWDIUS_MATRIX_TOKEN`       | `platforms.matrix.access_token` |
//! | WhatsApp access token     | `CLAWDIUS_WHATSAPP_TOKEN`     | `platforms.whatsapp.whatsapp_access_token` |
//! | Signal API token          | `CLAWDIUS_SIGNAL_TOKEN`       | `platforms.signal.signal_api_url` |
//!
//! # Usage
//!
//! ```ignore
//! let secret = SecretResolver::resolve(
//!     "CLAWDIUS_JWT_SECRET",           // env var name
//!     config.messaging.jwt_secret.as_deref(),  // config file value
//!     true,                              // warn if secret found in config
//! );
//! ```

#![deny(unsafe_code)]

/// A mapping from config field to its corresponding env var name.
pub struct SecretMapping {
    /// Environment variable name (e.g., `CLAWDIUS_JWT_SECRET`).
    pub env_var: &'static str,
    /// Human-readable description for log messages.
    pub description: &'static str,
}

/// All recognized secret fields and their env var mappings.
pub static SECRET_MAPPINGS: &[SecretMapping] = &[
    SecretMapping {
        env_var: "CLAWDIUS_JWT_SECRET",
        description: "JWT HMAC secret",
    },
    SecretMapping {
        env_var: "CLAWDIUS_ENCRYPTION_KEY",
        description: "State store encryption key",
    },
    SecretMapping {
        env_var: "CLAWDIUS_TELEGRAM_BOT_TOKEN",
        description: "Telegram bot token",
    },
    SecretMapping {
        env_var: "CLAWDIUS_DISCORD_BOT_TOKEN",
        description: "Discord bot token",
    },
    SecretMapping {
        env_var: "CLAWDIUS_SLACK_BOT_TOKEN",
        description: "Slack bot token",
    },
    SecretMapping {
        env_var: "CLAWDIUS_MATRIX_TOKEN",
        description: "Matrix access token",
    },
    SecretMapping {
        env_var: "CLAWDIUS_WHATSAPP_TOKEN",
        description: "WhatsApp access token",
    },
    SecretMapping {
        env_var: "CLAWDIUS_SIGNAL_TOKEN",
        description: "Signal API token",
    },
];

/// Resolve a secret value with the standard priority chain:
/// environment variable → config file value → empty.
///
/// # Arguments
///
/// * `env_var` — Name of the environment variable to check.
/// * `config_value` — The value from the config file (if any).
/// * `warn_in_config` — If `true`, emit a warning when the secret is found
///   in the config value rather than the env var (secrets in files are risky).
///
/// # Returns
///
/// The resolved secret string, or empty if neither source has a value.
pub fn resolve(env_var: &str, config_value: Option<&str>, warn_in_config: bool) -> String {
    // Priority 1: Environment variable
    if let Ok(env_val) = std::env::var(env_var) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Priority 2: Config file value
    if let Some(cfg_val) = config_value {
        let trimmed = cfg_val.trim();
        if !trimmed.is_empty() {
            if warn_in_config {
                tracing::warn!(
                    env_var = env_var,
                    "Secret found in config file — set {} via environment variable for production",
                    env_var
                );
            }
            return trimmed.to_string();
        }
    }

    // Priority 3: Not configured
    String::new()
}

/// Check all known secret environment variables and log warnings for any
/// secrets that are only present in the config file (not in env vars).
///
/// This is a startup-time audit — call it once after loading config.
pub fn audit_config_secrets(config: &crate::config::Config) {
    // JWT secret
    if !config.messaging.jwt_secret.is_empty() && std::env::var("CLAWDIUS_JWT_SECRET").is_err() {
        tracing::warn!(
            "JWT secret is set in config file. Set CLAWDIUS_JWT_SECRET env var \
             instead for production deployments."
        );
    }

    // Encryption key
    if !config.messaging.state_store.encryption_key.is_empty()
        && std::env::var("CLAWDIUS_ENCRYPTION_KEY").is_err()
    {
        tracing::warn!(
            "Encryption key is set in config file. Set CLAWDIUS_ENCRYPTION_KEY \
             env var instead for production deployments."
        );
    }

    // Platform tokens
    let checks: &[(&str, &str, Option<&str>)] = &[
        (
            "CLAWDIUS_TELEGRAM_BOT_TOKEN",
            "Telegram bot token",
            config
                .messaging
                .platforms
                .get("telegram")
                .and_then(|p| p.bot_token.as_deref()),
        ),
        (
            "CLAWDIUS_DISCORD_BOT_TOKEN",
            "Discord bot token",
            config
                .messaging
                .platforms
                .get("discord")
                .and_then(|p| p.discord_bot_token.as_deref()),
        ),
        (
            "CLAWDIUS_SLACK_BOT_TOKEN",
            "Slack bot token",
            config
                .messaging
                .platforms
                .get("slack")
                .and_then(|p| p.slack_bot_token.as_deref()),
        ),
        (
            "CLAWDIUS_MATRIX_TOKEN",
            "Matrix access token",
            config
                .messaging
                .platforms
                .get("matrix")
                .and_then(|p| p.access_token.as_deref()),
        ),
        (
            "CLAWDIUS_WHATSAPP_TOKEN",
            "WhatsApp access token",
            config
                .messaging
                .platforms
                .get("whatsapp")
                .and_then(|p| p.whatsapp_access_token.as_deref()),
        ),
    ];

    for (env_var, desc, config_val) in checks {
        if let Some(val) = config_val {
            if !val.trim().is_empty() && std::env::var(env_var).is_err() {
                tracing::warn!(
                    env_var,
                    secret_type = desc,
                    "{desc} is set in config file. Set {env_var} env var instead."
                );
            }
        }
    }
}

/// Mask a secret string for safe logging (shows first 4 chars + `***`).
///
/// # Examples
///
/// ```
/// assert_eq!(clawdius_core::messaging::secret_resolver::mask("sk-1234567890abcdef"), "sk-1***");
/// assert_eq!(clawdius_core::messaging::secret_resolver::mask(""), "***");
/// assert_eq!(clawdius_core::messaging::secret_resolver::mask("abc"), "abc***");
/// ```
#[must_use]
pub fn mask(secret: &str) -> String {
    if secret.len() <= 4 {
        format!("{secret}***")
    } else {
        format!("{}***", &secret[..4])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_env_over_config() {
        std::env::set_var("CLAWDIUS_TEST_SECRET_RESOLVE", "env-value");
        let result = resolve("CLAWDIUS_TEST_SECRET_RESOLVE", Some("config-value"), true);
        assert_eq!(result, "env-value");
        std::env::remove_var("CLAWDIUS_TEST_SECRET_RESOLVE");
    }

    #[test]
    fn resolve_falls_back_to_config() {
        // Ensure env var is NOT set
        std::env::remove_var("CLAWDIUS_TEST_SECRET_RESOLVE_FALLBACK");
        let result = resolve(
            "CLAWDIUS_TEST_SECRET_RESOLVE_FALLBACK",
            Some("config-value"),
            false, // Don't warn in test
        );
        assert_eq!(result, "config-value");
    }

    #[test]
    fn resolve_returns_empty_when_not_set() {
        std::env::remove_var("CLAWDIUS_TEST_SECRET_RESOLVE_EMPTY");
        let result = resolve("CLAWDIUS_TEST_SECRET_RESOLVE_EMPTY", None, false);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_trims_whitespace() {
        std::env::set_var("CLAWDIUS_TEST_SECRET_RESOLVE_TRIM", "  trimmed  ");
        let result = resolve("CLAWDIUS_TEST_SECRET_RESOLVE_TRIM", None, false);
        // resolve() trims whitespace from env var values
        assert_eq!(result, "trimmed");
        std::env::remove_var("CLAWDIUS_TEST_SECRET_RESOLVE_TRIM");
    }

    #[test]
    fn mask_short_secret() {
        assert_eq!(mask("abc"), "abc***");
    }

    #[test]
    fn mask_long_secret() {
        assert_eq!(mask("sk-1234567890abcdef"), "sk-1***");
    }

    #[test]
    fn mask_empty_secret() {
        assert_eq!(mask(""), "***");
    }

    #[test]
    fn secret_mappings_count() {
        assert!(!SECRET_MAPPINGS.is_empty());
        // Verify all env vars have the CLAWDIUS_ prefix
        for mapping in SECRET_MAPPINGS {
            assert!(
                mapping.env_var.starts_with("CLAWDIUS_"),
                "Env var {} should have CLAWDIUS_ prefix",
                mapping.env_var
            );
        }
    }
}
