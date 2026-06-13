//! Hook for chat state management.
//!
//! Manages messages, streaming state, token usage, and LLM interactions.

use crate::components::message::{ChatMessage, MessageRole};
use leptos::prelude::*;

#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub name: String,
    pub arguments: String,
    pub result: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub is_streaming: bool,
    pub current_model: String,
    pub current_provider: String,
    pub token_usage: TokenUsage,
    pub streaming_message_id: Option<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            is_streaming: false,
            current_model: String::from("default"),
            current_provider: String::from("default"),
            token_usage: TokenUsage::default(),
            streaming_message_id: None,
        }
    }
}

pub struct ChatActions {
    pub send_message: Callback<String>,
    pub cancel_streaming: Callback<()>,
    pub clear_history: Callback<()>,
    pub load_session: Callback<String>,
}

pub fn use_chat() -> (RwSignal<ChatState>, ChatActions) {
    let state = RwSignal::new(ChatState::default());

    let send_message = Callback::new(move |content: String| {
        let user_msg = ChatMessage {
            id: format!("msg-{}", uuid_part()),
            role: MessageRole::User,
            content,
            timestamp: now_millis(),
            model: None,
            tokens_used: None,
            is_streaming: false,
        };

        let assistant_id = format!("msg-{}", uuid_part());
        let assistant_msg = ChatMessage {
            id: assistant_id.clone(),
            role: MessageRole::Assistant,
            content: String::new(),
            timestamp: now_millis(),
            model: Some(state.get().current_model.clone()),
            tokens_used: None,
            is_streaming: true,
        };

        state.update(|s| {
            s.messages.push(user_msg);
            s.messages.push(assistant_msg);
            s.is_streaming = true;
            s.streaming_message_id = Some(assistant_id);
        });

        simulate_stream_response(state);
    });

    let cancel_streaming = Callback::new(move |_: ()| {
        state.update(|s| {
            s.is_streaming = false;
            s.streaming_message_id = None;
            for msg in &mut s.messages {
                msg.is_streaming = false;
            }
        });
    });

    let clear_history = Callback::new(move |_: ()| {
        state.update(|s| {
            s.messages.clear();
            s.token_usage = TokenUsage::default();
            s.streaming_message_id = None;
        });
    });

    let load_session = Callback::new(move |session_id: String| {
        state.update(|s| {
            s.messages.clear();
            s.token_usage = TokenUsage::default();
            s.streaming_message_id = None;
            let _ = session_id;
        });
    });

    (
        state,
        ChatActions {
            send_message,
            cancel_streaming,
            clear_history,
            load_session,
        },
    )
}

fn now_millis() -> i64 {
    js_sys::Date::now() as i64
}

fn uuid_part() -> String {
    let bytes = js_sys::Math::random().to_string();
    let trimmed = bytes.trim_start_matches("0.");
    trimmed[..trimmed.len().min(8)].to_string()
}

fn simulate_stream_response(state: RwSignal<ChatState>) {
    let state_clone = state;
    let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
        state_clone.update(|s| {
            s.is_streaming = false;
            s.streaming_message_id = None;
            if let Some(last) = s.messages.last_mut() {
                if last.is_streaming {
                    last.content = String::from("Response received.");
                    last.is_streaming = false;
                    last.tokens_used = Some(42);
                }
            }
            s.token_usage.total_tokens += 42;
        });
    });
    if let Some(w) = web_sys::window() {
        let func: &js_sys::Function = wasm_bindgen::JsCast::unchecked_ref(&callback);
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(func, 1000);
    }
}
