//! Status bar component.
//!
//! Displays current provider, model, mode, token usage,
//! connection status, and latency at the bottom of the app.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusBarState {
    pub provider: String,
    pub model: String,
    pub mode: String,
    pub tokens_used: u32,
    pub tokens_limit: u32,
    pub latency_ms: u32,
    pub is_connected: bool,
    pub connection_status: ConnectionStatus,
    pub workspace: Option<String>,
}

const fn connection_indicator(status: &ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connected => "O",
        ConnectionStatus::Disconnected => "X",
        ConnectionStatus::Reconnecting => "~",
    }
}

const fn connection_color(status: &ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connected => colors::SUCCESS,
        ConnectionStatus::Disconnected => colors::ERROR,
        ConnectionStatus::Reconnecting => colors::WARNING,
    }
}

const fn latency_color(ms: u32) -> &'static str {
    if ms < 200 {
        colors::SUCCESS
    } else if ms < 500 {
        colors::WARNING
    } else {
        colors::ERROR
    }
}

fn token_bar_pct(used: u32, limit: u32) -> u32 {
    used.checked_mul(100)
        .and_then(|v| v.checked_div(limit))
        .unwrap_or(0)
}

const fn token_bar_color(pct: u32) -> &'static str {
    if pct > 90 {
        colors::ERROR
    } else if pct > 70 {
        colors::WARNING
    } else {
        colors::ACCENT
    }
}

fn status_identity(
    provider: String,
    model: String,
    mode: String,
    conn_indicator: &'static str,
    conn_color: &'static str,
    conn_aria: String,
) -> impl IntoView {
    view! {
        <div class="status-left" style:display="flex" style:align-items="center" style:gap=spacing::SPACE_8>
            <span
                class="status-indicator"
                style:color=conn_color
                style:font-weight=typography::WEIGHT_BOLD
                aria-label=conn_aria
            >
                {conn_indicator}
            </span>
            <span
                class="status-provider"
                style:color=colors::TEXT_PRIMARY
            >
                {provider}
            </span>
            <span style:color=colors::TEXT_MUTED>/</span>
            <span
                class="status-model"
                style:color=colors::TEXT_PRIMARY
            >
                {model}
            </span>
            <span style:color=colors::TEXT_MUTED>/</span>
            <span
                class="status-mode"
                style:color=colors::ACCENT
                style:text-transform="uppercase"
                style:font-size=typography::SIZE_XS
            >
                {mode}
            </span>
        </div>
    }
}

fn token_bar(
    tokens_used: u32,
    tokens_limit: u32,
    pct: u32,
    bar_color: &'static str,
) -> impl IntoView {
    view! {
        <div
            class="status-token-bar"
            style:display="flex"
            style:align-items="center"
            style:gap=spacing::SPACE_8
            aria-label=format!("{tokens_used} of {tokens_limit} tokens used")
        >
            <div
                class="token-bar-track"
                style:width="60px"
                style:height="4px"
                style:background-color=colors::BORDER
                style:border-radius=radius::FULL
                style:overflow="hidden"
            >
                <div
                    class="token-bar-fill"
                    style:width=format!("{pct}%")
                    style:height="100%"
                    style:background-color=bar_color
                    style:border-radius=radius::FULL
                    style:transition=format!("width {}", crate::theme::transition::NORMAL)
                />
            </div>
            <span class="status-tokens" style:color=token_bar_color(pct)>
                {format!("{}/{}k", tokens_used / 1000, tokens_limit / 1000)}
            </span>
        </div>
    }
}

// leptos' #[component] macro re-emits the implementation as a `#[doc(hidden)]`
// pub fn __component_* and drops `#[must_use]` from it, so this targeted allow
// is the only way to satisfy clippy::must_use_candidate for that generated fn.
#[allow(clippy::must_use_candidate)]
#[component]
pub fn StatusBar(#[prop(into)] state: StatusBarState) -> impl IntoView {
    let StatusBarState {
        provider,
        model,
        mode,
        tokens_used,
        tokens_limit,
        latency_ms,
        is_connected: _,
        connection_status,
        workspace,
    } = state;
    let conn_indicator = connection_indicator(&connection_status);
    let conn_color = connection_color(&connection_status);
    let conn_aria = format!("Connection: {connection_status:?}");
    let lat_color = latency_color(latency_ms);
    let pct = token_bar_pct(tokens_used, tokens_limit);
    let bar_color = token_bar_color(pct);

    view! {
        <div
            class="status-bar"
            role="status"
            aria-label="Application status"
            style:display="flex"
            style:align-items="center"
            style:justify-content="space-between"
            style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_12)
            style:background-color=colors::BG_SURFACE
            style:border-top=format!("1px solid {}", colors::BORDER)
            style:font-family=typography::FONT_MONO
            style:font-size=typography::SIZE_XS
            style:color=colors::TEXT_SECONDARY
            style:flex-shrink="0"
        >
            {status_identity(provider, model, mode, conn_indicator, conn_color, conn_aria)}
            <div class="status-right" style:display="flex" style:align-items="center" style:gap=spacing::SPACE_12>
                {token_bar(tokens_used, tokens_limit, pct, bar_color)}
                <span style:color=colors::TEXT_MUTED>|</span>
                <span
                    class="status-latency"
                    style:color=lat_color
                    aria-label=format!("Latency: {latency_ms}ms")
                >
                    {format!("{latency_ms}ms")}
                </span>
                {workspace.map(|w| view! {
                    <>
                        <span style:color=colors::TEXT_MUTED>|</span>
                        <span
                            class="status-workspace"
                            style:color=colors::TEXT_SECONDARY
                            style:overflow="hidden"
                            style:text-overflow="ellipsis"
                            style:white-space="nowrap"
                            style:max-width="200px"
                        >
                            {w}
                        </span>
                    </>
                })}
            </div>
        </div>
    }
}
