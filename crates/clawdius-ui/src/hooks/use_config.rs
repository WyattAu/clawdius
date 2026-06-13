//! Hook for application configuration.
//!
//! Manages provider/model selection, theme toggle, and settings persistence.

use leptos::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub name: String,
    pub models: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ConfigState {
    pub current_provider: String,
    pub current_model: String,
    pub available_providers: Vec<ProviderConfig>,
    pub theme: ThemeMode,
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

pub struct ConfigActions {
    pub set_provider: Callback<String>,
    pub set_model: Callback<String>,
    pub toggle_theme: Callback<()>,
    pub set_endpoint: Callback<String>,
}

pub fn use_config() -> (RwSignal<ConfigState>, ConfigActions) {
    let state = RwSignal::new(ConfigState::default());

    let set_provider = Callback::new(move |provider: String| {
        state.update(|s| {
            s.current_provider = provider.clone();
            if let Some(p) = s.available_providers.iter().find(|p| p.name == provider) {
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

    let toggle_theme = Callback::new(move |_: ()| {
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
