use leptos::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::api::client::get_dev_token;
use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <AppLayout>
            <PageHeader title="설정"/>

            <div style="display:flex;flex-direction:column;gap:24px;max-width:640px;">
                <DevTokenSection/>
                <SystemInfoSection/>
                <FeatureFlagSection/>
            </div>
        </AppLayout>
    }
}

#[component]
fn DevTokenSection() -> impl IntoView {
    let token_display = RwSignal::new(Option::<String>::None);
    let user_id_display = RwSignal::new(Option::<String>::None);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let copied = RwSignal::new(false);

    let fetch_token = move |_| {
        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match get_dev_token(None).await {
                Ok(resp) => {
                    token_display.set(Some(resp.token));
                    user_id_display.set(Some(resp.user_id.to_string()));
                }
                Err(e) => {
                    error.set(Some(format!("토큰 발급 실패: {e}")));
                }
            }
            loading.set(false);
        });
    };

    let copy_token = move |_| {
        let Some(token) = token_display.get() else {
            return;
        };

        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };
        let clipboard = window.navigator().clipboard();

        copied.set(true);
        leptos::task::spawn_local(async move {
            let _ = JsFuture::from(clipboard.write_text(&token)).await;
            gloo_timers::future::TimeoutFuture::new(2000).await;
            copied.set(false);
        });
    };

    view! {
        <div class="card">
            <div class="card-section-title">"개발용 JWT 토큰"</div>
            <p class="text-muted" style="font-size:13px;margin-bottom:16px;">
                "API 서버에 dev 토큰을 요청합니다. 프로덕션 환경("
                <code>"APP_ENV=production"</code>
                ")에서는 비활성화됩니다."
            </p>

            {move || error.get().map(|e| view! {
                <div class="alert alert-error" style="margin-bottom:12px;">{e}</div>
            })}

            {move || token_display.get().map(|t| {
                let uid = user_id_display.get().unwrap_or_default();
                view! {
                    <div style="margin-bottom:16px;">
                        <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px;">
                            <span class="text-muted" style="font-size:12px;">"User ID: " {uid}</span>
                        </div>
                        <div class="token-box">
                            <code style="word-break:break-all;font-size:11px;">{t}</code>
                        </div>
                    </div>
                }
            })}

            <div style="display:flex;gap:8px;">
                <button
                    class="btn btn-primary btn-sm"
                    on:click=fetch_token
                    prop:disabled=loading
                >
                    {move || if loading.get() { "발급 중..." } else { "토큰 발급" }}
                </button>
                {move || token_display.get().map(|_| view! {
                    <button
                        class="btn btn-ghost btn-sm"
                        on:click=copy_token
                    >
                        {move || if copied.get() { "복사됨!" } else { "복사" }}
                    </button>
                })}
            </div>
        </div>
    }
}

#[component]
fn SystemInfoSection() -> impl IntoView {
    view! {
        <div class="card">
            <div class="card-section-title">"시스템 정보"</div>
            <table class="info-table">
                <tbody>
                    <tr>
                        <td class="info-label">"API 서버"</td>
                        <td><code>"/api"</code>" (Trunk 프록시 → localhost:5000)"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"스케줄러"</td>
                        <td>"30초마다 tick, 5분 슬롯 단위 실행"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"지원 시장"</td>
                        <td>"KRX 정규장 + 미국 NYSE/NASDAQ"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"주문 방식"</td>
                        <td>"지정가 전용"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"AI 파이프라인"</td>
                        <td>"Fundamental → News → Strategy → Critic (4-step)"</td>
                    </tr>
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn FeatureFlagSection() -> impl IntoView {
    let flags = [
        (
            "offline-fixtures",
            true,
            "fixture JSON으로 외부 API 대체 (기본)",
        ),
        ("online-kis", false, "KIS 실시간 API"),
        ("online-naver", false, "네이버 뉴스 API"),
        ("online-opendart", false, "DART 공시 API"),
        ("online-sec", false, "SEC Edgar API"),
        ("online-telegram", false, "Telegram 알림"),
        ("live-trading", false, "실전 브로커 주문 실행"),
    ];

    view! {
        <div class="card">
            <div class="card-section-title">"Feature Flags"</div>
            <p class="text-muted" style="font-size:13px;margin-bottom:16px;">
                "빌드 타임에 결정됩니다. 변경하려면 "
                <code>"cargo build --features"</code>
                " 옵션을 사용하세요."
            </p>
            <div style="display:flex;flex-direction:column;gap:8px;">
                {flags.into_iter().map(|(name, active, desc)| {
                    view! {
                        <div class="flag-row">
                            <div style="display:flex;align-items:center;gap:10px;">
                                <span class=if active { "dot dot-green" } else { "dot dot-gray" }></span>
                                <code style="font-size:12px;">{name}</code>
                            </div>
                            <span class="text-muted" style="font-size:12px;">{desc}</span>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
