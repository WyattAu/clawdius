use clawdius_ui::components::chat_input::ChatInput;
use clawdius_ui::components::message::{ChatMessage, Message, MessageRole};
use clawdius_ui::components::status_bar::{StatusBar, StatusBarState};
use clawdius_ui::layouts::main::MainLayout;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Title};

#[component]
fn HomePage(set_page: WriteSignal<String>) -> impl IntoView {
    view! {
        <div class="home-page">
            <h1 class="hero-title">"Clawdius"</h1>
            <p class="hero-subtitle">"AI-powered coding assistant"</p>
            <button
                class="cta-button"
                on:click=move |_| set_page.set("chat".to_string())
            >
                "Start Chat"
            </button>
        </div>
    }
}

#[component]
fn ChatPage(
    messages: ReadSignal<Vec<ChatMessage>>,
    set_messages: WriteSignal<Vec<ChatMessage>>,
) -> impl IntoView {
    let on_submit = move |text: String| {
        let msg = ChatMessage {
            id: format!("msg-{}", messages.get().len()),
            role: MessageRole::User,
            content: text,
            timestamp: 0,
            model: None,
            tokens_used: None,
            is_streaming: false,
        };
        set_messages.update(|m| m.push(msg));
    };

    view! {
        <div class="chat-page">
            <div class="messages">
                {move || {
                    messages
                        .get()
                        .into_iter()
                        .map(|msg| {
                            view! {
                                <Message message=msg />
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
            <ChatInput on_submit=on_submit />
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (page, set_page) = signal("home".to_string());
    let (messages, set_messages) = signal(Vec::<ChatMessage>::new());

    let status_state = StatusBarState {
        provider: "OpenAI".to_string(),
        model: "gpt-4".to_string(),
        mode: "Code".to_string(),
        tokens_used: 0,
        tokens_limit: 128000,
        latency_ms: 0,
        is_connected: true,
        workspace: Some("clawdius".to_string()),
    };

    view! {
        <Title text="Clawdius" />
        <MainLayout
            sidebar=view! {
                <nav class="sidebar-nav">
                    <button on:click=move |_| set_page.set("home".to_string())>
                        "Home"
                    </button>
                    <button on:click=move |_| set_page.set("chat".to_string())>
                        "Chat"
                    </button>
                </nav>
            }
            content=view! {
                {move || match page.get().as_str() {
                    "chat" => view! {
                        <ChatPage messages set_messages />
                    }
                        .into_any(),
                    _ => view! {
                        <HomePage set_page />
                    }
                        .into_any(),
                }}
            }
            status_bar=view! {
                <StatusBar state=status_state />
            }
        />
    }
}
