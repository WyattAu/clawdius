//! Chat input component with autocomplete support.
//!
//! Supports `/command` autocomplete and `@file` mentions,
//! multi-line editing, paste handling, and keyboard navigation.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use crate::theme::z_index;
use leptos::prelude::*;
#[allow(unused_imports)]
use leptos::wasm_bindgen::JsCast;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

fn build_commands() -> Vec<Suggestion> {
    vec![
        Suggestion {
            id: "help".into(),
            label: "/help".into(),
            description: Some("Show available commands".into()),
        },
        Suggestion {
            id: "provider".into(),
            label: "/provider".into(),
            description: Some("Switch LLM provider".into()),
        },
        Suggestion {
            id: "model".into(),
            label: "/model".into(),
            description: Some("Switch model".into()),
        },
        Suggestion {
            id: "sessions".into(),
            label: "/sessions".into(),
            description: Some("List sessions".into()),
        },
        Suggestion {
            id: "undo".into(),
            label: "/undo".into(),
            description: Some("Undo last change".into()),
        },
        Suggestion {
            id: "checkpoint".into(),
            label: "/checkpoint".into(),
            description: Some("Create checkpoint".into()),
        },
        Suggestion {
            id: "clear".into(),
            label: "/clear".into(),
            description: Some("Clear conversation".into()),
        },
        Suggestion {
            id: "compact".into(),
            label: "/compact".into(),
            description: Some("Compact context window".into()),
        },
    ]
}

