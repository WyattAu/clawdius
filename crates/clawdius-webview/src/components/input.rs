use leptos::*;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlInputElement, HtmlTextAreaElement};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FileAttachment {
    pub name: String,
    pub content: String,
    pub mime_type: String,
}

#[component]
pub fn ChatInput(
    input: ReadSignal<String>,
    set_input: WriteSignal<String>,
    on_send: Box<dyn Fn(String)>,
    loading: ReadSignal<bool>,
) -> impl IntoView {
    let on_send = Rc::new(on_send);
    let (attachments, set_attachments) = create_signal::<Vec<FileAttachment>>(Vec::new());
    let (show_mentions, set_show_mentions) = create_signal(false);
    let (_mention_filter, set_mention_filter) = create_signal(String::new());

    let handle_input = move |ev: Event| {
        let target = ev.target().expect("Event should have a target");
        let value = target.unchecked_into::<HtmlTextAreaElement>().value();
        set_input.set(value.clone());

        if let Some(at_pos) = value.rfind('@') {
            let after_at = &value[at_pos + 1..];
            if !after_at.contains(' ') && !after_at.contains('\n') {
                set_mention_filter.set(after_at.to_string());
                set_show_mentions.set(true);
            } else {
                set_show_mentions.set(false);
            }
        } else {
            set_show_mentions.set(false);
        }
    };

    let on_send_clone = Rc::clone(&on_send);
    let handle_send = move |_: ev::MouseEvent| {
        let msg = input.get();
        if !msg.trim().is_empty() {
            let mut full_message = msg.clone();

            let atts = attachments.get();
            if !atts.is_empty() {
                full_message.push_str("\n\n---\n**Attachments:**\n");
                for att in atts.iter() {
                    full_message.push_str(&format!("- {} ({})\n", att.name, att.mime_type));
                }
                set_attachments.set(Vec::new());
            }

            on_send_clone(full_message);
            set_input.set(String::new());
        }
    };

    let on_send_clone2 = Rc::clone(&on_send);
    let handle_keypress = move |ev: ev::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            let msg = input.get();
            if !msg.trim().is_empty() {
                let mut full_message = msg.clone();

                let atts = attachments.get();
                if !atts.is_empty() {
                    full_message.push_str("\n\n---\n**Attachments:**\n");
                    for att in atts.iter() {
                        full_message.push_str(&format!("- {} ({})\n", att.name, att.mime_type));
                    }
                    set_attachments.set(Vec::new());
                }

                on_send_clone2(full_message);
                set_input.set(String::new());
            }
        }
    };

    let handle_file_upload = move |ev: Event| {
        let target = ev.target().expect("Event should have a target");
        let input = target.unchecked_into::<HtmlInputElement>();

        if let Some(files) = input.files() {
            for i in 0..files.length() {
                if let Some(file) = files.item(i) {
                    let name = file.name();
                    let mime_type = file.type_();

                    let set_attachments_clone = set_attachments;
                    wasm_bindgen_futures::spawn_local(async move {
                        let array_buffer =
                            wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                                .await
                                .unwrap();
                        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                        let content = base64_encode(&uint8_array.to_vec());

                        set_attachments_clone.update(|atts| {
                            atts.push(FileAttachment {
                                name,
                                content,
                                mime_type,
                            });
                        });
                    });
                }
            }
        }

        input.set_value("");
    };

    view! {
        <div class="chat-input-container">
            {move || {
                let atts = attachments.get();
                if !atts.is_empty() {
                    view! {
                        <div class="attachments-list">
                            {atts.iter().map(|att| {
                                let name = att.name.clone();
                                view! {
                                    <div class="attachment-chip">
                                        <span class="attachment-icon">"📄"</span>
                                        <span class="attachment-name">{name}</span>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_view()
                } else {
                    view! { <div/> }.into_view()
                }
            }}
            <div class="input-row">
                <label class="file-upload-button" title="Attach file">
                    "📎"
                    <input
                        type="file"
                        class="file-input"
                        multiple=true
                        on:change=handle_file_upload
                    />
                </label>
                <textarea
                    class="chat-input chat-textarea"
                    placeholder="Type a message... (Shift+Enter for new line)"
                    on:input=handle_input
                    on:keypress=handle_keypress
                    prop:value=input
                    disabled=loading
                    rows="1"
                />
                <button
                    class="send-button"
                    on:click=handle_send
                    disabled=move || loading.get() || input.get().trim().is_empty()
                >
                    {move || if loading.get() { "⏳" } else { "➤" }}
                </button>
            </div>
            {move || {
                if show_mentions.get() {
                    view! {
                        <div class="mentions-dropdown">
                            <div class="mention-item" on:click=move |_| {
                                let current = input.get();
                                if let Some(at_pos) = current.rfind('@') {
                                    let before = &current[..at_pos + 1];
                                    set_input.set(format!("{}file ", before));
                                }
                                set_show_mentions.set(false);
                            }>
                                <span class="mention-icon">"📁"</span>
                                <span>"@file - Attach file"</span>
                            </div>
                            <div class="mention-item" on:click=move |_| {
                                let current = input.get();
                                if let Some(at_pos) = current.rfind('@') {
                                    let before = &current[..at_pos + 1];
                                    set_input.set(format!("{}code ", before));
                                }
                                set_show_mentions.set(false);
                            }>
                                <span class="mention-icon">"💻"</span>
                                <span>"@code - Insert code"</span>
                            </div>
                            <div class="mention-item" on:click=move |_| {
                                let current = input.get();
                                if let Some(at_pos) = current.rfind('@') {
                                    let before = &current[..at_pos + 1];
                                    set_input.set(format!("{}session ", before));
                                }
                                set_show_mentions.set(false);
                            }>
                                <span class="mention-icon">"💬"</span>
                                <span>"@session - Load session"</span>
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! { <div/> }.into_view()
                }
            }}
        </div>
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.encode(data)
}
