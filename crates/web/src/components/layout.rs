use leptos::prelude::*;

use super::nav::{BottomNav, Sidebar};

#[component]
pub fn AppLayout(children: Children) -> impl IntoView {
    view! {
        <div class="app-layout">
            <Sidebar/>
            <main class="main-content">
                {children()}
            </main>
            <BottomNav/>
        </div>
    }
}

#[component]
pub fn PageHeader(
    #[prop(into)] title: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="page-header">
            <h2>{title}</h2>
            {children.map(|c| view! { <div>{c()}</div> })}
        </div>
    }
}
