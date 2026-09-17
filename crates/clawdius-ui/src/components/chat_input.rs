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
use leptos::wasm_bindgen::JsCast;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Copy of the reactive state shared by the chat-input sub-views.
#[derive(Clone, Copy)]
struct ChatInputSignals {
    input_value: ReadSignal<String>,
    set_input_value: WriteSignal<String>,
    suggestions: ReadSignal<Vec<Suggestion>>,
    set_suggestions: WriteSignal<Vec<Suggestion>>,
    selected_idx: ReadSignal<usize>,
    set_selected_idx: WriteSignal<usize>,
    is_focused: RwSignal<bool>,
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

/// Compute the suggestion list for the current input value.
///
/// Returns `None` when no suggestion set applies (the list should clear).
fn compute_suggestions(value: &str) -> Option<Vec<Suggestion>> {
    value.strip_prefix('/').map_or_else(
        || {
            value.rfind('@').and_then(|at_pos| {
                let query = &value[at_pos + 1..];
                (!query.contains(' ') && !query.is_empty()).then(|| {
                    vec![
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
                    ]
                })
            })
        },
        |cmd| {
            Some(
                build_commands()
                    .into_iter()
                    .filter(|s| s.label[1..].starts_with(cmd) || s.label.contains(cmd))
                    .collect(),
            )
        },
    )
}

fn update_suggestions(s: ChatInputSignals, value: &str) {
    compute_suggestions(value).map_or_else(
        || s.set_suggestions.set(Vec::new()),
        |list| {
            s.set_suggestions.set(list);
            s.set_selected_idx.set(0);
        },
    );
}

/// Build the input value after accepting a suggestion.
fn complete_suggestion(current: &str, id: &str, label: &str) -> String {
    if current.starts_with('/') {
        format!("/{id} ")
    } else if let Some(at_pos) = current.rfind('@') {
        format!("{}{label} ", &current[..at_pos])
    } else {
        current.to_string()
    }
}

fn input_handler(s: ChatInputSignals, ev: &web_sys::Event) {
    let Some(target) = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    else {
        // No usable target (e.g. synthetic event); nothing to mirror.
        return;
    };
    let value = target.value();
    s.set_input_value.set(value.clone());
    update_suggestions(s, &value);
}

fn keydown_handler<F: Fn(String)>(
    s: ChatInputSignals,
    ev: &web_sys::KeyboardEvent,
    disabled: bool,
    on_submit: &F,
) {
    let sug = s.suggestions.get();
    let handled = !sug.is_empty()
        && match ev.key().as_str() {
            "ArrowDown" => {
                ev.prevent_default();
                s.set_selected_idx
                    .set((s.selected_idx.get() + 1).min(sug.len() - 1));
                true
            },
            "ArrowUp" => {
                ev.prevent_default();
                s.set_selected_idx
                    .set(s.selected_idx.get().saturating_sub(1));
                true
            },
            "Tab" | "Enter" => {
                ev.prevent_default();
                if let Some(selected) = sug.into_iter().nth(s.selected_idx.get()) {
                    let new_val =
                        complete_suggestion(&s.input_value.get(), &selected.id, &selected.label);
                    s.set_input_value.set(new_val);
                    s.set_suggestions.set(Vec::new());
                }
                true
            },
            "Escape" => {
                s.set_suggestions.set(Vec::new());
                true
            },
            _ => false,
        };
    if !handled && ev.key() == "Enter" && !ev.shift_key() {
        ev.prevent_default();
        let value = s.input_value.get();
        if !value.trim().is_empty() && !disabled {
            on_submit(value);
            s.set_input_value.set(String::new());
            s.set_suggestions.set(Vec::new());
        }
    }
}

fn suggestions_dropdown(s: ChatInputSignals) -> impl IntoView {
    let sug = s.suggestions.get();
    let is_cmd = sug.first().is_some_and(|sg| sg.id.starts_with('/'))
        || s.input_value.get().starts_with('/');
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
            {sug.into_iter().enumerate().map(|(i, sg)| {
                let is_sel = i == s.selected_idx.get();
                let bg = if is_sel { colors::BG_SURFACE } else { "transparent" };
                let sg_id = sg.id.clone();
                let sg_label = sg.label.clone();
                let sg_desc = sg.description.clone();
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
                            let current = s.input_value.get();
                            let new_val = complete_suggestion(&current, &sg_id, &sg_label);
                            s.set_input_value.set(new_val);
                            s.set_suggestions.set(Vec::new());
                        }
                    >
                        <span
                            class="autocomplete-label"
                            style:font-family=typography::FONT_MONO
                            style:font-size=typography::SIZE_SM
                            style:color=colors::ACCENT
                        >
                            {sg.label}
                        </span>
                        {sg_desc.map(|d| view! {
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
    }
}

fn attach_button(disabled: bool) -> impl IntoView {
    view! {
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
    }
}

fn token_counter(
    input_value: ReadSignal<String>,
    token_count: u32,
    token_limit: u32,
) -> impl IntoView {
    let char_count = move || input_value.get().len();
    let count_color = move || {
        let pct = token_count
            .checked_mul(100)
            .and_then(|v| v.checked_div(token_limit))
            .unwrap_or(0);
        if pct > 90 {
            colors::ERROR
        } else if pct > 70 {
            colors::WARNING
        } else {
            colors::TEXT_MUTED
        }
    };
    view! {
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
    }
}

fn input_row<F: Fn(String) + 'static>(
    s: ChatInputSignals,
    placeholder: &'static str,
    disabled: bool,
    token_count: u32,
    token_limit: u32,
    on_submit: F,
) -> impl IntoView {
    let show_border = move || {
        if s.is_focused.get() {
            colors::ACCENT_DIM
        } else {
            colors::BORDER
        }
    };
    view! {
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
            {attach_button(disabled)}
            <textarea
                class="chat-input"
                placeholder=placeholder
                prop:value=move || s.input_value.get()
                on:keydown=move |ev| keydown_handler(s, &ev, disabled, &on_submit)
                on:input=move |ev| input_handler(s, &ev)
                on:focus=move |_| s.is_focused.set(true)
                on:blur=move |_| s.is_focused.set(false)
                rows="1"
                disabled=disabled
                role="combobox"
                aria-expanded=move || !s.suggestions.get().is_empty()
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
            {token_counter(s.input_value, token_count, token_limit)}
        </div>
    }
}

#[must_use]
#[component]
pub fn ChatInput(
    #[prop(default = "Type a message...")] placeholder: &'static str,
    #[prop(default = false)] disabled: bool,
    #[prop(default = 0)] token_count: u32,
    #[prop(default = 200000)] token_limit: u32,
    on_submit: impl Fn(String) + 'static,
) -> impl IntoView {
    let (input_value, set_input_value) = signal(String::new());
    let (suggestions, set_suggestions) = signal(Vec::<Suggestion>::new());
    let (selected_idx, set_selected_idx) = signal(0usize);
    let is_focused = RwSignal::new(false);
    let s = ChatInputSignals {
        input_value,
        set_input_value,
        suggestions,
        set_suggestions,
        selected_idx,
        set_selected_idx,
        is_focused,
    };
    let opacity = move || if disabled { "0.5" } else { "1.0" };

    view! {
        <div
            class="chat-input-container"
            style:position="relative"
            style:opacity=opacity
        >
            {move || {
                if s.suggestions.get().is_empty() {
                    ().into_any()
                } else {
                    suggestions_dropdown(s).into_any()
                }
            }}
            {input_row(s, placeholder, disabled, token_count, token_limit, on_submit)}
        </div>
    }
}
