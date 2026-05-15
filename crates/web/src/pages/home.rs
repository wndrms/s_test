use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="home">
            <h1 class="logo">"✦ Lumos"</h1>
            <p class="tagline">"AI-powered fund manager for KRX & US markets"</p>
            <a href="/managers" class="btn-primary">"매니저 시작하기"</a>
        </div>
    }
}