#[component]
pub fn ChatInput(
    #[prop(default = "Type a message...")] placeholder: &'static str,
    #[prop(default = false)] disabled: bool,
    #[prop(default = 0)] token_count: u32,
    #[prop(default = 200000)] token_limit: u32,
    on_submit: impl Fn(String) + 'static,
) -> impl IntoView {
    let (input_value, set_input_value): (ReadSignal<String>, WriteSignal<String>) =
        signal(String::new());
    let (suggestions, set_suggestions): (
        ReadSignal<Vec<Suggestion>>,
        WriteSignal<Vec<Suggestion>>,
    ) = signal(Vec::new());
    let (selected_idx, set_selected_idx) = signal(0usize);
    let (is_focused, set_is_focused) = signal(false);

    let char_count = move || input_value.get().len();
    let count_pct = move || {
        if token_limit == 0 {
            0u32
        } else {
            token_count * 100 / token_limit
        }
    };
    let count_color = move || {
        let pct = count_pct();
        if pct > 90 {
            colors::ERROR
        } else if pct > 70 {
            colors::WARNING
        } else {
            colors::TEXT_MUTED
        }
    };

    let update_suggestions = move |value: &str| {
        if let Some(cmd) = value.strip_prefix('/') {
            let filtered: Vec<_> = build_commands()
                .into_iter()
                .filter(|s| s.label[1..].starts_with(cmd) || s.label.contains(cmd))
                .collect();
            set_suggestions.set(filtered);
            set_selected_idx.set(0);
        } else if let Some(at_pos) = value.rfind('@') {
            let query = &value[at_pos + 1..];
            if !query.contains(' ') && !query.is_empty() {
                set_suggestions.set(vec![
                    Suggestion {
                        id: "src/lib.rs".into(),
                        label: "src/lib.rs".into(),
                        description: None,
                    },
                    Suggestion {
                        id: "Cargo.toml".into(),
                        label: "Cargo.toml".into(),
                        description: None,
                    },
                ]);
                set_selected_idx.set(0);
            } else {
                set_suggestions.set(Vec::new());
            }
        } else {
            set_suggestions.set(Vec::new());
        }
    };

    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        let sug = suggestions.get();
        if !sug.is_empty() {
            match ev.key().as_str() {
                "ArrowDown" => {
                    ev.prevent_default();
                    let next = (selected_idx.get() + 1).min(sug.len() - 1);
                    set_selected_idx.set(next);
                    return;
                },
                "ArrowUp" => {
                    ev.prevent_default();
                    let prev = selected_idx.get().saturating_sub(1);
                    set_selected_idx.set(prev);
                    return;
                },
                "Tab" | "Enter" => {
                    ev.prevent_default();
                    if let Some(s) = sug.into_iter().nth(selected_idx.get()) {
                        let current = input_value.get();
                        let new_val = if current.starts_with('/') {
                            format!("/{} ", s.id)
                        } else if let Some(at_pos) = current.rfind('@') {
                            format!("{}{} ", &current[..at_pos], s.label)
                        } else {
                            current
                        };
                        set_input_value.set(new_val);
                        set_suggestions.set(Vec::new());
                    }
                    return;
                },
                "Escape" => {
                    set_suggestions.set(Vec::new());
                    return;
                },
                _ => {},
            }
        }
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            let value = input_value.get();
            if !value.trim().is_empty() && !disabled {
                on_submit(value);
                set_input_value.set(String::new());
                set_suggestions.set(Vec::new());
            }
        }
    };

    let handle_input = move |ev: web_sys::Event| {
        let target: web_sys::HtmlTextAreaElement = ev.target().unwrap().unchecked_into();
        let value = target.value();
        set_input_value.set(value.clone());
        update_suggestions(&value);
    };

    let has_suggestions = move || !suggestions.get().is_empty();
    let show_border = move || {
        if is_focused.get() {
            colors::ACCENT_DIM
        } else {
            colors::BORDER
        }
    };
    let opacity = move || if disabled { "0.5" } else { "1.0" };

    view! {
        <div
            class="chat-input-container"
            style:position="relative"
            style:opacity=opacity
        >
            {move || {
                let sug = suggestions.get();
                if !sug.is_empty() {
                    let is_cmd = sug.first().map(|s| s.id.starts_with('/')).unwrap_or(false) || input_value.get().starts_with('/');
                    view! {
                        <div
                            class="autocomplete-dropdown"
                            role="listbox"
                            aria-label=if is_cmd { "Command suggestions" } else { "File suggestions" }
                            style:position="absolute"
                            style:bottom="100%"
                            style:left="0"
                            style:right="0"
                            style:background-color=colors::BG_ELEVATED
                            style:border=format!("1px solid {}", colors::BORDER)
                            style:border-radius=radius::MD
                            style:max-height="200px"
                            style:overflow-y="auto"
                            style:z-index=z_index::DROPDOWN.to_string()
                            style:margin-bottom=spacing::SPACE_4
                        >
                            {sug.into_iter().enumerate().map(|(i, s)| {
                                let is_sel = i == selected_idx.get();
                                let bg = if is_sel { colors::BG_SURFACE } else { "transparent" };
                                let s_id = s.id.clone();
                                let s_label = s.label.clone();
                                let s_desc = s.description.clone();
                                view! {
                                    <div
                                        class=format!("autocomplete-item{}", if is_sel { " selected" } else { "" })
                                        role="option"
                                        aria-selected=is_sel
                                        style:display="flex"
                                        style:justify-content="space-between"
                                        style:align-items="center"
                                        style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                                        style:background-color=bg
                                        style:cursor="pointer"
                                        style:border-radius=radius::SM
                                        style:color=if is_sel { colors::TEXT_PRIMARY } else { colors::TEXT_SECONDARY }
                                        on:click=move |_| {
                                            let current = input_value.get();
                                            let new_val = if current.starts_with('/') {
                                                format!("/{} ", s_id)
                                            } else if let Some(at_pos) = current.rfind('@') {
                                                format!("{}{} ", &current[..at_pos], s_label)
                                            } else {
                                                current
                                            };
                                            set_input_value.set(new_val);
                                            set_suggestions.set(Vec::new());
                                        }
                                    >
                                        <span
                                            class="autocomplete-label"
                                            style:font-family=typography::FONT_MONO
                                            style:font-size=typography::SIZE_SM
                                            style:color=colors::ACCENT
                                        >
                                            {s.label}
                                        </span>
                                        {s_desc.map(|d| view! {
                                            <span
                                                class="autocomplete-desc"
                                                style:font-size=typography::SIZE_XS
                                                style:color=colors::TEXT_MUTED
                                            >
                                                {d}
                                            </span>
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
            <div
                class="chat-input-row"
                style:display="flex"
                style:align-items="flex-end"
                style:gap=spacing::SPACE_8
                style:padding=spacing::SPACE_12
                style:background-color=colors::BG_SURFACE
                style:border=format!("1px solid {}", show_border())
                style:border-radius=radius::LG
                style:transition=format!("border-color {}", crate::theme::transition::FAST)
            >
                <button
                    class="chat-attach"
                    title="Attach file"
                    aria-label="Attach file"
                    style:background="transparent"
                    style:border="none"
                    style:color=colors::TEXT_SECONDARY
                    style:cursor="pointer"
                    style:padding=spacing::SPACE_8
                    style:font-size=typography::SIZE_LG
                    style:flex-shrink="0"
                    disabled=disabled
                >
                    "+"
                </button>
                <textarea
                    class="chat-input"
                    placeholder=placeholder
                    prop:value=move || input_value.get()
                    on:keydown=handle_keydown
                    on:input=handle_input
                    on:focus=move |_| set_is_focused.set(true)
                    on:blur=move |_| set_is_focused.set(false)
                    rows="1"
                    disabled=disabled
                    role="combobox"
                    aria-expanded=has_suggestions
                    aria-haspopup="listbox"
                    aria-autocomplete="list"
                    style:flex="1"
                    style:background="transparent"
                    style:border="none"
                    style:outline="none"
                    style:color=colors::TEXT_PRIMARY
                    style:font-family=typography::FONT_SANS
                    style:font-size=typography::SIZE_BASE
                    style:line-height=typography::LINE_HEIGHT_NORMAL
                    style:resize="none"
                    style:min-height="24px"
                    style:max-height="200px"
                />
                <span
                    class="input-counter"
                    style:color=count_color()
                    style:font-family=typography::FONT_MONO
                    style:font-size=typography::SIZE_XS
                    style:flex-shrink="0"
                    style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                    style:white-space="nowrap"
                >
                    {move || format!("{}/{}", char_count(), token_limit)}
                </span>
            </div>
        </div>
    }
}
