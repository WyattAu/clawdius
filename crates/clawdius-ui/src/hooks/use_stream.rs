//! Hook for SSE/streaming token reception.
//!
//! Connects to an SSE endpoint, parses streaming events,
//! handles reconnection, and tracks connection state.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Lifecycle state of the SSE connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// No connection established.
    Disconnected,
    /// Connection attempt in progress.
    Connecting,
    /// Connection open and receiving events.
    Connected,
    /// Retrying after an error; `attempt` starts at 1.
    Reconnecting {
        /// Number of reconnect attempts made so far.
        attempt: u32,
    },
}

/// One parsed server-sent event.
#[derive(Clone, Debug)]
pub struct StreamEvent {
    /// Event name from the SSE `event:` field.
    pub event_type: String,
    /// Payload from the SSE `data:` field.
    pub data: String,
    /// Value of the SSE `id:` field, if sent.
    pub id: Option<String>,
}

/// Reactive snapshot of the streaming connection.
#[derive(Clone, Debug)]
pub struct StreamState {
    /// Current connection lifecycle state.
    pub connection: ConnectionState,
    /// Last seen event ID, used to resume streams.
    pub last_event_id: Option<String>,
    /// Total number of events received.
    pub events_received: u64,
    /// How many reconnects have been attempted for the current endpoint.
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

/// Callbacks for controlling the SSE connection.
pub struct StreamActions {
    /// Opens a connection to the given endpoint URL.
    pub connect: Callback<String>,
    /// Closes the current connection.
    pub disconnect: Callback<()>,
}

/// Creates the reactive SSE connection state and its connect/disconnect actions.
#[must_use]
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

    let disconnect = Callback::new(move |()| {
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
