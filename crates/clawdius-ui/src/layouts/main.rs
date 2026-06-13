//! Main application layout.
//!
//! Three-column layout: sidebar (sessions/files) | chat area | status bar.
//! Responsive: collapses to single-column on mobile.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Main application layout with sidebar, chat, and status bar.
#[component]
pub fn MainLayout(
    /// Sidebar content (sessions, file tree).
    sidebar: impl IntoView,
    /// Main content area (chat messages).
    content: impl IntoView,
    /// Bottom status bar.
    status_bar: impl IntoView,
) -> impl IntoView {
    view! {
        <div class="main-layout">
            <div class="sidebar">
                {sidebar}
            </div>
            <div class="main-content">
                <div class="content-area">
                    {content}
                </div>
                <div class="status-area">
                    {status_bar}
                </div>
            </div>
        </div>
    }
}
