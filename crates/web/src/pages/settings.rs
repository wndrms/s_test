use leptos::prelude::*;

use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <AppLayout>
            <PageHeader title="설정"/>

            <div style="display:flex;flex-direction:column;gap:24px;max-width:640px;">
                <SystemInfoSection/>
                <FeatureFlagSection/>
            </div>
        </AppLayout>
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
                        <td><code>"/api"</code>" (자동 인증 활성화)"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"스케줄러"</td>
                        <td>"30초마다 tick, 5분 슬롯 단위 실행"</td>
                    </tr>
                    <tr>
                        <td class="info-label">"지원 시장"</td>
                        <td>"KRX 정규장 / 미국 NYSE·NASDAQ (매니저별 선택)"</td>
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
