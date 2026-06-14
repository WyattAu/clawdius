use leptos::prelude::*;
use leptos::{component, view, IntoView};

use clawdius_ui::components::chat_input::ChatInput;
use clawdius_ui::components::message::Message as MessageComponent;
use clawdius_ui::components::session_list::SessionList as SessionListComponent;
use clawdius_ui::components::session_list::SessionSummary as UiSessionSummary;
use clawdius_ui::components::status_bar::{ConnectionStatus, StatusBar, StatusBarState};
use clawdius_ui::hooks::use_chat::{ChatActions, ChatState};
use clawdius_ui::hooks::use_config::ConfigState;
use clawdius_ui::layouts::main::MainLayout;

#[component]
pub fn ChatPage(
    chat_state: RwSignal<ChatState>,
    chat_actions: ChatActions,
    config_state: RwSignal<ConfigState>,
) -> impl IntoView {
    let sidebar_sessions = Memo::new(move |_| {
        vec![UiSessionSummary {
            id: "demo-1".into(),
            title: "Welcome".into(),
            created_at: 0,
            updated_at: 0,
            message_count: chat_state.get().messages.len() as u32,
            provider: chat_state.get().current_provider.clone(),
            model: chat_state.get().current_model.clone(),
            preview: None,
        }]
    });

    let on_session_select = Callback::new(move |id: String| {
        chat_actions.load_session.run(id);
    });

    let sidebar = move || {
        view! {
            <SessionListComponent
                sessions=sidebar_sessions.get()
                active_id=None
                on_select=Some(on_session_select)
                on_delete=None
            />
        }
    };

    let messages_view = move || {
        let msgs = chat_state.get().messages.clone();
        msgs.into_iter()
            .map(|msg| {
                view! {
                    <MessageComponent message=msg />
                }
            })
            .collect::<Vec<_>>()
    };

    let status = Memo::new(move |_| {
        let cs = chat_state.get();
        let cfg = config_state.get();
        StatusBarState {
            provider: cfg.current_provider.clone(),
            model: cfg.current_model.clone(),
            mode: "chat".into(),
            tokens_used: cs.token_usage.total_tokens,
            tokens_limit: 200000,
            latency_ms: 0,
            is_connected: true,
            connection_status: if cs.is_streaming {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Connected
            },
            workspace: None,
        }
    });

    let content = move || {
        view! {
            <div
                class="chat-page"
                style:display="flex"
                style:flex-direction="column"
                style:height="100%"
            >
                <div
                    class="messages-area"
                    style:flex="1"
                    style:overflow-y="auto"
                    style:padding="16px"
                >
                    {messages_view()}
                </div>
                <div class="chat-input-area" style:flex-shrink="0">
                    <ChatInput
                        on_submit=move |text: String| {
                            chat_actions.send_message.run(text);
                        }
                        disabled=false
                        token_count=chat_state.get().token_usage.total_tokens
                        token_limit=200000
                    />
                </div>
            </div>
        }
    };

    let status_bar = move || {
        view! {
            <StatusBar state=status.get() />
        }
    };

    view! {
        <MainLayout
            sidebar=sidebar()
            content=content()
            status_bar=status_bar()
        />
    }
}
