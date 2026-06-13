use leptos::prelude::*;
use leptos::{component, view, IntoView};

use clawdius_ui::hooks::use_config::{ConfigActions, ConfigState, ThemeMode};
use clawdius_ui::theme::colors;
use clawdius_ui::theme::spacing;

/// Settings page for provider, model, and theme configuration.
#[component]
pub fn SettingsPage(
    config_state: RwSignal<ConfigState>,
    config_actions: ConfigActions,
) -> impl IntoView {
    let providers = Memo::new(move |_| config_state.get().available_providers.clone());

    let models = Memo::new(move |_| {
        let cs = config_state.get();
        cs.available_providers
            .iter()
            .find(|p| p.name == cs.current_provider)
            .map(|p| p.models.clone())
            .unwrap_or_default()
    });

    view! {
        <div
            class="settings-page"
            style:padding=spacing::SPACE_24
            style:height="100%"
            style:overflow-y="auto"
            style:background-color=colors::BG_PRIMARY
            style:color=colors::TEXT_PRIMARY
            style:font-family="Inter, sans-serif"
        >
            <h2
                style:font-size="1.5rem"
                style:font-weight="600"
                style:margin-bottom=spacing::SPACE_24
            >
                "Settings"
            </h2>

            <section style:margin-bottom=spacing::SPACE_32>
                <label
                    style:display="block"
                    style:font-size="0.875rem"
                    style:font-weight="500"
                    style:margin-bottom=spacing::SPACE_8
                    style:color=colors::TEXT_SECONDARY
                >
                    "Provider"
                </label>
                <select
                    style:width="100%"
                    style:max-width="400px"
                    style:padding="8px 12px"
                    style:border-radius="6px"
                    style:border=format!("1px solid {}", colors::BORDER)
                    style:background-color=colors::BG_ELEVATED
                    style:color=colors::TEXT_PRIMARY
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        config_actions.set_provider.run(val);
                    }
                >
                    {move || {
                        providers
                            .get()
                            .into_iter()
                            .map(|p| {
                                let name = p.name.clone();
                                let label = name.clone();
                                let selected =
                                    config_state.get().current_provider == name;
                                view! {
                                    <option value={name} selected={selected}>
                                        {label}
                                    </option>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </section>

            <section style:margin-bottom=spacing::SPACE_32>
                <label
                    style:display="block"
                    style:font-size="0.875rem"
                    style:font-weight="500"
                    style:margin-bottom=spacing::SPACE_8
                    style:color=colors::TEXT_SECONDARY
                >
                    "Model"
                </label>
                <select
                    style:width="100%"
                    style:max-width="400px"
                    style:padding="8px 12px"
                    style:border-radius="6px"
                    style:border=format!("1px solid {}", colors::BORDER)
                    style:background-color=colors::BG_ELEVATED
                    style:color=colors::TEXT_PRIMARY
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        config_actions.set_model.run(val);
                    }
                >
                    {move || {
                        models
                            .get()
                            .into_iter()
                            .map(|m| {
                                let label = m.clone();
                                let selected =
                                    config_state.get().current_model == m;
                                view! {
                                    <option value={m} selected={selected}>
                                        {label}
                                    </option>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </section>

            <section style:margin-bottom=spacing::SPACE_32>
                <label
                    style:display="block"
                    style:font-size="0.875rem"
                    style:font-weight="500"
                    style:margin-bottom=spacing::SPACE_8
                    style:color=colors::TEXT_SECONDARY
                >
                    "Theme"
                </label>
                <button
                    style:padding="8px 16px"
                    style:border-radius="6px"
                    style:border="none"
                    style:cursor="pointer"
                    style:background-color=colors::ACCENT
                    style:color="#0a0a0a"
                    style:font-weight="600"
                    on:click=move |_| config_actions.toggle_theme.run(())
                >
                    {move || match config_state.get().theme {
                        ThemeMode::Dark => "Switch to Light",
                        ThemeMode::Light => "Switch to Dark",
                    }}
                </button>
            </section>

            <section>
                <label
                    style:display="block"
                    style:font-size="0.875rem"
                    style:font-weight="500"
                    style:margin-bottom=spacing::SPACE_8
                    style:color=colors::TEXT_SECONDARY
                >
                    "API Endpoint"
                </label>
                <input
                    type="text"
                    value={move || config_state.get().api_endpoint.clone()}
                    style:width="100%"
                    style:max-width="400px"
                    style:padding="8px 12px"
                    style:border-radius="6px"
                    style:border=format!("1px solid {}", colors::BORDER)
                    style:background-color=colors::BG_ELEVATED
                    style:color=colors::TEXT_PRIMARY
                    on:input=move |ev| {
                        let val = event_target_value(&ev);
                        config_actions.set_endpoint.run(val);
                    }
                />
            </section>
        </div>
    }
}
