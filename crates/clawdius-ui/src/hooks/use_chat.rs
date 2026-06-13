//! Hook for chat state management.
//!
//! Manages messages, streaming state, and LLM interactions.

use leptos::prelude::*;

/// Chat state managed by the hook.
pub struct ChatState {
    pub messages: Vec<super::super::components::message::ChatMessage>,
    pub is_streaming: bool,
    pub current_model: String,
    pub current_provider: String,
}

/// Provides chat state management.
pub fn use_chat() -> (ReadSignal<ChatState>, WriteSignal<ChatState>) {
    signal(ChatState {
        messages: Vec::new(),
        is_streaming: false,
        current_model: String::new(),
        current_provider: String::new(),
    })
}
