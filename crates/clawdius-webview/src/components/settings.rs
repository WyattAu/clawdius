use super::common::{Button, ButtonVariant, Toast, ToastMessage, ToastType};
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WebviewConfig {
    pub providers: Vec<ProviderConfig>,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub name: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub enabled: bool,
    pub base_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub custom_colors: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
    Custom,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct KeybindingConfig {
    pub send_message: String,
    pub new_session: String,
    pub open_settings: String,
    pub toggle_sidebar: String,
}

impl Default for WebviewConfig {
    fn default() -> Self {
        Self {
            providers: vec![
                ProviderConfig {
                    name: "anthropic".to_string(),
                    api_key: None,
                    model: Some("claude-3-5-sonnet-20241022".to_string()),
                    enabled: true,
                    base_url: None,
                },
                ProviderConfig {
                    name: "openai".to_string(),
                    api_key: None,
                    model: Some("gpt-4".to_string()),
                    enabled: false,
                    base_url: None,
                },
            ],
            theme: ThemeConfig {
                mode: ThemeMode::Dark,
                custom_colors: None,
            },
            keybindings: KeybindingConfig {
                send_message: "Enter".to_string(),
                new_session: "Ctrl+N".to_string(),
                open_settings: "Ctrl+,".to_string(),
                toggle_sidebar: "Ctrl+B".to_string(),
            },
        }
    }
}

#[component]
pub fn SettingsView() -> impl IntoView {
    let (config, set_config) = create_signal(WebviewConfig::default());
    let (active_provider, set_active_provider) = create_signal(0);
    let (toasts, set_toasts) = create_signal::<Vec<ToastMessage>>(Vec::new());
    let (show_import_modal, set_show_import_modal) = create_signal(false);
    let (import_data, set_import_data) = create_signal(String::new());
    let (has_changes, set_has_changes) = create_signal(false);

    let send_to_vscode = |msg_type: &str, data: serde_json::Value| {
        if let Some(window) = web_sys::window() {
            if let Ok(vscode) = js_sys::Reflect::get(
                &window,
                &wasm_bindgen::JsValue::from_str("acquireVsCodeApi"),
            ) {
                let vscode = js_sys::Function::from(vscode)
                    .call0(&wasm_bindgen::JsValue::NULL)
                    .unwrap();
                let post_message =
                    js_sys::Reflect::get(&vscode, &wasm_bindgen::JsValue::from_str("postMessage"))
                        .unwrap();
                let post_message = js_sys::Function::from(post_message);
                let msg = serde_json::json!({
                    "type": msg_type,
                    "data": data
                });
                let msg_js = wasm_bindgen::JsValue::from_str(&serde_json::to_string(&msg).unwrap());
                post_message.call1(&vscode, &msg_js).unwrap();
            }
        }
    };

    create_effect(move |_| {
        send_to_vscode("getSettings", serde_json::json!({}));
    });

    create_effect(move |_| {
        if let Some(window) = web_sys::window() {
            let callback = wasm_bindgen::closure::Closure::wrap(Box::new(
                move |event: web_sys::MessageEvent| {
                    let data = event.data();
                    if data.is_string() {
                        if let Some(data_str) = data.as_string() {
                            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&data_str) {
                                if let Some(msg_type) = msg.get("type").and_then(|t| t.as_str()) {
                                    match msg_type {
                                        "settingsData" => {
                                            if let Some(settings_data) = msg.get("data") {
                                                if let Ok(settings) =
                                                    serde_json::from_value::<WebviewConfig>(
                                                        settings_data.clone(),
                                                    )
                                                {
                                                    set_config.set(settings);
                                                }
                                            }
                                        }
                                        "settingsSaved" => {
                                            set_toasts.update(|t| {
                                                t.push(ToastMessage {
                                                    id: uuid::Uuid::new_v4().to_string(),
                                                    message: "Settings saved successfully"
                                                        .to_string(),
                                                    toast_type: ToastType::Success,
                                                });
                                            });
                                            set_has_changes.set(false);
                                        }
                                        "settingsImported" => {
                                            set_toasts.update(|t| {
                                                t.push(ToastMessage {
                                                    id: uuid::Uuid::new_v4().to_string(),
                                                    message: "Settings imported successfully"
                                                        .to_string(),
                                                    toast_type: ToastType::Success,
                                                });
                                            });
                                            set_show_import_modal.set(false);
                                            set_import_data.set(String::new());
                                        }
                                        "settingsReset" => {
                                            set_config.set(WebviewConfig::default());
                                            set_toasts.update(|t| {
                                                t.push(ToastMessage {
                                                    id: uuid::Uuid::new_v4().to_string(),
                                                    message: "Settings reset to defaults"
                                                        .to_string(),
                                                    toast_type: ToastType::Info,
                                                });
                                            });
                                        }
                                        "error" => {
                                            if let Some(error_msg) =
                                                msg.get("data").and_then(|d| d.get("message"))
                                            {
                                                if let Some(msg_text) = error_msg.as_str() {
                                                    set_toasts.update(|t| {
                                                        t.push(ToastMessage {
                                                            id: uuid::Uuid::new_v4().to_string(),
                                                            message: msg_text.to_string(),
                                                            toast_type: ToastType::Error,
                                                        });
                                                    });
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                },
            ) as Box<dyn Fn(_)>);

            let _ = window
                .add_event_listener_with_callback("message", callback.as_ref().unchecked_ref());
            #[allow(unused_must_use)]
            callback.forget();
        }
    });

    let save_settings = move |_| {
        send_to_vscode("saveSettings", serde_json::to_value(config.get()).unwrap());
    };

    let reset_settings = move |_| {
        send_to_vscode("resetSettings", serde_json::json!({}));
    };

    let export_settings = move |_| {
        let json = serde_json::to_string_pretty(&config.get()).unwrap();
        if let Some(window) = web_sys::window() {
            let _ = window.navigator().clipboard().write_text(&json);
            set_toasts.update(|t| {
                t.push(ToastMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    message: "Settings copied to clipboard".to_string(),
                    toast_type: ToastType::Success,
                });
            });
        }
    };

    let import_settings = move |_| {
        if let Ok(imported) = serde_json::from_str::<WebviewConfig>(&import_data.get()) {
            set_config.set(imported);
            send_to_vscode(
                "importSettings",
                serde_json::to_value(config.get()).unwrap(),
            );
        } else {
            set_toasts.update(|t| {
                t.push(ToastMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    message: "Invalid settings format".to_string(),
                    toast_type: ToastType::Error,
                });
            });
        }
    };

    let update_provider_api_key = move |index: usize, key: String| {
        set_config.update(|cfg| {
            if let Some(provider) = cfg.providers.get_mut(index) {
                provider.api_key = Some(key);
            }
        });
        set_has_changes.set(true);
    };

    let update_provider_model = move |index: usize, model: String| {
        set_config.update(|cfg| {
            if let Some(provider) = cfg.providers.get_mut(index) {
                provider.model = Some(model);
            }
        });
        set_has_changes.set(true);
    };

    let update_provider_enabled = move |index: usize, enabled: bool| {
        set_config.update(|cfg| {
            if let Some(provider) = cfg.providers.get_mut(index) {
                provider.enabled = enabled;
            }
        });
        set_has_changes.set(true);
    };

    let update_theme_mode = move |mode: ThemeMode| {
        set_config.update(|cfg| {
            cfg.theme.mode = mode;
        });
        set_has_changes.set(true);
    };

    let provider_models: Vec<(String, String)> = match active_provider.get() {
        0 => vec![
            (
                "claude-3-5-sonnet-20241022".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
            ),
            (
                "claude-3-5-haiku-20241022".to_string(),
                "claude-3-5-haiku-20241022".to_string(),
            ),
            (
                "claude-3-opus-20240229".to_string(),
                "claude-3-opus-20240229".to_string(),
            ),
        ],
        1 => vec![
            ("gpt-4".to_string(), "gpt-4".to_string()),
            ("gpt-4-turbo".to_string(), "gpt-4-turbo".to_string()),
            ("gpt-3.5-turbo".to_string(), "gpt-3.5-turbo".to_string()),
        ],
        _ => vec![],
    };

    view! {
        <div class="settings-view">
            <Toast toasts=toasts set_toasts=set_toasts/>
            <div class="settings-header">
                <h2>"Settings"</h2>
                <div class="settings-actions">
                    <Button
                        variant=ButtonVariant::Secondary
                        on_click=export_settings
                    >
                        "Export"
                    </Button>
                    <Button
                        variant=ButtonVariant::Secondary
                        on_click=move |_| set_show_import_modal.set(true)
                    >
                        "Import"
                    </Button>
                    <Button
                        variant=ButtonVariant::Secondary
                        on_click=reset_settings
                    >
                        "Reset"
                    </Button>
                    <Button
                        variant=ButtonVariant::Primary
                        disabled=!has_changes.get()
                        on_click=save_settings
                    >
                        "Save"
                    </Button>
                </div>
            </div>

            <div class="settings-content">
                <div class="settings-section">
                    <h3>"Providers"</h3>
                    <div class="provider-tabs">
                        {move || {
                            config.get().providers.iter().enumerate().map(|(index, provider)| {
                                let index2 = index;
                                let provider_name = provider.name.clone();
                                let is_active = active_provider.get() == index;
                                view! {
                                    <button
                                        class=format!("provider-tab {}", if is_active { "active" } else { "" })
                                        on:click=move |_| set_active_provider.set(index2)
                                    >
                                        {provider_name}
                                    </button>
                                }
                            }).collect::<Vec<_>>()
                        }}
                    </div>

                    {move || {
                        let current_provider = config.get().providers.get(active_provider.get()).cloned();
                        if let Some(provider) = current_provider {
                            let provider_index = active_provider.get();
                            let api_key_value = provider.api_key.clone().unwrap_or_default();
                            let model_value = provider.model.clone().unwrap_or_default();
                            let enabled_value = provider.enabled;
                            let models = provider_models.clone();

                            view! {
                                <div class="provider-settings">
                                    <div class="setting-row">
                                        <label>"Enabled"</label>
                                        <input
                                            type="checkbox"
                                            checked=enabled_value
                                            on:change=move |ev| {
                                                let checked = event_target_checked(&ev);
                                                update_provider_enabled(provider_index, checked);
                                            }
                                        />
                                    </div>
                                    <div class="setting-row">
                                        <label>"API Key"</label>
                                        <input
                                            type="password"
                                            class="setting-input"
                                            placeholder="Enter API key..."
                                            prop:value=api_key_value
                                            on:input=move |ev| {
                                                let value = event_target_value(&ev);
                                                update_provider_api_key(provider_index, value);
                                            }
                                        />
                                    </div>
                                    <div class="setting-row">
                                        <label>"Model"</label>
                                        <select
                                            class="setting-select"
                                            on:change=move |ev| {
                                                let value = event_target_value(&ev);
                                                update_provider_model(provider_index, value);
                                            }
                                        >
                                            {models.iter().map(|(label, value)| {
                                                let selected = value == &model_value;
                                                view! {
                                                    <option value=value selected=selected>
                                                        {label.clone()}
                                                    </option>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </select>
                                    </div>
                                    {provider.base_url.as_ref().map(|url| view! {
                                        <div class="setting-row">
                                            <label>"Base URL"</label>
                                            <input
                                                type="text"
                                                class="setting-input"
                                                readonly=true
                                                prop:value=url.clone()
                                            />
                                        </div>
                                    })}
                                </div>
                            }.into_view()
                        } else {
                            view! { <div/> }.into_view()
                        }
                    }}
                </div>

                <div class="settings-section">
                    <h3>"Theme"</h3>
                    <div class="theme-options">
                        <button
                            class=format!("theme-option {}", if config.get().theme.mode == ThemeMode::Dark { "active" } else { "" })
                            on:click=move |_| update_theme_mode(ThemeMode::Dark)
                        >
                            "Dark"
                        </button>
                        <button
                            class=format!("theme-option {}", if config.get().theme.mode == ThemeMode::Light { "active" } else { "" })
                            on:click=move |_| update_theme_mode(ThemeMode::Light)
                        >
                            "Light"
                        </button>
                        <button
                            class=format!("theme-option {}", if config.get().theme.mode == ThemeMode::Custom { "active" } else { "" })
                            on:click=move |_| update_theme_mode(ThemeMode::Custom)
                        >
                            "Custom"
                        </button>
                    </div>
                </div>

                <div class="settings-section">
                    <h3>"Keybindings"</h3>
                    <div class="keybinding-settings">
                        <div class="setting-row">
                            <label>"Send Message"</label>
                            <input
                                type="text"
                                class="setting-input keybinding-input"
                                readonly=true
                                prop:value=config.get().keybindings.send_message
                            />
                        </div>
                        <div class="setting-row">
                            <label>"New Session"</label>
                            <input
                                type="text"
                                class="setting-input keybinding-input"
                                readonly=true
                                prop:value=config.get().keybindings.new_session
                            />
                        </div>
                        <div class="setting-row">
                            <label>"Open Settings"</label>
                            <input
                                type="text"
                                class="setting-input keybinding-input"
                                readonly=true
                                prop:value=config.get().keybindings.open_settings
                            />
                        </div>
                        <div class="setting-row">
                            <label>"Toggle Sidebar"</label>
                            <input
                                type="text"
                                class="setting-input keybinding-input"
                                readonly=true
                                prop:value=config.get().keybindings.toggle_sidebar
                            />
                        </div>
                    </div>
                </div>
            </div>

            <super::common::Modal
                title="Import Settings"
                show=show_import_modal.get()
                on_close=move || {
                    set_show_import_modal.set(false);
                    set_import_data.set(String::new());
                }
            >
                <div class="import-modal-content">
                    <p>"Paste your settings JSON below:"</p>
                    <textarea
                        class="import-textarea"
                        placeholder="Paste JSON here..."
                        prop:value=import_data.get()
                        on:input=move |ev| {
                            set_import_data.set(event_target_value(&ev));
                        }
                    />
                    <div class="modal-actions">
                        <Button
                            variant=ButtonVariant::Secondary
                            on_click=move |_| {
                                set_show_import_modal.set(false);
                                set_import_data.set(String::new());
                            }
                        >
                            "Cancel"
                        </Button>
                        <Button
                            variant=ButtonVariant::Primary
                            on_click=import_settings
                        >
                            "Import"
                        </Button>
                    </div>
                </div>
            </super::common::Modal>
        </div>
    }
}

fn event_target_value(ev: &leptos::ev::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .unwrap()
        .unchecked_into::<web_sys::HtmlInputElement>()
        .value()
}

fn event_target_checked(ev: &leptos::ev::Event) -> bool {
    use wasm_bindgen::JsCast;
    ev.target()
        .unwrap()
        .unchecked_into::<web_sys::HtmlInputElement>()
        .checked()
}
