//! User onboarding and first-run experience

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Config, Error, Result};

/// Onboarding status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingStatus {
    /// Onboarding is complete and Clawdius is ready to use
    Complete,
    /// Missing API key for a specific provider
    MissingApiKey {
        /// Provider name
        provider: String,
    },
    /// Missing configuration file
    MissingConfig,
    /// First run - no configuration exists
    FirstRun,
}

/// Onboarding step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingStep {
    /// Welcome screen
    Welcome,
    /// License agreement
    LicenseAgreement,
    /// Choose LLM provider
    ChooseProvider,
    /// Configure API key
    ConfigureApiKey { provider: String },
    /// Choose default settings
    ChooseSettings,
    /// Setup project
    SetupProject,
    /// Complete
    Complete,
}

/// Onboarding progress
#[derive(Debug, Clone)]
pub struct OnboardingProgress {
    /// Current step
    pub current_step: OnboardingStep,
    /// Completed steps
    pub completed_steps: Vec<OnboardingStep>,
    /// Step data (user inputs)
    pub step_data: HashMap<String, String>,
    /// Progress percentage (0-100)
    pub progress: u8,
}

impl Default for OnboardingProgress {
    fn default() -> Self {
        Self {
            current_step: OnboardingStep::Welcome,
            completed_steps: Vec::new(),
            step_data: HashMap::new(),
            progress: 0,
        }
    }
}

impl OnboardingProgress {
    /// Create new progress tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance to next step
    pub fn advance(&mut self, step: OnboardingStep) {
        self.completed_steps.push(self.current_step.clone());
        self.current_step = step;
        self.progress = ((self.completed_steps.len() as f32 / 7.0) * 100.0) as u8;
    }

    /// Set step data
    pub fn set_data(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.step_data.insert(key.into(), value.into());
    }

    /// Get step data
    pub fn get_data(&self, key: &str) -> Option<&String> {
        self.step_data.get(key)
    }

    /// Check if step is completed
    pub fn is_step_completed(&self, step: &OnboardingStep) -> bool {
        self.completed_steps.contains(step)
    }
}

/// Provider option for onboarding
#[derive(Debug, Clone)]
pub struct ProviderOption {
    /// Provider ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Is recommended
    pub recommended: bool,
    /// Features
    pub features: Vec<String>,
}

/// Default settings preset
#[derive(Debug, Clone, Copy)]
pub struct DefaultSettings {
    /// Sandbox tier (0-3)
    pub sandbox_tier: u8,
    /// Enable telemetry
    pub telemetry: bool,
    /// Auto-save sessions
    pub auto_save: bool,
    /// Verbose output
    pub verbose: bool,
}

impl DefaultSettings {
    /// Balanced preset
    pub fn balanced() -> Self {
        Self {
            sandbox_tier: 2,
            telemetry: false,
            auto_save: true,
            verbose: false,
        }
    }

    /// Security-focused preset
    pub fn security() -> Self {
        Self {
            sandbox_tier: 3,
            telemetry: false,
            auto_save: true,
            verbose: false,
        }
    }

    /// Performance preset
    pub fn performance() -> Self {
        Self {
            sandbox_tier: 1,
            telemetry: false,
            auto_save: false,
            verbose: false,
        }
    }

    /// Development preset
    pub fn development() -> Self {
        Self {
            sandbox_tier: 0,
            telemetry: true,
            auto_save: true,
            verbose: true,
        }
    }
}

/// Interactive onboarding wizard
pub struct OnboardingWizard {
    progress: OnboardingProgress,
    config_path: PathBuf,
}

