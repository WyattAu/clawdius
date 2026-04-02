use super::input::ChatInput;
use super::message::{Message, MessageComponent, MessageRole};
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use web_sys::MessageEvent;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VSCodeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    data: serde_json::Value,
}

#[component]
pub fn ChatView() -> impl IntoView {
    let (messages, set_messages) = create_signal::<Vec<Message>>(Vec::new());
    let (input, set_input) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);

    let send_to_vscode = |msg: VSCodeMessage| {
        if let Some(window) = web_sys::window() {
            if let Ok(vscode) = js_sys::Reflect::get(
                &window,
                &wasm_bindgen::JsValue::from_str("acquireVsCodeApi"),
            ) {
                let Some(vscode_val) = js_sys::Function::from(vscode)
                    .call0(&wasm_bindgen::JsValue::NULL)
                    .ok()
                else {
                    return;
                };
                let Some(post_message_val) = js_sys::Reflect::get(
                    &vscode_val,
                    &wasm_bindgen::JsValue::from_str("postMessage"),
                )
                .ok() else {
                    return;
                };
                let post_message = js_sys::Function::from(post_message_val);
                let Ok(msg_json) = serde_json::to_string(&msg) else {
                    return;
                };
                let msg_js = wasm_bindgen::JsValue::from_str(&msg_json);
                let _ = post_message.call1(&vscode_val, &msg_js);
            }
        }
    };

    let on_send = Box::new(move |msg: String| {
        let user_message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: msg.clone(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        };

        set_messages.update(|msgs| msgs.push(user_message));
        set_loading.set(true);

        send_to_vscode(VSCodeMessage {
            msg_type: "query".to_string(),
            data: serde_json::json!({ "query": msg }),
        });
    });

    create_effect(move |_| {
        if let Some(window) = web_sys::window() {
            let callback =
                wasm_bindgen::closure::Closure::wrap(Box::new(move |event: MessageEvent| {
                    let data = event.data();
                    if data.is_string() {
                        if let Some(data_str) = data.as_string() {
                            if let Ok(msg) = serde_json::from_str::<VSCodeMessage>(&data_str) {
                                match msg.msg_type.as_str() {
                                    "response" => {
                                        if let Some(content) =
                                            msg.data.get("content").and_then(|c| c.as_str())
                                        {
                                            let assistant_message = Message {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                role: MessageRole::Assistant,
                                                content: content.to_string(),
                                                timestamp: chrono::Local::now()
                                                    .format("%H:%M:%S")
                                                    .to_string(),
                                            };
                                            set_messages
                                                .update(|msgs| msgs.push(assistant_message));
                                            set_loading.set(false);
                                        }
                                    },
                                    "error" => {
                                        set_loading.set(false);
                                        web_sys::console::error_1(
                                            &wasm_bindgen::JsValue::from_str(&format!(
                                                "Error: {:?}",
                                                msg.data
                                            )),
                                        );
                                    },
                                    _ => {},
                                }
                            }
                        }
                    }
                }) as Box<dyn Fn(_)>);

            window
                .add_event_listener_with_callback("message", callback.as_ref().unchecked_ref())
                .ok();
            #[allow(unused_must_use)]
            callback.forget();
        }
    });

    view! {
        <div class="chat-container">
            <div class="messages">
                <For
                    each=move || messages.get()
                    key=|msg| msg.id.clone()
                    children=move |msg| {
                        view! { <MessageComponent message=msg.clone()/> }
                    }
                />
            </div>
            <ChatInput
                input=input
                set_input=set_input
                on_send=on_send
                loading=loading
            />
        </div>
    }
}
