use leptos::*;

#[derive(Clone, Debug, PartialEq)]
pub enum SidebarTab {
    Chat,
    History,
    Settings,
}

#[component]
pub fn Sidebar(
    active_tab: ReadSignal<SidebarTab>,
    set_active_tab: WriteSignal<SidebarTab>,
) -> impl IntoView {
    view! {
        <div class="sidebar">
            <div class="sidebar-header">
                <h2>"Clawdius"</h2>
            </div>
            <nav class="sidebar-nav">
                <button
                    class=move || if active_tab.get() == SidebarTab::Chat { "nav-item active" } else { "nav-item" }
                    on:click=move |_| set_active_tab.set(SidebarTab::Chat)
                >
                    "Chat"
                </button>
                <button
                    class=move || if active_tab.get() == SidebarTab::History { "nav-item active" } else { "nav-item" }
                    on:click=move |_| set_active_tab.set(SidebarTab::History)
                >
                    "History"
                </button>
                <button
                    class=move || if active_tab.get() == SidebarTab::Settings { "nav-item active" } else { "nav-item" }
                    on:click=move |_| set_active_tab.set(SidebarTab::Settings)
                >
                    "Settings"
                </button>
            </nav>
        </div>
    }
}
