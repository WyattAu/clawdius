use clawdius_ui::components::chat_input::ChatInput;
use clawdius_ui::components::message::{ChatMessage, Message, MessageRole};
use clawdius_ui::components::session_list::{SessionList, SessionSummary};
use clawdius_ui::components::status_bar::{ConnectionStatus, StatusBar, StatusBarState};
use clawdius_ui::layouts::main::MainLayout;
use clawdius_ui::theme::colors;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div
            class="home-page"
            style:display="flex"
            style:flex-direction="column"
            style:align-items="center"
            style:justify-content="center"
            style:height="100%"
            style:gap="24px"
            style:background-color=colors::BG_PRIMARY
            style:color=colors::TEXT_PRIMARY
            style:font-family="Inter, sans-serif"
        >
            <h1
                style:font-size="3rem"
                style:font-weight="700"
                style:color=colors::ACCENT
                style:letter-spacing="-0.02em"
            >
                "Clawdius"
            </h1>
            <p style:font-size="1.25rem" style:color=colors::TEXT_SECONDARY>
                "High-assurance AI coding assistant"
            </p>
            <div style:display="flex" style:gap="12px" style:margin-top="16px">
                <span style:color=colors::TEXT_MUTED style:font-size="0.875rem">
                    "318 Lean4 theorems"
                </span>
                <span style:color=colors::BORDER>"|"</span>
                <span style:color=colors::TEXT_MUTED style:font-size="0.875rem">
                    "2,606 tests"
                </span>
                <span style:color=colors::BORDER>"|"</span>
                <span style:color=colors::TEXT_MUTED style:font-size="0.875rem">
                    "10+ LLM providers"
                </span>
            </div>
        </div>
    }
}

#[component]
fn ChatPage(messages: RwSignal<Vec<ChatMessage>>, is_streaming: RwSignal<bool>) -> impl IntoView {
    let on_submit = move |text: String| {
        let user_msg = ChatMessage {
            id: format!("msg-{}", messages.get().len()),
            role: MessageRole::User,
            content: text,
            timestamp: 0,
            model: None,
            tokens_used: None,
            is_streaming: false,
        };
        let assistant_msg = ChatMessage {
            id: format!("msg-{}", messages.get().len() + 1),
            role: MessageRole::Assistant,
            content: String::new(),
            timestamp: 0,
            model: Some("claude-sonnet-4".to_string()),
            tokens_used: None,
            is_streaming: true,
        };
        messages.update(|m| {
            m.push(user_msg);
            m.push(assistant_msg);
        });
        is_streaming.set(true);
    };

    let sidebar_sessions = RwSignal::new(vec![SessionSummary {
        id: "session-1".to_string(),
        title: "New Chat".to_string(),
        created_at: 0,
        updated_at: 0,
        message_count: 0,
        provider: "anthropic".to_string(),
        model: "claude-sonnet-4".to_string(),
        preview: None,
    }]);

    let sidebar = move || {
        view! {
            <SessionList
                sessions=sidebar_sessions.get()
                active_id=None
                on_select=None
                on_delete=None
            />
        }
    };

    let content = move || {
        view! {
            <div style:display="flex" style:flex-direction="column" style:height="100%">
                <div style:flex="1" style:overflow-y="auto" style:padding="16px">
                    {move || {
                        let msgs = messages.get();
                        if msgs.is_empty() {
                            view! {
                                <div
                                    style:display="flex"
                                    style:align-items="center"
                                    style:justify-content="center"
                                    style:height="100%"
                                    style:color=colors::TEXT_MUTED
                                >
                                    "Start a conversation"
                                </div>
                            }.into_any()
                        } else {
                            msgs.into_iter()
                                .map(|msg| {
                                    view! {
                                        <div style:margin-bottom="16px">
                                            <Message message=msg />
                                        </div>
                                    }
                                })
                                .collect::<Vec<_>>()
                                .into_any()
                        }
                    }}
                </div>
                <div style:flex-shrink="0" style:padding="16px">
                    <ChatInput
                        on_submit=on_submit
                        disabled=is_streaming.get()
                        token_count=0
                        token_limit=200000
                    />
                </div>
            </div>
        }
    };

    let status_state = StatusBarState {
        provider: "Anthropic".to_string(),
        model: "claude-sonnet-4".to_string(),
        mode: "chat".to_string(),
        tokens_used: 0,
        tokens_limit: 200000,
        latency_ms: 0,
        is_connected: true,
        connection_status: ConnectionStatus::Connected,
        workspace: Some("clawdius".to_string()),
    };

    let status_bar = move || {
        view! {
            <StatusBar state=status_state.clone() />
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

#[derive(Clone, Copy, PartialEq)]
enum WebPage {
    Home,
    Chat,
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let current_page = RwSignal::new(WebPage::Home);
    let messages = RwSignal::new(Vec::<ChatMessage>::new());
    let is_streaming = RwSignal::new(false);

    let nav_item = move |page: WebPage, label: &'static str| {
        let is_active = current_page.get() == page;
        let bg = if is_active {
            colors::BG_ELEVATED
        } else {
            "transparent"
        };
        let color = if is_active {
            colors::ACCENT
        } else {
            colors::TEXT_SECONDARY
        };
        view! {
            <button
                style:background-color=bg
                style:color=color
                style:border="none"
                style:cursor="pointer"
                style:padding="8px 16px"
                style:border-radius="4px"
                style:font-size="0.875rem"
                style:font-family="Inter, sans-serif"
                on:click=move |_| current_page.set(page)
            >
                {label}
            </button>
        }
    };

    view! {
        <Title text="Clawdius" />
        <div
            style:width="100%"
            style:height="100vh"
            style:display="flex"
            style:flex-direction="column"
            style:background-color=colors::BG_PRIMARY
        >
            <nav
                style:display="flex"
                style:gap="4px"
                style:padding="8px 16px"
                style:background-color=colors::BG_SECONDARY
                style:border-bottom={format!("1px solid {}", colors::BORDER)}
            >
                {nav_item(WebPage::Home, "Home")}
                {nav_item(WebPage::Chat, "Chat")}
            </nav>
            <main style:flex="1" style:overflow="hidden">
                {move || match current_page.get() {
                    WebPage::Home => view! { <HomePage /> }.into_any(),
                    WebPage::Chat => view! {
                        <ChatPage messages is_streaming />
                    }.into_any(),
                }}
            </main>
        </div>
    }
}
