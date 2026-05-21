use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};

use crate::api::client::{delete_manager, get_risk_policy};

#[component]
pub fn SettingsTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let policy = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            get_risk_policy(uuid).await.ok()
        }
    });

    view! {
        <div style="max-width:540px;">
            <Suspense fallback=move || view! { <SettingsSkeleton/> }>
                {move || match policy.get().map(|w| (*w).clone()) {
                    None => view! { <SettingsSkeleton/> }.into_any(),
                    Some(None) => view! { <p class="text-muted">"리스크 정책을 불러오지 못했습니다."</p> }.into_any(),
                    Some(Some(p)) => view! {
                        <div>
                            <h3 style="margin-bottom:20px;">"리스크 정책"</h3>

                            <div class="card card-sm" style="margin-bottom:16px;">
                                <div style="display:flex;flex-direction:column;gap:16px;">
                                    <PolicyField
                                        label="최대 종목 비중"
                                        value=format!("{}%", p.max_position_pct)
                                        hint="종목당 포트폴리오 대비 최대 비중"
                                    />
                                    <PolicyField
                                        label="최대 단일 주문금액"
                                        value=format!("{} KRW", p.max_single_order_amount_krw)
                                        hint="1회 주문 최대 금액 (원화 기준)"
                                    />
                                    <PolicyField
                                        label="일 최대 손실"
                                        value=format!("-{}%", p.max_daily_loss_pct)
                                        hint="일 손실이 이 수치를 초과하면 자동매매 일시정지"
                                    />
                                    <PolicyField
                                        label="일 최대 거래 횟수"
                                        value=p.max_daily_trade_count.to_string()
                                        hint=""
                                    />
                                    <PolicyField
                                        label="AI 최소 신뢰도"
                                        value=format!("{}%", p.min_ai_confidence_pct)
                                        hint="이 수치 미만이면 주문 계획 생성 안 함"
                                    />
                                    <PolicyField
                                        label="최소 근거 수"
                                        value=p.min_evidence_count.to_string()
                                        hint="Evidence Card 최소 개수"
                                    />
                                </div>
                            </div>

                            <div class="card card-sm" style="background:var(--red-dim);border-color:var(--red);">
                                <h3 style="color:var(--red);margin-bottom:12px;">"위험 구역"</h3>
                                <p class="text-muted" style="font-size:0.85rem;margin-bottom:12px;">
                                    "매니저를 삭제해도 주문·체결·분석 기록은 보존됩니다."
                                </p>
                                <DeleteManagerButton/>
                            </div>
                        </div>
                    }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn DeleteManagerButton() -> impl IntoView {
    let params = use_params_map();
    let navigate = use_navigate();
    let deleting = RwSignal::new(false);
    let delete_error = RwSignal::new(None::<String>);
    let confirm_delete = RwSignal::new(false);

    let on_delete = move |_| {
        delete_error.set(None);

        if !confirm_delete.get() {
            confirm_delete.set(true);
            return;
        }

        let manager_id =
            params.with(|p| p.get("id").and_then(|id| uuid::Uuid::parse_str(&id).ok()));
        let Some(manager_id) = manager_id else {
            delete_error.set(Some("매니저 ID를 확인할 수 없습니다.".to_string()));
            return;
        };

        deleting.set(true);
        let nav = navigate.clone();
        leptos::task::spawn_local(async move {
            match delete_manager(manager_id).await {
                Ok(_) => nav("/managers", Default::default()),
                Err(e) => {
                    delete_error.set(Some(format!("삭제 실패: {e}")));
                    deleting.set(false);
                    confirm_delete.set(false);
                }
            }
        });
    };

    view! {
        {move || delete_error.get().map(|e| view! {
            <div class="alert alert-error" style="margin-bottom:12px;">{e}</div>
        })}
        <button
            class="btn btn-danger btn-sm"
            on:click=on_delete
            prop:disabled=move || deleting.get()
        >
            {move || {
                if deleting.get() {
                    "삭제 중..."
                } else if confirm_delete.get() {
                    "한 번 더 눌러 삭제"
                } else {
                    "매니저 삭제"
                }
            }}
        </button>
    }
}

#[component]
fn PolicyField(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    #[prop(into)] hint: String,
) -> impl IntoView {
    view! {
        <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;">
            <div>
                <div style="font-size:0.875rem;font-weight:500;">{label}</div>
                {if !hint.is_empty() {
                    view! { <div class="text-tiny">{hint}</div> }.into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            </div>
            <div style="font-size:0.875rem;font-weight:600;white-space:nowrap;">{value}</div>
        </div>
    }
}

#[component]
fn SettingsSkeleton() -> impl IntoView {
    view! {
        <div style="display:flex;flex-direction:column;gap:12px;">
            {(0..5).map(|_| view! {
                <div class="skeleton" style="height:36px;width:100%;"></div>
            }).collect_view()}
        </div>
    }
}
