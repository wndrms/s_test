use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};

use crate::api::client::{delete_manager, get_manager, get_risk_policy};

#[component]
pub fn SettingsTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let manager = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            get_manager(uuid).await.ok()
        }
    });

    let policy = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            get_risk_policy(uuid).await.ok()
        }
    });

    let market_label = move || {
        manager.get()
            .and_then(|w| (*w).clone())
            .map(|m| match m.region.as_str() {
                "US" => "미국 NYSE · NASDAQ",
                _ => "KRX 정규장",
            })
            .unwrap_or("—")
            .to_string()
    };

    view! {
        <div style="max-width:540px;">
            <Suspense fallback=move || view! { <SettingsSkeleton/> }>
                {move || match policy.get().map(|w| (*w).clone()) {
                    None => view! { <SettingsSkeleton/> }.into_any(),
                    Some(None) => view! { <p class="text-muted">"리스크 정책을 불러오지 못했습니다."</p> }.into_any(),
                    Some(Some(p)) => view! {
                        <div>
                            <div class="card card-sm" style="margin-bottom:20px;display:flex;align-items:center;gap:12px;">
                                <span class="text-muted" style="font-size:0.85rem;">"지원 시장"</span>
                                <span style="font-weight:600;font-size:0.875rem;">{market_label()}</span>
                            </div>

                            <h3 style="margin-bottom:4px;">"리스크 정책"</h3>
                            <p class="text-muted" style="font-size:0.85rem;margin-bottom:20px;">
                                "자동매매 주문이 실행되기 전 적용되는 핵심 안전 기준입니다."
                            </p>

                            <div class="card card-sm" style="margin-bottom:16px;">
                                <div style="display:flex;flex-direction:column;gap:16px;">
                                    <PolicyField
                                        label="일 최대 손실"
                                        value=format!("-{}%", p.max_daily_loss_pct)
                                        hint="일 손실이 초과되면 당일 자동매매 중단 (권장 2~5%)"
                                    />
                                    <PolicyField
                                        label="최대 단일 주문금액"
                                        value=format!("{} KRW", p.max_single_order_amount_krw)
                                        hint="1회 주문 상한 — 오작동·과대 주문 방지 (권장 자본의 5~10%)"
                                    />
                                    <PolicyField
                                        label="AI 최소 신뢰도"
                                        value=format!("{}%", p.min_ai_confidence_pct)
                                        hint="이 신뢰도 미만의 시나리오는 주문하지 않음 (권장 40~60%)"
                                    />
                                    <PolicyField
                                        label="최소 근거 수"
                                        value=p.min_evidence_count.to_string()
                                        hint="시나리오 1건당 필요한 최소 Evidence 개수 (권장 2개 이상)"
                                    />
                                </div>
                            </div>

                            <details class="card card-sm" style="margin-bottom:16px;">
                                <summary style="cursor:pointer;font-size:0.875rem;font-weight:500;">
                                    "시스템 기본 안전장치"
                                </summary>
                                <p class="text-muted" style="font-size:0.8rem;margin-top:12px;">
                                    "아래 항목은 시스템이 자동 적용하는 고정 안전장치입니다 (사용자 조정 불필요)."
                                </p>
                                <div style="display:flex;flex-direction:column;gap:12px;margin-top:12px;">
                                    <PolicyField
                                        label="주문 방식"
                                        value="지정가 / 정규장 전용".to_string()
                                        hint="시장가·장전·장후 매매는 허용하지 않습니다"
                                    />
                                    <PolicyField
                                        label="데이터 신선도"
                                        value=format!("시세 {}초 / 잔고 {}초", p.require_fresh_quote_seconds, p.require_fresh_account_seconds)
                                        hint="오래된 데이터로는 주문하지 않습니다"
                                    />
                                </div>
                            </details>

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
