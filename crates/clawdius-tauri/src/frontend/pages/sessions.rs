use leptos::prelude::*;
use leptos::{component, view, IntoView};

use clawdius_ui::components::message::{ChatMessage, MessageRole};
use clawdius_ui::components::session_list::SessionList as SessionListComponent;
use clawdius_ui::components::session_list::SessionSummary as UiSessionSummary;
use clawdius_ui::hooks::use_chat::ChatState;
use clawdius_ui::theme::colors;
use clawdius_ui::theme::spacing;

#[component]
pub fn SessionsPage(chat_state: RwSignal<ChatState>) -> impl IntoView {
    let (selected_session, set_selected_session) = signal(None::<String>);
    let (_search_query, _set_search_query) = signal(String::new());

    let sessions = Memo::new(move |_| {
        let _ = chat_state.get();
        vec![
            UiSessionSummary {
                id: "session-1".into(),
                title: "Rust ownership discussion".into(),
                created_at: 1700000000000i64,
                updated_at: 1700000100000i64,
                message_count: 12,
                provider: "anthropic".into(),
                model: "claude-sonnet-4-20250514".into(),
                preview: Some("How does borrowing work?".into()),
            },
            UiSessionSummary {
                id: "session-2".into(),
                title: "API design patterns".into(),
                created_at: 1699900000000i64,
                updated_at: 1699900100000i64,
                message_count: 8,
                provider: "openai".into(),
                model: "gpt-4o".into(),
                preview: Some("REST vs GraphQL pros and cons".into()),
            },
        ]
    });

    let on_select = Callback::new(move |id: String| {
        set_selected_session.set(Some(id));
    });

    let on_delete = Callback::new(move |id: String| {
        let _ = id;
    });

    let selected_messages = Memo::new(move |_| {
        let sel = selected_session.get();
        match sel.as_deref() {
            Some("session-1") => vec![
                ChatMessage {
                    id: "m1".into(),
                    role: MessageRole::User,
                    content: "How does borrowing work in Rust?".into(),
                    timestamp: 1700000000000i64,
                    model: None,
                    tokens_used: None,
                    is_streaming: false,
                },
                ChatMessage {
                    id: "m2".into(),
                    role: MessageRole::Assistant,
                    content: "Borrowing in Rust allows you to reference data without taking ownership. There are two types: immutable references (&T) and mutable references (&mut T). The key rules are...".into(),
                    timestamp: 1700000001000i64,
                    model: Some("claude-sonnet-4-20250514".into()),
                    tokens_used: Some(150),
                    is_streaming: false,
                },
            ],
            _ => vec![],
        }
    });

    view! {
        <div
            class="sessions-page"
            style:display="flex"
            style:height="100%"
            style:background-color=colors::BG_PRIMARY
        >
            <div
                class="sessions-sidebar"
                style:width="320px"
                style:flex-shrink="0"
                style:border-right=format!("1px solid {}", colors::BORDER)
            >
                <SessionListComponent
                    sessions=sessions.get()
                    active_id=selected_session.get()
                    on_select=Some(on_select)
                    on_delete=Some(on_delete)
                />
            </div>
            <div
                class="session-detail"
                style:flex="1"
                style:overflow-y="auto"
                style:padding=spacing::SPACE_24
            >
                {move || {
                    let msgs = selected_messages.get();
                    if msgs.is_empty() {
                        view! {
                            <div
                                class="sessions-empty"
                                style:display="flex"
                                style:align-items="center"
                                style:justify-content="center"
                                style:height="100%"
                                style:color=colors::TEXT_MUTED
                                style:font-size="0.875rem"
                            >
                                "Select a session to view messages"
                            </div>
                        }.into_any()
                    } else {
                        msgs.into_iter()
                            .map(|msg| {
                                view! {
                                    <div style:margin-bottom=spacing::SPACE_12>
                                        <span
                                            style:color=colors::ACCENT
                                            style:font-size="0.75rem"
                                            style:font-weight="600"
                                            style:text-transform="uppercase"
                                        >
                                            {match msg.role {
                                                MessageRole::User => "You",
                                                MessageRole::Assistant => "Assistant",
                                                MessageRole::System => "System",
                                                MessageRole::Tool => "Tool",
                                            }}
                                        </span>
                                        <p
                                            style:color=colors::TEXT_PRIMARY
                                            style:margin-top="4px"
                                            style:line-height="1.5"
                                        >
                                            {msg.content}
                                        </p>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}