impl OnboardingWizard {
    /// Create a new wizard
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            progress: OnboardingProgress::new(),
            config_path,
        }
    }

    /// Get current progress
    pub fn progress(&self) -> &OnboardingProgress {
        &self.progress
    }

    /// Get mutable progress
    pub fn progress_mut(&mut self) -> &mut OnboardingProgress {
        &mut self.progress
    }

    /// Get welcome message
    pub fn get_welcome_message() -> String {
        r#"
╔══════════════════════════════════════════════════════════════╗
║   ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗          ║
║  ██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝          ║
║  ██║     ██║     ███████║██║   ██║██║  ██║█████╗            ║
║  ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝            ║
║  ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗          ║
║   ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝          ║
║                                                              ║
║   High-Assurance AI Coding Assistant                        ║
║   Built with Rust • Sandboxed • Formally Verified           ║
║                                                              ║
╚══════════════════════════════════════════════════════════════╝

Welcome to Clawdius! This wizard will help you get started.

Clawdius is a high-assurance AI coding assistant that:
  • Runs 100% locally with your code
  • Provides 4-tier sandboxing for security
  • Supports 11+ LLM providers
  • Includes formal verification with Lean4
"#
        .to_string()
    }

    /// Get provider options
    pub fn get_provider_options() -> Vec<ProviderOption> {
        vec![
            ProviderOption {
                id: "anthropic".to_string(),
                name: "Anthropic Claude".to_string(),
                description: "Claude 3.5 Sonnet, Opus, Haiku".to_string(),
                recommended: true,
                features: vec![
                    "Best code generation".to_string(),
                    "Long context".to_string(),
                ],
            },
            ProviderOption {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                description: "GPT-4o, GPT-4 Turbo".to_string(),
                recommended: false,
                features: vec!["Widely used".to_string(), "Fast responses".to_string()],
            },
            ProviderOption {
                id: "zai".to_string(),
                name: "Zhipu AI".to_string(),
                description: "GLM-4, ChatGLM".to_string(),
                recommended: false,
                features: vec![
                    "Chinese optimized".to_string(),
                    "Cost effective".to_string(),
                ],
            },
            ProviderOption {
                id: "ollama".to_string(),
                name: "Ollama (Local)".to_string(),
                description: "Run models locally".to_string(),
                recommended: false,
                features: vec!["100% private".to_string(), "No API costs".to_string()],
            },
        ]
    }

    /// Get settings presets
    pub fn get_settings_presets() -> Vec<(&'static str, DefaultSettings)> {
        vec![
            ("Balanced", DefaultSettings::balanced()),
            ("Security", DefaultSettings::security()),
            ("Performance", DefaultSettings::performance()),
            ("Development", DefaultSettings::development()),
        ]
    }
}

/// Onboarding manager
#[derive(Debug)]
pub struct Onboarding {
    /// Path to the config file
    config_path: PathBuf,
    /// List of API keys needed
    api_keys_needed: Vec<String>,
}

impl Onboarding {
    /// Create a new onboarding instance
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            api_keys_needed: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "zai".to_string(),
            ],
        }
    }

    /// Create onboarding instance with default config path
    pub fn with_default_path() -> Self {
        Self::new(Config::default_path())
    }

    /// Check the current environment and return onboarding status
    pub fn check_environment() -> OnboardingStatus {
        let onboarding = Self::with_default_path();
        onboarding.check()
    }

    /// Check the onboarding status
    pub fn check(&self) -> OnboardingStatus {
        if !self.config_path.exists() {
            return OnboardingStatus::FirstRun;
        }

        match Config::load(&self.config_path) {
            Ok(config) => {
                if let Some(ref default_provider) = config.llm.default_provider {
                    if !self.has_api_key(default_provider, &config) {
                        return OnboardingStatus::MissingApiKey {
                            provider: default_provider.clone(),
                        };
                    }
                } else {
                    for provider in &self.api_keys_needed {
                        if self.has_api_key(provider, &config) {
                            return OnboardingStatus::Complete;
                        }
                    }

                    if !self.api_keys_needed.is_empty() {
                        return OnboardingStatus::MissingApiKey {
                            provider: self.api_keys_needed[0].clone(),
                        };
                    }
                }

                OnboardingStatus::Complete
            }
            Err(_) => OnboardingStatus::MissingConfig,
        }
    }

    /// Check if an API key is available for a provider
    fn has_api_key(&self, provider: &str, config: &Config) -> bool {
        #[cfg(feature = "keyring")]
        {
            if let Ok(Some(_)) = crate::config::KeyringStorage::global().get_api_key(provider) {
                return true;
            }
        }

        let env_var = format!("{}_API_KEY", provider.to_uppercase());
        if std::env::var(&env_var).is_ok() {
            return true;
        }

        let provider_config = match provider {
            "anthropic" => &config.llm.anthropic,
            "openai" => &config.llm.openai,
            "zai" => &config.llm.zai,
            _ => return false,
        };

        match provider_config {
            Some(cfg) => {
                if let Some(ref key) = cfg.api_key {
                    return !key.is_empty();
                }
                if let Some(ref env) = cfg.api_key_env {
                    return std::env::var(env).is_ok();
                }
                false
            }
            None => false,
        }
    }

    /// Create the .clawdius directory structure
    pub fn create_directory_structure(&self, project_path: &Path) -> Result<()> {
        let clawdius_dir = project_path.join(".clawdius");
        fs::create_dir_all(&clawdius_dir)?;

        fs::create_dir_all(clawdius_dir.join("sops"))?;
        fs::create_dir_all(clawdius_dir.join("specs"))?;
        fs::create_dir_all(clawdius_dir.join("graph"))?;
        fs::create_dir_all(clawdius_dir.join("commands"))?;

        Ok(())
    }

    /// Create the default configuration file
    pub fn create_config(&self, provider: &str, api_key: Option<&str>) -> Result<()> {
        let mut config = Config::default();

        config.llm.default_provider = Some(provider.to_string());

        if let Some(key) = api_key {
            #[cfg(feature = "keyring")]
            {
                crate::config::KeyringStorage::global().set_api_key(provider, key)?;
            }

            #[cfg(not(feature = "keyring"))]
            {
                let provider_config = match provider {
                    "anthropic" => &mut config.llm.anthropic,
                    "openai" => &mut config.llm.openai,
                    "zai" => &mut config.llm.zai,
                    _ => return Err(Error::Config(format!("Unknown provider: {}", provider))),
                };

                *provider_config = Some(crate::config::ProviderConfig {
                    model: None,
                    api_key_env: None,
                    api_key: Some(key.to_string()),
                    base_url: None,
                });
            }
        }

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        config.save(&self.config_path)?;

        Ok(())
    }

    /// Create a default configuration without API keys
    pub fn create_default_config(&self) -> Result<()> {
        let config = Config::default();

        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        config.save(&self.config_path)?;

        Ok(())
    }

    /// Get the config path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Set the list of API keys needed
    pub fn set_api_keys_needed(&mut self, providers: Vec<String>) {
        self.api_keys_needed = providers;
    }
}

