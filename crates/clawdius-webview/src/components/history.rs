use super::common::{Button, ButtonVariant, Modal, SearchInput};
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    pub title: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub preview: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub session: SessionInfo,
    pub messages: Vec<SessionMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DateFilter {
    All,
    Today,
    ThisWeek,
    ThisMonth,
}

#[component]
pub fn HistoryView() -> impl IntoView {
    let (sessions, set_sessions) = create_signal::<Vec<SessionInfo>>(Vec::new());
    let (search_query, set_search_query) = create_signal(String::new());
    let (selected_session, set_selected_session) = create_signal::<Option<String>>(None);
    let (session_data, set_session_data) = create_signal::<Option<SessionData>>(None);
    let (date_filter, set_date_filter) = create_signal(DateFilter::All);
    let (provider_filter, set_provider_filter) = create_signal(String::new());
    let (show_delete_modal, set_show_delete_modal) = create_signal(false);
    let (session_to_delete, set_session_to_delete) = create_signal::<Option<String>>(None);
    let (loading, set_loading) = create_signal(false);

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
        send_to_vscode("getSessions", serde_json::json!({}));
        set_loading.set(true);
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
                                        "sessionsList" => {
                                            if let Some(sessions_data) = msg.get("data") {
                                                if let Ok(sessions_list) =
                                                    serde_json::from_value::<Vec<SessionInfo>>(
                                                        sessions_data.clone(),
                                                    )
                                                {
                                                    set_sessions.set(sessions_list);
                                                    set_loading.set(false);
                                                }
                                            }
                                        }
                                        "sessionData" => {
                                            if let Some(data) = msg.get("data") {
                                                if let Ok(session) =
                                                    serde_json::from_value::<SessionData>(
                                                        data.clone(),
                                                    )
                                                {
                                                    set_session_data.set(Some(session));
                                                }
                                            }
                                        }
                                        "sessionDeleted" => {
                                            if let Some(session_id) =
                                                msg.get("data").and_then(|d| d.get("id"))
                                            {
                                                if let Some(id) = session_id.as_str() {
                                                    set_sessions.update(|s| {
                                                        s.retain(|s| s.id != id);
                                                    });
                                                    if selected_session.get().as_deref() == Some(id)
                                                    {
                                                        set_selected_session.set(None);
                                                        set_session_data.set(None);
                                                    }
                                                }
                                            }
                                        }
                                        "sessionExported" => {
                                            if let Some(content) =
                                                msg.get("data").and_then(|d| d.get("content"))
                                            {
                                                if let Some(content_str) = content.as_str() {
                                                    let _ = web_sys::window()
                                                        .unwrap()
                                                        .navigator()
                                                        .clipboard()
                                                        .write_text(content_str);
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

    let filtered_sessions = create_memo(move |_| {
        let query = search_query.get().to_lowercase();
        let _date = date_filter.get();
        let provider = provider_filter.get();

        sessions
            .get()
            .iter()
            .filter(|session| {
                let matches_search = query.is_empty()
                    || session
                        .title
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || session
                        .preview
                        .as_ref()
                        .map(|p| p.to_lowercase().contains(&query))
                        .unwrap_or(false);

                let matches_provider =
                    provider.is_empty() || session.provider.as_deref() == Some(provider.as_str());

                matches_search && matches_provider
            })
            .cloned()
            .collect::<Vec<_>>()
    });

    let load_session = move |session_id: String| {
        set_selected_session.set(Some(session_id.clone()));
        send_to_vscode("getSession", serde_json::json!({ "id": session_id }));
    };

    let delete_session = move |session_id: String| {
        set_session_to_delete.set(Some(session_id));
        set_show_delete_modal.set(true);
    };

    let confirm_delete = move |_| {
        if let Some(id) = session_to_delete.get() {
            send_to_vscode("deleteSession", serde_json::json!({ "id": id }));
        }
        set_show_delete_modal.set(false);
        set_session_to_delete.set(None);
    };

    let export_session = move |session_id: String| {
        send_to_vscode("exportSession", serde_json::json!({ "id": session_id }));
    };

    let load_session_click = load_session;
    let delete_session_click = delete_session;
    let export_session_click = export_session;

    view! {
        <div class="history-view">
            <div class="history-sidebar">
                <div class="history-controls">
                    <SearchInput
                        value=search_query.get()
                        placeholder="Search sessions...".to_string()
                        on_input=Box::new(move |ev| {
                            let value = event_target_value(&ev);
                            set_search_query.set(value);
                        })
                    />
                    <div class="filter-controls">
                        <select
                            class="filter-dropdown"
                            on:change=move |ev| {
                                let value = event_target_value(&ev);
                                let filter = match value.as_str() {
                                    "today" => DateFilter::Today,
                                    "week" => DateFilter::ThisWeek,
                                    "month" => DateFilter::ThisMonth,
                                    _ => DateFilter::All,
                                };
                                set_date_filter.set(filter);
                            }
                        >
                            <option value="all">"All Time"</option>
                            <option value="today">"Today"</option>
                            <option value="week">"This Week"</option>
                            <option value="month">"This Month"</option>
                        </select>
                        <select
                            class="filter-dropdown"
                            on:change=move |ev| {
                                set_provider_filter.set(event_target_value(&ev));
                            }
                        >
                            <option value="">"All Providers"</option>
                            <option value="anthropic">"Anthropic"</option>
                            <option value="openai">"OpenAI"</option>
                            <option value="local">"Local"</option>
                        </select>
                    </div>
                </div>
                <div class="session-list">
                    {move || {
                        if loading.get() {
                            view! {
                                <div class="loading">"Loading sessions..."</div>
                            }.into_view()
                        } else {
                            view! {
                                <For
                                    each=move || filtered_sessions.get()
                                    key=|session| session.id.clone()
                                    children=move |session| {
                                        let session_id = session.id.clone();
                                        let session_id_load = session_id.clone();
                                        let session_id_delete = session_id.clone();
                                        let session_id_export = session_id.clone();
                                        let is_selected = move || {
                                            selected_session.get().as_deref() == Some(&session_id)
                                        };

                                        view! {
                                            <div
                                                class=format!("session-item {}", if is_selected() { "selected" } else { "" })
                                                on:click=move |_| load_session_click(session_id_load.clone())
                                            >
                                                <div class="session-item-header">
                                                    <h4 class="session-title">
                                                        {session.title.clone().unwrap_or_else(|| "Untitled".to_string())}
                                                    </h4>
                                                    <span class="session-date">
                                                        {session.updated_at.clone()}
                                                    </span>
                                                </div>
                                                <div class="session-meta">
                                                    <span class="session-provider">
                                                        {session.provider.clone().unwrap_or_default()}
                                                    </span>
                                                    <span class="session-count">
                                                        {format!("{} messages", session.message_count)}
                                                    </span>
                                                </div>
                                                {session.preview.clone().map(|p| view! {
                                                    <div class="session-preview">{p}</div>
                                                })}
                                                <div class="session-actions">
                                                    <Button
                                                        variant=ButtonVariant::Ghost
                                                        class="session-action-btn".to_string()
                                                        on_click=move |_| {
                                                            export_session_click(session_id_export.clone())
                                                        }
                                                    >
                                                        "Export"
                                                    </Button>
                                                    <Button
                                                        variant=ButtonVariant::Danger
                                                        class="session-action-btn".to_string()
                                                        on_click=move |ev| {
                                                            ev.stop_propagation();
                                                            delete_session_click(session_id_delete.clone())
                                                        }
                                                    >
                                                        "Delete"
                                                    </Button>
                                                </div>
                                            </div>
                                        }
                                    }
                                />
                            }.into_view()
                        }
                    }}
                </div>
            </div>
            <div class="session-preview-pane">
                {move || {
                    if let Some(data) = session_data.get() {
                        view! {
                            <div class="preview-content">
                                <div class="preview-header">
                                    <h3>
                                        {data.session.title.clone().unwrap_or_else(|| "Untitled".to_string())}
                                    </h3>
                                    <div class="preview-meta">
                                        <span>"Provider: "{data.session.provider.clone().unwrap_or_default()}</span>
                                        <span>"Model: "{data.session.model.clone().unwrap_or_default()}</span>
                                        <span>"Messages: "{data.session.message_count}</span>
                                    </div>
                                </div>
                                <div class="preview-messages">
                                    <For
                                        each=move || data.messages.clone()
                                        key=|msg| msg.id.clone()
                                        children=|msg| {
                                            let role_class = match msg.role.as_str() {
                                                "user" => "preview-message-user",
                                                "assistant" => "preview-message-assistant",
                                                _ => "preview-message-system",
                                            };
                                            view! {
                                                <div class=format!("preview-message {}", role_class)>
                                                    <div class="preview-message-header">
                                                        <span class="preview-message-role">
                                                            {msg.role.clone()}
                                                        </span>
                                                        <span class="preview-message-time">
                                                            {msg.timestamp.clone()}
                                                        </span>
                                                    </div>
                                                    <div class="preview-message-content">
                                                        {msg.content.clone()}
                                                    </div>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="preview-placeholder">
                                "Select a session to preview"
                            </div>
                        }.into_view()
                    }
                }}
            </div>
            <Modal
                title="Delete Session"
                show=show_delete_modal.get()
                on_close=move || {
                    set_show_delete_modal.set(false);
                    set_session_to_delete.set(None);
                }
            >
                <div class="delete-confirmation">
                    <p>"Are you sure you want to delete this session? This action cannot be undone."</p>
                    <div class="modal-actions">
                        <Button
                            variant=ButtonVariant::Secondary
                            on_click=move |_| {
                                set_show_delete_modal.set(false);
                                set_session_to_delete.set(None);
                            }
                        >
                            "Cancel"
                        </Button>
                        <Button
                            variant=ButtonVariant::Danger
                            on_click=confirm_delete
                        >
                            "Delete"
                        </Button>
                    </div>
                </div>
            </Modal>
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
