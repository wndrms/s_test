use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::client::{create_manager, CreateManagerRequest};
use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn ManagerNewPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let mode = RwSignal::new("paper".to_string());
    let region = RwSignal::new("KR".to_string());
    let initial_capital = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let submitting = RwSignal::new(false);

    let navigate = use_navigate();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let name_val = name.get();
        let mode_val = mode.get();
        let region_val = region.get();
        let capital_str = initial_capital.get();

        if name_val.trim().is_empty() {
            error.set(Some("매니저 이름을 입력해주세요.".to_string()));
            return;
        }
        if name_val.trim().len() > 100 {
            error.set(Some("매니저 이름은 100자 이하여야 합니다.".to_string()));
            return;
        }

        let capital: f64 = match capital_str.trim().parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            _ => {
                error.set(Some("초기 자본금은 0보다 큰 숫자여야 합니다.".to_string()));
                return;
            }
        };

        let base_currency = if region_val == "US" { "USD" } else { "KRW" }.to_string();

        error.set(None);
        submitting.set(true);
        let nav = navigate.clone();

        leptos::task::spawn_local(async move {
            let req = CreateManagerRequest {
                // 개발 환경에서는 서버가 JWT user_id와 .env KIS 설정으로 기본 연결을 보강한다.
                broker_connection_id: None,
                name: name_val.trim().to_string(),
                mode: mode_val,
                region: region_val,
                base_currency,
                initial_capital: capital,
            };

            match create_manager(req).await {
                Ok(_) => {
                    submitting.set(false);
                    nav("/managers", Default::default());
                }
                Err(e) => {
                    error.set(Some(format!("생성 실패: {e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <AppLayout>
            <PageHeader title="새 매니저 만들기"/>

            <div class="card" style="max-width:560px;">
                <form on:submit=on_submit>
                    <div class="form-group">
                        <label class="form-label">"매니저 이름"</label>
                        <input
                            type="text"
                            class="form-input"
                            placeholder="예: KR 성장주 전략"
                            prop:value=name
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                    </div>

                    <div class="form-row">
                        <div class="form-group">
                            <label class="form-label">"매매 모드"</label>
                            <select
                                class="form-select"
                                prop:value=mode
                                on:change=move |ev| mode.set(event_target_value(&ev))
                            >
                                <option value="paper">"모의 (Paper)"</option>
                                <option value="live">"실전 (Live)"</option>
                            </select>
                        </div>

                        <div class="form-group">
                            <label class="form-label">"시장"</label>
                            <select
                                class="form-select"
                                prop:value=region
                                on:change=move |ev| region.set(event_target_value(&ev))
                            >
                                <option value="KR">"한국 (KRX)"</option>
                                <option value="US">"미국 (NYSE/NASDAQ)"</option>
                            </select>
                        </div>
                    </div>

                    <div class="form-group">
                        <label class="form-label">
                            {move || if region.get() == "US" { "초기 자본금 (USD)" } else { "초기 자본금 (KRW)" }}
                        </label>
                        <input
                            type="number"
                            class="form-input"
                            placeholder=move || if region.get() == "US" { "예: 10000" } else { "예: 10000000" }
                            prop:value=initial_capital
                            on:input=move |ev| initial_capital.set(event_target_value(&ev))
                            min="1"
                            step="1"
                        />
                        <p class="form-hint">
                            "Risk Gate 기준: 단일 주문 ≤ 100만원, 종목 비중 ≤ 자산의 5%"
                        </p>
                    </div>

                    {move || error.get().map(|e| view! {
                        <div class="alert alert-error">{e}</div>
                    })}

                    <div class="form-actions">
                        <a href="/managers" class="btn btn-ghost">"취소"</a>
                        <button
                            type="submit"
                            class="btn btn-primary"
                            prop:disabled=submitting
                        >
                            {move || if submitting.get() { "생성 중..." } else { "매니저 생성" }}
                        </button>
                    </div>
                </form>
            </div>
        </AppLayout>
    }
}
