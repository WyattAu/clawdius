use leptos::prelude::*;
use leptos::{component, view, IntoView};

use crate::frontend::pages::chat::ChatPage;
use crate::frontend::pages::sessions::SessionsPage;
use crate::frontend::pages::settings::SettingsPage;
use clawdius_ui::hooks::use_chat::use_chat;
use clawdius_ui::hooks::use_config::use_config;
use clawdius_ui::theme::colors;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Page {
    Chat,
    Sessions,
    Settings,
}

#[component]
pub fn App() -> impl IntoView {
    let (current_page, set_current_page) = signal(Page::Chat);
    let (chat_state, chat_actions) = use_chat();
    let (config_state, config_actions) = use_config();

    let nav_item = move |page: Page, label: &'static str| {
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
        let page_clone = page;
        view! {
            <button
                class="nav-item"
                style:background-color=bg
                style:color=color
                style:border="none"
                style:cursor="pointer"
                style:padding="8px 16px"
                style:border-radius="4px"
                style:font-size="0.875rem"
                style:font-family="Inter, sans-serif"
                style:transition="all 150ms"
                on:click=move |_| set_current_page.set(page_clone)
            >
                {label}
            </button>
        }
    };

    view! {
        <div
            class="app-root"
            style:width="100%"
            style:height="100vh"
            style:display="flex"
            style:flex-direction="column"
            style:background-color=colors::BG_PRIMARY
            style:color=colors::TEXT_PRIMARY
            style:font-family="Inter, sans-serif"
        >
            <nav
                class="app-nav"
                style:display="flex"
                style:gap="4px"
                style:padding="8px 16px"
                style:background-color=colors::BG_SECONDARY
                style:border-bottom=format!("1px solid {}", colors::BORDER)
            >
                {nav_item(Page::Chat, "Chat")}
                {nav_item(Page::Sessions, "Sessions")}
                {nav_item(Page::Settings, "Settings")}
            </nav>
            <main class="app-main" style:flex="1" style:overflow="hidden">
                {move || match current_page.get() {
                    Page::Chat => view! {
                        <ChatPage
                            chat_state=chat_state
                            chat_actions=chat_actions.clone()
                            config_state=config_state
                        />
                    }.into_any(),
                    Page::Sessions => view! {
                        <SessionsPage chat_state=chat_state />
                    }.into_any(),
                    Page::Settings => view! {
                        <SettingsPage
                            config_state=config_state
                            config_actions=config_actions.clone()
                        />
                    }.into_any(),
                }}
            </main>
        </div>
    }
}
