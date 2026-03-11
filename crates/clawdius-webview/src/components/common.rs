use leptos::*;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[component]
pub fn Button<F>(
    children: Children,
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] on_click: Option<F>,
    #[prop(optional)] class: String,
) -> impl IntoView
where
    F: Fn(leptos::ev::MouseEvent) + 'static,
{
    let variant_class = match variant {
        ButtonVariant::Primary => "btn-primary",
        ButtonVariant::Secondary => "btn-secondary",
        ButtonVariant::Danger => "btn-danger",
        ButtonVariant::Ghost => "btn-ghost",
    };

    view! {
        <button
            class=format!("btn {} {}", variant_class, class)
            disabled=disabled
            on:click=move |ev| {
                if let Some(handler) = &on_click {
                    handler(ev);
                }
            }
        >
            {children()}
        </button>
    }
}

#[component]
pub fn Input<F1, F2>(
    #[prop(into)] value: String,
    #[prop(optional)] placeholder: String,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] error: Option<String>,
    #[prop(optional)] on_input: Option<F1>,
    #[prop(optional)] on_keypress: Option<F2>,
    #[prop(optional)] class: String,
) -> impl IntoView
where
    F1: Fn(leptos::ev::Event) + 'static,
    F2: Fn(leptos::ev::KeyboardEvent) + 'static,
{
    view! {
        <div class=format!("input-wrapper {}", class)>
            <input
                type="text"
                class=format!("input {}", if error.is_some() { "input-error" } else { "" })
                placeholder=placeholder
                prop:value=value
                disabled=disabled
                on:input=move |ev| {
                    if let Some(handler) = &on_input {
                        handler(ev);
                    }
                }
                on:keypress=move |ev| {
                    if let Some(handler) = &on_keypress {
                        handler(ev);
                    }
                }
            />
            {move || error.as_ref().map(|e| view! {
                <span class="input-error-text">{e.clone()}</span>
            })}
        </div>
    }
}

#[component]
pub fn Modal<F>(
    #[prop(into)] title: String,
    show: bool,
    on_close: F,
    children: Children,
) -> impl IntoView
where
    F: Fn() + 'static + Clone,
{
    let on_close_overlay = on_close.clone();
    let on_close_button = on_close.clone();
    let title_clone = title.clone();

    if !show {
        return view! { <div/> }.into_view();
    }

    view! {
        <div class="modal-overlay" on:click=move |_| {
            on_close_overlay();
        }>
            <div class="modal" on:click=|ev| ev.stop_propagation()>
                <div class="modal-header">
                    <h3>{title_clone}</h3>
                    <button class="modal-close" on:click=move |_| {
                        on_close_button();
                    }>
                        "×"
                    </button>
                </div>
                <div class="modal-content">
                    {children()}
                </div>
            </div>
        </div>
    }
    .into_view()
}

#[derive(Clone, Debug)]
pub struct ToastMessage {
    pub id: String,
    pub message: String,
    pub toast_type: ToastType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ToastType {
    Success,
    Error,
    Warning,
    Info,
}

#[component]
pub fn Toast(
    toasts: ReadSignal<Vec<ToastMessage>>,
    set_toasts: WriteSignal<Vec<ToastMessage>>,
) -> impl IntoView {
    view! {
        <div class="toast-container">
            <For
                each=move || toasts.get()
                key=|toast| toast.id.clone()
                children=move |toast| {
                    let toast_id = toast.id.clone();
                    let toast_class = match toast.toast_type {
                        ToastType::Success => "toast-success",
                        ToastType::Error => "toast-error",
                        ToastType::Warning => "toast-warning",
                        ToastType::Info => "toast-info",
                    };

                    view! {
                        <div class=format!("toast {}", toast_class)>
                            <span>{toast.message.clone()}</span>
                            <button
                                class="toast-close"
                                on:click=move |_| {
                                    set_toasts.update(|t| {
                                        t.retain(|t| t.id != toast_id);
                                    });
                                }
                            >
                                "×"
                            </button>
                        </div>
                    }
                }
            />
        </div>
    }
}

#[component]
pub fn LoadingSpinner() -> impl IntoView {
    view! {
        <div class="loading-spinner">
            <div class="spinner"></div>
        </div>
    }
}

#[component]
pub fn Dropdown<F>(
    #[prop(into)] value: String,
    options: Vec<(String, String)>,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] on_change: Option<F>,
    #[prop(optional)] class: String,
) -> impl IntoView
where
    F: Fn(leptos::ev::Event) + 'static,
{
    view! {
        <select
            class=format!("dropdown {}", class)
            disabled=disabled
            on:change=move |ev| {
                if let Some(handler) = &on_change {
                    handler(ev);
                }
            }
        >
            {options.iter().map(|(label, val)| {
                let selected = val == &value;
                view! {
                    <option value=val selected=selected>
                        {label.clone()}
                    </option>
                }
            }).collect::<Vec<_>>()}
        </select>
    }
}

#[component]
pub fn SearchInput<F>(
    #[prop(into)] value: String,
    #[prop(optional)] placeholder: String,
    #[prop(optional)] on_input: Option<F>,
) -> impl IntoView
where
    F: Fn(leptos::ev::Event) + 'static,
{
    view! {
        <div class="search-input-wrapper">
            <span class="search-icon">"🔍"</span>
            <input
                type="text"
                class="search-input"
                placeholder=placeholder
                prop:value=value
                on:input=move |ev| {
                    if let Some(handler) = &on_input {
                        handler(ev);
                    }
                }
            />
        </div>
    }
}
