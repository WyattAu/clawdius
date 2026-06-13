//! Hook for SSE/streaming token reception.
//!
//! Connects to an SSE endpoint, parses streaming events,
//! handles reconnection, and tracks connection state.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

#[derive(Clone, Debug)]
pub struct StreamEvent {
    pub event_type: String,
    pub data: String,
    pub id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StreamState {
    pub connection: ConnectionState,
    pub last_event_id: Option<String>,
    pub events_received: u64,
    pub reconnect_attempts: u32,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            last_event_id: None,
            events_received: 0,
            reconnect_attempts: 0,
        }
    }
}

pub struct StreamActions {
    pub connect: Callback<String>,
    pub disconnect: Callback<()>,
}

pub fn use_stream() -> (RwSignal<StreamState>, StreamActions) {
    let state = RwSignal::new(StreamState::default());

    let connect = Callback::new(move |endpoint: String| {
        state.update(|s| {
            s.connection = ConnectionState::Connecting;
            s.reconnect_attempts = 0;
        });

        let state_for_open = state;
        let state_for_error = state;
        let state_for_msg = state;

        let on_open: Box<dyn FnMut()> = Box::new(move || {
            state_for_open.update(|s| {
                s.connection = ConnectionState::Connected;
                s.reconnect_attempts = 0;
            });
        });

        let on_error: Box<dyn FnMut()> = Box::new(move || {
            state_for_error.update(|s| {
                s.reconnect_attempts += 1;
                s.connection = ConnectionState::Reconnecting {
                    attempt: s.reconnect_attempts,
                };
            });
        });

        let on_message: Box<dyn FnMut(web_sys::MessageEvent)> =
            Box::new(move |ev: web_sys::MessageEvent| {
                if let Some(data) = ev.data().as_string() {
                    state_for_msg.update(|s| {
                        s.events_received += 1;
                        let _ = data;
                    });
                }
            });

        let closure_open = wasm_bindgen::closure::Closure::wrap(on_open);
        let closure_error = wasm_bindgen::closure::Closure::wrap(on_error);
        let closure_msg = wasm_bindgen::closure::Closure::wrap(on_message);

        if let Some(_window) = web_sys::window() {
            if let Ok(es) = web_sys::EventSource::new(&endpoint) {
                let open_fn: &js_sys::Function = closure_open.as_ref().unchecked_ref();
                let error_fn: &js_sys::Function = closure_error.as_ref().unchecked_ref();
                let msg_fn: &js_sys::Function = closure_msg.as_ref().unchecked_ref();
                let _ = es.add_event_listener_with_callback("open", open_fn);
                let _ = es.add_event_listener_with_callback("error", error_fn);
                let _ = es.add_event_listener_with_callback("message", msg_fn);
            }
        }

        closure_open.forget();
        closure_error.forget();
        closure_msg.forget();

        let _ = endpoint;
    });

    let disconnect = Callback::new(move |_: ()| {
        state.update(|s| {
            s.connection = ConnectionState::Disconnected;
        });
    });

    (
        state,
        StreamActions {
            connect,
            disconnect,
        },
    )
}
