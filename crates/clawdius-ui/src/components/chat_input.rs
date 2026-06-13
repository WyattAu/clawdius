//! Chat input component with autocomplete support.
//!
//! Supports `/command` autocomplete and `@file` mentions,
//! multi-line editing, and paste handling.

use leptos::prelude::*;
#[allow(unused_imports)]
use leptos::wasm_bindgen::JsCast;
use leptos::{component, view, IntoView};

/// Autocomplete suggestion.
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Renders the chat input area with autocomplete.
#[component]
pub fn ChatInput(
    /// Placeholder text.
    #[prop(default = "Type a message...")]
    placeholder: &'static str,
    /// Callback when message is submitted.
    on_submit: impl Fn(String) + 'static,
) -> impl IntoView {
    let (input_value, set_input_value): (ReadSignal<String>, WriteSignal<String>) =
        signal(String::new());
    let (suggestions, set_suggestions): (
        ReadSignal<Vec<Suggestion>>,
        WriteSignal<Vec<Suggestion>>,
    ) = signal(Vec::new());

    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            let value = input_value.get();
            if !value.trim().is_empty() {
                on_submit(value);
                set_input_value.set(String::new());
                set_suggestions.set(Vec::new());
            }
        }
    };

    let handle_input = move |ev: web_sys::Event| {
        let target: web_sys::HtmlInputElement = ev.target().unwrap().unchecked_into();
        let value = target.value();
        set_input_value.set(value.clone());

        // Detect command autocomplete
        if let Some(cmd) = value.strip_prefix('/') {
            let commands = vec![
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
            ];
            let filtered: Vec<_> = commands
                .into_iter()
                .filter(|s| s.label.contains(cmd))
                .collect();
            set_suggestions.set(filtered);
        } else if let Some(at_pos) = value.rfind('@') {
            // File mention autocomplete
            let query = &value[at_pos + 1..];
            if !query.contains(' ') && !query.is_empty() {
                // TODO: query file index from clawdius-core
                set_suggestions.set(vec![Suggestion {
                    id: "src/lib.rs".into(),
                    label: "src/lib.rs".into(),
                    description: None,
                }]);
            }
        } else {
            set_suggestions.set(Vec::new());
        }
    };

    view! {
        <div class="chat-input-container">
            {move || {
                let sug = suggestions.get();
                if !sug.is_empty() {
                    view! {
                        <div class="autocomplete-dropdown">
                            {sug.into_iter().map(|s| view! {
                                <div class="autocomplete-item">
                                    <span class="autocomplete-label">{s.label}</span>
                                    {s.description.map(|d| view! {
                                        <span class="autocomplete-desc">{d}</span>
                                    })}
                                </div>
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                } else {
                    ().into_any()
                }
            }}
            <div class="chat-input-row">
                <textarea
                    class="chat-input"
                    placeholder=placeholder
                    prop:value=move || input_value.get()
                    on:keydown=handle_keydown
                    on:input=handle_input
                    rows="1"
                />
            </div>
        </div>
    }
}
