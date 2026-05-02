use super::{OutputFormat, ConfigAction};

use std::path::{Path, PathBuf};
use anyhow::Context;

pub(super) fn handle_config(
    action: ConfigAction,
    config_path: Option<PathBuf>,
    _output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::config::Config;

    let config_file = config_path.unwrap_or_else(|| {
        std::path::PathBuf::from(".clawdius/config.toml")
    });

    match action {
        ConfigAction::Show => {
            let config = if config_file.exists() {
                Config::load(&config_file)?
            } else {
                Config::default()
            };
            let toml_str = toml::to_string_pretty(&config)?;
            // Mask API keys before displaying
            let masked = mask_api_keys(&toml_str);
            println!("{masked}");
        },

        ConfigAction::Get { key } => {
            let config = if config_file.exists() {
                Config::load(&config_file)?
            } else {
                Config::default()
            };
            let value = get_config_value(&config, &key)?;
            println!("{value}");
        },

        ConfigAction::Set { key, value } => {
            let mut config = if config_file.exists() {
                Config::load(&config_file)?
            } else {
                Config::default()
            };
            set_config_value(&mut config, &key, &value)?;
            // Ensure parent directory exists
            if let Some(parent) = config_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            config.save(&config_file)?;
            println!("✅ Set {} = {}", key, if key.contains("api_key") || key.contains("key") { "***" } else { &value });
            println!("   Config saved to {}", config_file.display());
        },

        ConfigAction::Path => {
            println!("{}", config_file.display());
        },

        ConfigAction::List => {
            println!("Available config keys:");
            println!();
            println!("  llm.default_provider       Default LLM provider (anthropic, openai, openrouter, ollama)");
            println!("  llm.max_tokens            Max tokens per response (default: 4096)");
            println!("  llm.anthropic.model       Anthropic model name");
            println!("  llm.anthropic.api_key      Anthropic API key");
            println!("  llm.anthropic.api_key_env  Env var for Anthropic API key");
            println!("  llm.openai.model          OpenAI model name");
            println!("  llm.openai.api_key         OpenAI API key");
            println!("  llm.openai.api_key_env     Env var for OpenAI API key");
            println!("  llm.ollama.model          Ollama model name");
            println!("  llm.ollama.base_url        Ollama server URL");
            println!("  session.max_context        Max context window messages");
            println!("  session.max_history        Max history messages stored");
            println!("  output.format              Output format (text, json)");
            println!();
            println!("Environment variables (override config):");
            println!("  ANTHROPIC_API_KEY          Anthropic API key");
            println!("  OPENAI_API_KEY             OpenAI API key");
            println!("  OPENROUTER_API_KEY          OpenRouter API key");
            println!("  OPENROUTER_MODEL            OpenRouter model override");
        },
    }

    Ok(())
}

/// Get a config value by dot-separated key path (e.g. "`llm.default_provider`").
fn get_config_value(config: &clawdius_core::config::Config, key: &str) -> anyhow::Result<String> {
    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["llm", "default_provider"] => {
            Ok(config.llm.default_provider.clone().unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "max_tokens"] => Ok(config.llm.max_tokens.to_string()),
        ["llm", "anthropic", "model"] => {
            Ok(config.llm.anthropic.as_ref()
                .and_then(|p| p.model.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "anthropic", "api_key"] => {
            Ok(config.llm.anthropic.as_ref()
                .and_then(|p| p.api_key.clone()).map_or_else(|| "(not set)".to_string(), |_| "***".to_string()))
        },
        ["llm", "anthropic", "api_key_env"] => {
            Ok(config.llm.anthropic.as_ref()
                .and_then(|p| p.api_key_env.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "anthropic", "base_url"] => {
            Ok(config.llm.anthropic.as_ref()
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "openai", "model"] => {
            Ok(config.llm.openai.as_ref()
                .and_then(|p| p.model.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "openai", "api_key"] => {
            Ok(config.llm.openai.as_ref()
                .and_then(|p| p.api_key.clone()).map_or_else(|| "(not set)".to_string(), |_| "***".to_string()))
        },
        ["llm", "openai", "api_key_env"] => {
            Ok(config.llm.openai.as_ref()
                .and_then(|p| p.api_key_env.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "openai", "base_url"] => {
            Ok(config.llm.openai.as_ref()
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "ollama", "model"] => {
            Ok(config.llm.ollama.as_ref()
                .and_then(|p| p.model.clone())
                .unwrap_or_else(|| "(not set)".to_string()))
        },
        ["llm", "ollama", "base_url"] => {
            Ok(config.llm.ollama.as_ref().map_or_else(|| "http://localhost:11434".to_string(), |p| p.base_url.clone()))
        },
        _ => anyhow::bail!("Unknown config key: {key}. Run 'clawdius config list' for available keys."),
    }
}

/// Set a config value by dot-separated key path.
fn set_config_value(config: &mut clawdius_core::config::Config, key: &str, value: &str) -> anyhow::Result<()> {
    use clawdius_core::config::{ProviderConfig, OllamaConfig};

    let parts: Vec<&str> = key.split('.').collect();
    match parts.as_slice() {
        ["llm", "default_provider"] => {
            config.llm.default_provider = Some(value.to_string());
        },
        ["llm", "max_tokens"] => {
            config.llm.max_tokens = value.parse().context("max_tokens must be a number")?;
        },
        ["llm", "anthropic", "model"] => {
            let provider = config.llm.anthropic.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.model = Some(value.to_string());
        },
        ["llm", "anthropic", "api_key"] => {
            let provider = config.llm.anthropic.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.api_key = Some(value.to_string());
        },
        ["llm", "anthropic", "api_key_env"] => {
            let provider = config.llm.anthropic.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.api_key_env = Some(value.to_string());
        },
        ["llm", "anthropic", "base_url"] => {
            let provider = config.llm.anthropic.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.base_url = Some(value.to_string());
        },
        ["llm", "openai", "model"] => {
            let provider = config.llm.openai.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.model = Some(value.to_string());
        },
        ["llm", "openai", "api_key"] => {
            let provider = config.llm.openai.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.api_key = Some(value.to_string());
        },
        ["llm", "openai", "api_key_env"] => {
            let provider = config.llm.openai.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.api_key_env = Some(value.to_string());
        },
        ["llm", "openai", "base_url"] => {
            let provider = config.llm.openai.get_or_insert(ProviderConfig { model: None, api_key_env: None, api_key: None, base_url: None });
            provider.base_url = Some(value.to_string());
        },
        ["llm", "ollama", "model"] => {
            let ollama = config.llm.ollama.get_or_insert_with(|| OllamaConfig {
                model: None,
                base_url: "http://localhost:11434".to_string(),
            });
            ollama.model = Some(value.to_string());
        },
        ["llm", "ollama", "base_url"] => {
            let ollama = config.llm.ollama.get_or_insert_with(|| OllamaConfig {
                model: None,
                base_url: "http://localhost:11434".to_string(),
            });
            ollama.base_url = value.to_string();
        },
        _ => anyhow::bail!("Unknown config key: {key}. Run 'clawdius config list' for available keys."),
    }
    Ok(())
}

/// Mask API key values in TOML output for safe display.
fn mask_api_keys(toml: &str) -> String {
    let mut result = toml.to_string();
    if let Ok(re) = regex::Regex::new(r#"(api_key\s*=\s*)"([^"]{8,}")"#) {
        result = re.replace_all(&result, "${1}***").to_string();
    }
    result
}
