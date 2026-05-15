use leptos::prelude::*;

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div class="not-found">
            <h2>"404"</h2>
            <p>"페이지를 찾을 수 없습니다."</p>
            <a href="/" class="btn-secondary">"홈으로"</a>
        </div>
    }
}
