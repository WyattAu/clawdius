//! Status bar component.
//!
//! Displays current provider, model, mode, token usage,
//! and connection status at the bottom of the application.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Status bar state.
#[derive(Clone, Debug)]
pub struct StatusBarState {
    pub provider: String,
    pub model: String,
    pub mode: String,
    pub tokens_used: u32,
    pub tokens_limit: u32,
    pub latency_ms: u32,
    pub is_connected: bool,
    pub workspace: Option<String>,
}

/// Renders the application status bar.
#[component]
pub fn StatusBar(
    /// Current status state.
    #[prop(into)]
    state: StatusBarState,
) -> impl IntoView {
    let connection_indicator = if state.is_connected { "O" } else { "X" };

    view! {
        <div class="status-bar">
            <div class="status-left">
                <span class="status-indicator">{connection_indicator}</span>
                <span class="status-provider">{state.provider}</span>
                <span class="status-divider">" | "</span>
                <span class="status-model">{state.model}</span>
                <span class="status-divider">" | "</span>
                <span class="status-mode">{state.mode}</span>
            </div>
            <div class="status-right">
                <span class="status-tokens">
                    {format!("{}/{} tokens", state.tokens_used, state.tokens_limit)}
                </span>
                <span class="status-divider">" | "</span>
                <span class="status-latency">
                    {format!("{}ms", state.latency_ms)}
                </span>
                {state.workspace.map(|w| view! {
                    <>
                        <span class="status-divider">" | "</span>
                        <span class="status-workspace">{w}</span>
                    </>
                })}
            </div>
        </div>
    }
}
