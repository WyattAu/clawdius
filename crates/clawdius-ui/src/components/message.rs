//! Chat message display component.
//!
//! Renders a single chat message with role-based styling,
//! markdown rendering, and syntax-highlighted code blocks.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Role of the message sender.
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A single chat message.
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

/// Renders a chat message with role-appropriate styling.
#[component]
pub fn Message(
    /// The message to render.
    #[prop(into)]
    message: ChatMessage,
) -> impl IntoView {
    let role_class = match &message.role {
        MessageRole::User => "message-user",
        MessageRole::Assistant => "message-assistant",
        MessageRole::System => "message-system",
        MessageRole::Tool => "message-tool",
    };

    let role_label = match &message.role {
        MessageRole::User => "You".to_string(),
        MessageRole::Assistant => message
            .model
            .clone()
            .unwrap_or_else(|| "Assistant".to_string()),
        MessageRole::System => "System".to_string(),
        MessageRole::Tool => "Tool".to_string(),
    };

    view! {
        <div class=format!("message {role_class}")>
            <div class="message-header">
                <span class="message-role">{role_label}</span>
                {message.tokens_used.map(|t| view! {
                    <span class="message-tokens">{format!("{t} tokens")}</span>
                })}
            </div>
            <div class="message-content">
                // TODO: integrate markdown renderer + code block syntax highlighting
                {message.content.clone()}
            </div>
            {message.is_streaming.then(|| view! {
                <span class="message-cursor">"..."</span>
            })}
        </div>
    }
}
