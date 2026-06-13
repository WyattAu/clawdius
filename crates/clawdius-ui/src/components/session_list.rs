//! Session list component.
//!
//! Displays chat session history with search,
//! date grouping, and management actions.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Summary of a chat session.
#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: u32,
    pub provider: String,
    pub model: String,
}

/// Renders a searchable session list.
#[component]
pub fn SessionList(
    /// Sessions to display.
    #[prop(into)]
    sessions: Vec<SessionSummary>,
    /// Currently active session ID.
    #[prop(optional)]
    active_id: Option<String>,
) -> impl IntoView {
    view! {
        <div class="session-list">
            <div class="session-list-header">
                <input
                    class="session-search"
                    type="text"
                    placeholder="Search sessions..."
                />
            </div>
            <div class="session-list-items">
                {sessions.into_iter().map(|session| {
                    let is_active = active_id.as_ref() == Some(&session.id);
                    let active_class = if is_active { "session-active" } else { "" };
                    view! {
                        <div class=format!("session-item {active_class}")>
                            <div class="session-title">{session.title}</div>
                            <div class="session-meta">
                                <span>{session.model}</span>
                                <span>{format!("{} messages", session.message_count)}</span>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
