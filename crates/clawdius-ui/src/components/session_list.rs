//! Session list component.
//!
//! Displays chat session history with search,
//! date grouping, keyboard navigation, and management actions.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: u32,
    pub provider: String,
    pub model: String,
    pub preview: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    Older,
}

fn classify_date(ts: i64) -> DateGroup {
    let _ = ts;
    DateGroup::Older
}

fn group_label(group: &DateGroup) -> &'static str {
    match group {
        DateGroup::Today => "Today",
        DateGroup::Yesterday => "Yesterday",
        DateGroup::ThisWeek => "This Week",
        DateGroup::Older => "Older",
    }
}

fn format_relative_time(ts: i64) -> String {
    let _ = ts;
    String::from("--")
}

#[component]
pub fn SessionList(
    #[prop(into)] sessions: Vec<SessionSummary>,
    #[prop(optional)] active_id: Option<String>,
    #[prop(optional)] on_select: Option<Callback<String>>,
    #[prop(optional)] on_delete: Option<Callback<String>>,
) -> impl IntoView {
    let (search, set_search) = signal(String::new());
    let (focused_idx, set_focused_idx) = signal(0usize);

    let filtered = Memo::new(move |_| {
        let q = search.get().to_lowercase();
        sessions
            .iter()
            .filter(|s| {
                if q.is_empty() {
                    return true;
                }
                s.title.to_lowercase().contains(&q) || s.model.to_lowercase().contains(&q)
            })
            .cloned()
            .collect::<Vec<_>>()
    });

    let grouped = move || {
        let items = filtered.get();
        let mut groups: Vec<(DateGroup, Vec<SessionSummary>)> = Vec::new();
        for session in items {
            let group = classify_date(session.updated_at);
            if let Some(entry) = groups.iter_mut().find(|(g, _)| g == &group) {
                entry.1.push(session);
            } else {
                groups.push((group, vec![session]));
            }
        }
        let order = [
            DateGroup::Today,
            DateGroup::Yesterday,
            DateGroup::ThisWeek,
            DateGroup::Older,
        ];
        groups.sort_by_key(|(g, _)| order.iter().position(|o| o == g).unwrap_or(99));
        groups
    };

    let flat_count = move || filtered.get().len();

    view! {
        <div
            class="session-list"
            role="list"
            aria-label="Chat sessions"
            style:display="flex"
            style:flex-direction="column"
            style:height="100%"
            style:background-color=colors::BG_PRIMARY
            style:font-family=typography::FONT_SANS
        >
            <div
                class="session-list-header"
                style:padding=spacing::SPACE_12
                style:border-bottom=format!("1px solid {}", colors::BORDER)
            >
                <input
                    class="session-search"
                    type="text"
                    placeholder="Search sessions..."
                    aria-label="Search sessions"
                    prop:value=move || search.get()
                    on:input=move |ev: web_sys::Event| {
                        let target: web_sys::HtmlInputElement = ev.target().unwrap().unchecked_into();
                        set_search.set(target.value());
                    }
                    style:width="100%"
                    style:background-color=colors::BG_SURFACE
                    style:border=format!("1px solid {}", colors::BORDER)
                    style:border-radius=radius::SM
                    style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                    style:color=colors::TEXT_PRIMARY
                    style:font-size=typography::SIZE_SM
                    style:outline="none"
                    style:box-sizing="border-box"
                />
            </div>
            <div
                class="session-list-items"
                style:flex="1"
                style:overflow-y="auto"
                style:padding=format!("{} 0", spacing::SPACE_4)
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" => {
                            ev.prevent_default();
                            set_focused_idx.update(|i| {
                                if *i < flat_count().saturating_sub(1) { *i += 1; }
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            set_focused_idx.update(|i| { *i = i.saturating_sub(1); });
                        }
                        "Enter" => {
                            let items = filtered.get();
                            if let Some(session) = items.get(focused_idx.get()) {
                                if let Some(cb) = on_select {
                                    cb.run(session.id.clone());
                                }
                            }
                        }
                        "Delete" => {
                            let items = filtered.get();
                            if let Some(session) = items.get(focused_idx.get()) {
                                if let Some(cb) = on_delete {
                                    cb.run(session.id.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            >
                {move || {
                    let groups = grouped();
                    let mut idx = 0usize;
                    let focused = focused_idx.get();
                    groups.into_iter().flat_map(|(group, items)| {
                        let label = group_label(&group);
                        let mut result = vec![view! {
                            <div
                                class="session-group-label"
                                style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                                style:color=colors::TEXT_MUTED
                                style:font-size=typography::SIZE_XS
                                style:font-weight=typography::WEIGHT_SEMIBOLD
                                style:text-transform="uppercase"
                                style:letter-spacing="0.05em"
                            >
                                {label}
                            </div>
                        }.into_any()];
                        for session in items {
                            let current_idx = idx;
                            idx += 1;
                            let is_active = active_id.as_ref() == Some(&session.id);
                            let is_focused = current_idx == focused;
                            let sid = session.id.clone();
                            let sid_del = session.id.clone();
                            let sel_cb = on_select;
                            let del_cb = on_delete;
                            let preview = session.preview.clone();
                            let time_str = format_relative_time(session.updated_at);
                            let sess_model = session.model.clone();
                            let msg_count = session.message_count;
                            let sess_title = session.title.clone();
                            result.push(view! {
                                <div
                                    class=format!("session-item{}", if is_active { " session-active" } else { "" })
                                    role="listitem"
                                    aria-selected=is_active
                                    tabindex=if is_focused { "0" } else { "-1" }
                                    style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                                    style:cursor="pointer"
                                    style:background-color=if is_active { colors::BG_ELEVATED } else if is_focused { colors::BG_SURFACE } else { "transparent" }
                                    style:border-left=if is_active { format!("2px solid {}", colors::ACCENT) } else { "2px solid transparent".to_string() }
                                    style:border-radius=radius::SM
                                    style:margin=format!("0 {}", spacing::SPACE_4)
                                    on:click=move |_| {
                                        if let Some(cb) = sel_cb { cb.run(sid.clone()); }
                                    }
                                >
                                    <div style:display="flex" style:justify-content="space-between" style:align-items="flex-start">
                                        <div class="session-title" style:color=if is_active { colors::TEXT_PRIMARY } else { colors::TEXT_SECONDARY } style:font-size=typography::SIZE_SM style:font-weight=if is_active { typography::WEIGHT_MEDIUM } else { typography::WEIGHT_NORMAL } style:overflow="hidden" style:text-overflow="ellipsis" style:white-space="nowrap" style:max-width="80%">
                                            {sess_title}
                                        </div>
                                        {del_cb.map(move |_| {
                                            let del_id = sid_del.clone();
                                            let cb = del_cb;
                                            view! {
                                                <button
                                                    class="session-delete"
                                                    aria-label="Delete session"
                                                    style:background="transparent"
                                                    style:border="none"
                                                    style:color=colors::TEXT_MUTED
                                                    style:cursor="pointer"
                                                    style:font-size=typography::SIZE_XS
                                                    style:padding=spacing::SPACE_4
                                                    style:border-radius=radius::SM
                                                    on:click=move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        if let Some(c) = cb { c.run(del_id.clone()); }
                                                    }
                                                >
                                                    "x"
                                                </button>
                                            }
                                        })}
                                    </div>
                                    {preview.map(|p| view! {
                                        <div
                                            class="session-preview"
                                            style:color=colors::TEXT_MUTED
                                            style:font-size=typography::SIZE_XS
                                            style:margin-top=spacing::SPACE_4
                                            style:overflow="hidden"
                                            style:text-overflow="ellipsis"
                                            style:white-space="nowrap"
                                        >
                                            {p}
                                        </div>
                                    })}
                                    <div class="session-meta" style:display="flex" style:gap=spacing::SPACE_12 style:margin-top=spacing::SPACE_4 style:align-items="center">
                                        <span style:color=colors::TEXT_MUTED style:font-size=typography::SIZE_XS style:font-family=typography::FONT_MONO>
                                            {sess_model}
                                        </span>
                                        <span style:color=colors::TEXT_MUTED style:font-size=typography::SIZE_XS>
                                            {format!("{} msg", msg_count)}
                                        </span>
                                        <span style:color=colors::TEXT_MUTED style:font-size=typography::SIZE_XS>
                                            {time_str}
                                        </span>
                                    </div>
                                </div>
                            }.into_any());
                        }
                        result
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}
