//! Chat message display component.
//!
//! Renders a single chat message with role-based styling,
//! markdown rendering, and syntax-highlighted code blocks.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: i64,
    pub model: Option<String>,
    pub tokens_used: Option<u32>,
    pub is_streaming: bool,
}

enum MarkdownSegment {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
}

fn render_simplified_markdown(content: &str) -> Vec<MarkdownSegment> {
    let mut segments = Vec::new();
    let mut remaining = content;
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                segments.push(MarkdownSegment::Text(remaining[..pos].to_string()));
            }
            let after = &remaining[pos + 2..];
            if let Some(end) = after.find("**") {
                segments.push(MarkdownSegment::Bold(after[..end].to_string()));
                remaining = &after[end + 2..];
            } else {
                segments.push(MarkdownSegment::Text(remaining[pos..].to_string()));
                break;
            }
        } else if let Some(pos) = remaining.find('`') {
            if pos > 0 {
                segments.push(MarkdownSegment::Text(remaining[..pos].to_string()));
            }
            let after = &remaining[pos + 1..];
            if let Some(end) = after.find('`') {
                segments.push(MarkdownSegment::Code(after[..end].to_string()));
                remaining = &after[end + 1..];
            } else {
                segments.push(MarkdownSegment::Text(remaining[pos..].to_string()));
                break;
            }
        } else if let Some(pos) = remaining.find('*') {
            if pos > 0 {
                segments.push(MarkdownSegment::Text(remaining[..pos].to_string()));
            }
            let after = &remaining[pos + 1..];
            if let Some(end) = after.find('*') {
                segments.push(MarkdownSegment::Italic(after[..end].to_string()));
                remaining = &after[end + 1..];
            } else {
                segments.push(MarkdownSegment::Text(remaining[pos..].to_string()));
                break;
            }
        } else {
            segments.push(MarkdownSegment::Text(remaining.to_string()));
            break;
        }
    }
    segments
}

fn format_timestamp(ts: i64) -> String {
    let secs = ts / 1000;
    let hours = ((secs % 86400) / 3600) as u32;
    let minutes = ((secs % 3600) / 60) as u32;
    format!("{hours:02}:{minutes:02}")
}

fn role_bg(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => colors::USER_MSG_BG,
        MessageRole::Assistant => colors::ASSISTANT_MSG_BG,
        MessageRole::System => colors::BG_SURFACE,
        MessageRole::Tool => colors::BG_ELEVATED,
    }
}

fn role_accent(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => colors::ACCENT,
        MessageRole::Assistant => colors::TEXT_PRIMARY,
        MessageRole::System => colors::WARNING,
        MessageRole::Tool => colors::TEXT_SECONDARY,
    }
}

#[component]
pub fn Message(#[prop(into)] message: ChatMessage) -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let role_label = match &message.role {
        MessageRole::User => "You".to_string(),
        MessageRole::Assistant => message
            .model
            .clone()
            .unwrap_or_else(|| "Assistant".to_string()),
        MessageRole::System => "System".to_string(),
        MessageRole::Tool => "Tool".to_string(),
    };
    let role_class = match &message.role {
        MessageRole::User => "message-user",
        MessageRole::Assistant => "message-assistant",
        MessageRole::System => "message-system",
        MessageRole::Tool => "message-tool",
    };
    let bg = role_bg(&message.role);
    let accent = role_accent(&message.role);
    let ts_display = format_timestamp(message.timestamp);
    let segments = render_simplified_markdown(&message.content);
    let content_copy = message.content.clone();
    let aria_label = format!("{role_label} message");

    view! {
        <div
            class=format!("message {role_class}")
            role="article"
            aria-label=aria_label
            style:background-color=bg
            style:border-left=format!("3px solid {accent}")
            style:padding=spacing::SPACE_16
            style:border-radius=radius::LG
            style:margin-bottom=spacing::SPACE_12
        >
            <div class="message-header" style:display="flex" style:justify-content="space-between" style:align-items="center" style:margin-bottom=spacing::SPACE_8>
                <span
                    class="message-role"
                    style:color=accent
                    style:font-family=typography::FONT_DISPLAY
                    style:font-size=typography::SIZE_SM
                    style:font-weight=typography::WEIGHT_SEMIBOLD
                    style:text-transform="uppercase"
                    style:letter-spacing="0.05em"
                >
                    {role_label}
                </span>
                <div class="message-meta" style:display="flex" style:gap=spacing::SPACE_12 style:align-items="center">
                    {message.tokens_used.map(|t| view! {
                        <span
                            class="message-tokens"
                            style:color=colors::TEXT_MUTED
                            style:font-size=typography::SIZE_XS
                            style:font-family=typography::FONT_MONO
                        >
                            {format!("{t} tok")}
                        </span>
                    })}
                    <span
                        class="message-time"
                        style:color=colors::TEXT_MUTED
                        style:font-size=typography::SIZE_XS
                        style:font-family=typography::FONT_MONO
                    >
                        {ts_display}
                    </span>
                    <button
                        class="message-copy"
                        style:background="transparent"
                        style:border="none"
                        style:color=colors::TEXT_SECONDARY
                        style:cursor="pointer"
                        style:font-size=typography::SIZE_XS
                        style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                        style:border-radius=radius::SM
                        title="Copy message"
                        on:click=move |_| {
                            set_copied.set(true);
                            copy_to_clipboard(&content_copy);
                            schedule_timeout(move || set_copied.set(false), 1500);
                        }
                    >
                        {move || if copied.get() { "Copied!" } else { "Copy" }}
                    </button>
                </div>
            </div>
            <div
                class="message-content"
                style:color=colors::TEXT_PRIMARY
                style:font-size=typography::SIZE_BASE
                style:line-height=typography::LINE_HEIGHT_RELAXED
                style:font-family=typography::FONT_SANS
                style:white-space="pre-wrap"
                style:word-break="break-word"
            >
                {segments.into_iter().map(|seg| match seg {
                    MarkdownSegment::Text(t) => view! { <span>{t}</span> }.into_any(),
                    MarkdownSegment::Bold(t) => view! {
                        <strong style:font-weight=typography::WEIGHT_BOLD>{t}</strong>
                    }.into_any(),
                    MarkdownSegment::Italic(t) => view! {
                        <em>{t}</em>
                    }.into_any(),
                    MarkdownSegment::Code(t) => view! {
                        <code
                            style:background-color=colors::CODE_BG
                            style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                            style:border-radius=radius::SM
                            style:font-family=typography::FONT_MONO
                            style:font-size=typography::SIZE_SM
                            style:color=colors::ACCENT
                        >
                            {t}
                        </code>
                    }.into_any(),
                }).collect::<Vec<_>>()}
            </div>
            {message.is_streaming.then(|| view! {
                <span
                    class="message-cursor"
                    style:display="inline-block"
                    style:width="8px"
                    style:height="16px"
                    style:background-color=accent
                    style:margin-left=spacing::SPACE_4
                    style:animation="blink 1s step-end infinite"
                    aria-label="Typing"
                />
            })}
        </div>
    }
}

fn copy_to_clipboard(text: &str) {
    if let Some(w) = web_sys::window() {
        let nav = w.navigator();
        let clipboard = nav.clipboard();
        let _ = clipboard.write_text(text);
    }
}

fn schedule_timeout(f: impl FnOnce() + 'static, ms: i32) {
    if let Some(w) = web_sys::window() {
        let closure = wasm_bindgen::closure::Closure::once_into_js(f);
        let func: &js_sys::Function = closure.unchecked_ref();
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(func, ms);
    }
}
