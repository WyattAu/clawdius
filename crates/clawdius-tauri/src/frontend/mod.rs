pub mod app;
pub mod pages;

pub use app::App;

use leptos::prelude::*;

pub fn mount_app() {
    mount_to_body(|| view! { <App /> });
}
