use leptos::prelude::*;
use leptos_router::hooks::use_location;

#[component]
pub fn Sidebar() -> impl IntoView {
    view! {
        <aside class="sidebar">
            <div class="sidebar-logo">
                <span class="logo">"✦ "<span class="text-accent">"Lumos"</span></span>
            </div>
            <nav class="sidebar-nav">
                <NavItem href="/" icon="🏠" label="대시보드"/>
                <NavItem href="/managers" icon="📊" label="매니저"/>
                <NavItem href="/llm-keys" icon="🔑" label="LLM 키"/>
                <NavItem href="/settings" icon="⚙️" label="설정"/>
            </nav>
        </aside>
    }
}

#[component]
pub fn BottomNav() -> impl IntoView {
    view! {
        <nav class="bottom-nav">
            <BottomNavItem href="/" icon="🏠" label="홈"/>
            <BottomNavItem href="/managers" icon="📊" label="매니저"/>
            <BottomNavItem href="/llm-keys" icon="🔑" label="키"/>
            <BottomNavItem href="/settings" icon="⚙️" label="설정"/>
        </nav>
    }
}

#[component]
fn NavItem(
    #[prop(into)] href: String,
    #[prop(into)] icon: String,
    #[prop(into)] label: String,
) -> impl IntoView {
    let location = use_location();
    let href_clone = href.clone();
    let is_active = move || location.pathname.get() == href_clone;

    view! {
        <a href=href class=move || format!("sidebar-nav-item{}", if is_active() { " active" } else { "" })>
            <span class="sidebar-nav-icon">{icon}</span>
            <span>{label}</span>
        </a>
    }
}

#[component]
fn BottomNavItem(
    #[prop(into)] href: String,
    #[prop(into)] icon: String,
    #[prop(into)] label: String,
) -> impl IntoView {
    let location = use_location();
    let href_clone = href.clone();
    let is_active = move || location.pathname.get() == href_clone;

    view! {
        <a href=href class=move || format!("bottom-nav-item{}", if is_active() { " active" } else { "" })>
            <span class="nav-icon">{icon}</span>
            <span>{label}</span>
        </a>
    }
}
