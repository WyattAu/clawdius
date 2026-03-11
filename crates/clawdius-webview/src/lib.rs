//! Clawdius WebView - Leptos-based WASM UI
//!
//! This crate provides the webview UI for the VSCode extension.

#![deny(unsafe_code)]
#![allow(missing_docs)]

mod components;

#[cfg(test)]
mod tests;

use components::{ChatView, HistoryView, SettingsView, Sidebar, SidebarTab};
use leptos::*;

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    let (active_tab, set_active_tab) = create_signal(SidebarTab::Chat);

    view! {
        <div class="app">
            <Sidebar
                active_tab=active_tab
                set_active_tab=set_active_tab
            />
            <main class="main-content">
                {move || match active_tab.get() {
                    SidebarTab::Chat => view! { <ChatView/> }.into_view(),
                    SidebarTab::History => view! { <HistoryView/> }.into_view(),
                    SidebarTab::Settings => view! { <SettingsView/> }.into_view(),
                }}
            </main>
            <style>
                {include_str!("../styles.css")}
            </style>
        </div>
    }
}

/// Initialize the webview
pub fn init() {
    console_error_panic_hook::set_once();

    mount_to_body(|| view! { <App/> });
}
