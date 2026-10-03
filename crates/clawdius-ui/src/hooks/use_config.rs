//! Hook for application configuration.
//!
//! Manages provider/model selection, theme toggle, and settings persistence.

use leptos::prelude::*;

/// UI color scheme selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    /// Dark theme.
    Dark,
    /// Light theme.
    Light,
}

/// One LLM provider and the models it offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Provider display name.
    pub name: String,
    /// Model identifiers available from this provider.
    pub models: Vec<String>,
}

/// Reactive application configuration.
#[derive(Clone, Debug)]
pub struct ConfigState {
    /// Currently selected provider name.
    pub current_provider: String,
    /// Currently selected model name.
    pub current_model: String,
    /// Providers and models offered in the picker.
    pub available_providers: Vec<ProviderConfig>,
    /// Active UI color scheme.
    pub theme: ThemeMode,
    /// Base URL of the clawdius backend.
    pub api_endpoint: String,
}

impl Default for ConfigState {
    fn default() -> Self {
        Self {
            current_provider: String::from("openai"),
            current_model: String::from("gpt-4o"),
            available_providers: vec![
                ProviderConfig {
                    name: String::from("openai"),
                    models: vec![
                        String::from("gpt-4o"),
                        String::from("gpt-4o-mini"),
                        String::from("o1"),
                        String::from("o1-mini"),
                    ],
                },
                ProviderConfig {
                    name: String::from("anthropic"),
                    models: vec![
                        String::from("claude-sonnet-4-20250514"),
                        String::from("claude-3.5-haiku-20241022"),
                    ],
                },
                ProviderConfig {
                    name: String::from("google"),
                    models: vec![
                        String::from("gemini-2.5-pro"),
                        String::from("gemini-2.5-flash"),
                    ],
                },
            ],
            theme: ThemeMode::Dark,
            api_endpoint: String::from("http://localhost:3000"),
        }
    }
}

/// Callbacks for mutating the configuration state.
#[derive(Clone)]
pub struct ConfigActions {
    /// Selects a provider, resetting the model if it is unsupported there.
    pub set_provider: Callback<String>,
    /// Selects a model within the current provider.
    pub set_model: Callback<String>,
    /// Toggles between dark and light themes.
    pub toggle_theme: Callback<()>,
    /// Overrides the backend base URL.
    pub set_endpoint: Callback<String>,
}

/// Creates the reactive configuration state and its action callbacks.
#[must_use]
pub fn use_config() -> (RwSignal<ConfigState>, ConfigActions) {
    let state = RwSignal::new(ConfigState::default());

    let set_provider = Callback::new(move |provider: String| {
        state.update(|s| {
            s.current_provider = provider;
            if let Some(p) = s
                .available_providers
                .iter()
                .find(|p| p.name == s.current_provider)
            {
                if !p.models.contains(&s.current_model) {
                    if let Some(first) = p.models.first() {
                        s.current_model = first.clone();
                    }
                }
            }
        });
    });

    let set_model = Callback::new(move |model: String| {
        state.update(|s| {
            s.current_model = model;
        });
    });

    let toggle_theme = Callback::new(move |()| {
        state.update(|s| {
            s.theme = match s.theme {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
        });
    });

    let set_endpoint = Callback::new(move |endpoint: String| {
        state.update(|s| {
            s.api_endpoint = endpoint;
        });
    });

    (
        state,
        ConfigActions {
            set_provider,
            set_model,
            toggle_theme,
            set_endpoint,
        },
    )
}