/// First-run welcome message
pub fn print_welcome_message() {
    println!("{}", OnboardingWizard::get_welcome_message());
}

/// Print onboarding status message
pub fn print_onboarding_status(status: &OnboardingStatus) {
    match status {
        OnboardingStatus::Complete => {
            println!("✓ Clawdius is configured and ready!");
        }
        OnboardingStatus::MissingApiKey { provider } => {
            println!("⚠ Missing API key for {}", provider);
            println!("  Run: clawdius auth set-key {}", provider);
            println!();
            println!(
                "  Or set the environment variable: {}_API_KEY",
                provider.to_uppercase()
            );
        }
        OnboardingStatus::MissingConfig => {
            println!("⚠ Missing configuration file");
            println!("  Run: clawdius init");
        }
        OnboardingStatus::FirstRun => {
            println!("✨ First run detected!");
            println!("  Run: clawdius init");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_onboarding_first_run() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".clawdius/config.toml");

        let onboarding = Onboarding::new(config_path);
        assert_eq!(onboarding.check(), OnboardingStatus::FirstRun);
    }

    #[test]
    fn test_create_directory_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".clawdius/config.toml");

        let onboarding = Onboarding::new(config_path);
        onboarding
            .create_directory_structure(temp_dir.path())
            .unwrap();

        assert!(temp_dir.path().join(".clawdius").exists());
        assert!(temp_dir.path().join(".clawdius/sops").exists());
        assert!(temp_dir.path().join(".clawdius/specs").exists());
        assert!(temp_dir.path().join(".clawdius/graph").exists());
        assert!(temp_dir.path().join(".clawdius/commands").exists());
    }

    #[test]
    fn test_create_default_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".clawdius/config.toml");

        let onboarding = Onboarding::new(config_path.clone());
        onboarding.create_default_config().unwrap();

        assert!(config_path.exists());

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.project.name, "clawdius");
    }

    #[test]
    fn test_onboarding_progress() {
        let mut progress = OnboardingProgress::new();
        assert_eq!(progress.current_step, OnboardingStep::Welcome);

        progress.advance(OnboardingStep::ChooseProvider);
        assert_eq!(progress.current_step, OnboardingStep::ChooseProvider);
        assert!(progress.is_step_completed(&OnboardingStep::Welcome));
    }

    #[test]
    fn test_provider_options() {
        let options = OnboardingWizard::get_provider_options();
        assert!(!options.is_empty());
        assert!(options.iter().any(|o| o.recommended));
    }

    #[test]
    fn test_settings_presets() {
        let balanced = DefaultSettings::balanced();
        let security = DefaultSettings::security();
        assert!(security.sandbox_tier > balanced.sandbox_tier);
    }
}
