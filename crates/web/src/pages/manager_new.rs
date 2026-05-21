use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::client::{
    create_manager, verify_kis_auth, CreateManagerRequest, ValidateKisConnectionRequest,
};
use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn ManagerNewPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let mode = RwSignal::new("paper".to_string());
    let region = RwSignal::new("KR".to_string());
    let initial_capital = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let submitting = RwSignal::new(false);
    let kis_app_key = RwSignal::new(String::new());
    let kis_app_secret = RwSignal::new(String::new());
    let kis_account_no = RwSignal::new(String::new());
    let kis_account_product = RwSignal::new("01".to_string());
    let verifying_account = RwSignal::new(false);
    let account_message = RwSignal::new(Option::<String>::None);
    let account_verified = RwSignal::new(false);

    let navigate = use_navigate();

    let optional_trimmed = |value: String| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    };

    let on_verify_account = move |_| {
        let app_key = kis_app_key.get().trim().to_string();
        let app_secret = kis_app_secret.get().trim().to_string();
        let account_no = kis_account_no.get().trim().to_string();
        let account_product = optional_trimmed(kis_account_product.get());
        let mode_val = mode.get();
        let region_val = region.get();

        account_verified.set(false);
        account_message.set(None);

        if app_key.is_empty() || app_secret.is_empty() || account_no.is_empty() {
            account_message.set(Some(
                "KIS App Key, App Secret, 계좌번호를 입력해주세요.".to_string(),
            ));
            return;
        }

        verifying_account.set(true);
        leptos::task::spawn_local(async move {
            let req = ValidateKisConnectionRequest {
                app_key,
                app_secret,
                account_no,
                account_product,
                mode: mode_val,
                region: region_val,
            };

            match verify_kis_auth(req).await {
                Ok(resp) => {
                    account_verified.set(resp.success);
                    account_message.set(Some(resp.message));
                }
                Err(e) => {
                    account_verified.set(false);
                    account_message.set(Some(format!("계좌 확인 실패: {e}")));
                }
            }
            verifying_account.set(false);
        });
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let name_val = name.get();
        let mode_val = mode.get();
        let region_val = region.get();
        let capital_str = initial_capital.get();
        let kis_app_key_val = optional_trimmed(kis_app_key.get());
        let kis_app_secret_val = optional_trimmed(kis_app_secret.get());
        let kis_account_no_val = optional_trimmed(kis_account_no.get());
        let kis_account_product_val = optional_trimmed(kis_account_product.get());

        if name_val.trim().is_empty() {
            error.set(Some("매니저 이름을 입력해주세요.".to_string()));
            return;
        }
        if name_val.trim().len() > 100 {
            error.set(Some("매니저 이름은 100자 이하여야 합니다.".to_string()));
            return;
        }

        let capital = if mode_val == "live" {
            None
        } else {
            match capital_str.trim().parse::<f64>() {
                Ok(v) if v > 0.0 => Some(v),
                _ => {
                    error.set(Some("초기 자본금은 0보다 큰 숫자여야 합니다.".to_string()));
                    return;
                }
            }
        };

        let kis_required_count = [
            kis_app_key_val.is_some(),
            kis_app_secret_val.is_some(),
            kis_account_no_val.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        if kis_required_count > 0 && kis_required_count < 3 {
            error.set(Some(
                "계좌 연결에는 KIS App Key, App Secret, 계좌번호가 모두 필요합니다.".to_string(),
            ));
            return;
        }

        let base_currency = if region_val == "US" { "USD" } else { "KRW" }.to_string();

        error.set(None);
        submitting.set(true);
        let nav = navigate.clone();

        leptos::task::spawn_local(async move {
            let req = CreateManagerRequest {
                broker_connection_id: None,
                name: name_val.trim().to_string(),
                mode: mode_val,
                region: region_val,
                base_currency,
                initial_capital: capital,
                kis_app_key: kis_app_key_val,
                kis_app_secret: kis_app_secret_val,
                kis_account_no: kis_account_no_val,
                kis_account_product: kis_account_product_val,
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

                    <div
                        class="form-group"
                        style=move || if mode.get() == "live" { "display:none;" } else { "" }
                    >
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

                    <div style="border-top:1px solid var(--border); padding-top:16px; margin-top:8px;">
                        <div class="card-section-title">"계좌 연결"</div>

                        <div class="form-row">
                            <div class="form-group">
                                <label class="form-label">"KIS App Key"</label>
                                <input
                                    type="text"
                                    class="form-input"
                                    placeholder="KIS App Key"
                                    prop:value=kis_app_key
                                    on:input=move |ev| {
                                        account_verified.set(false);
                                        account_message.set(None);
                                        kis_app_key.set(event_target_value(&ev));
                                    }
                                />
                            </div>

                            <div class="form-group">
                                <label class="form-label">"KIS App Secret"</label>
                                <input
                                    type="password"
                                    class="form-input"
                                    placeholder="KIS App Secret"
                                    prop:value=kis_app_secret
                                    on:input=move |ev| {
                                        account_verified.set(false);
                                        account_message.set(None);
                                        kis_app_secret.set(event_target_value(&ev));
                                    }
                                />
                            </div>
                        </div>

                        <div class="form-row">
                            <div class="form-group">
                                <label class="form-label">"계좌번호"</label>
                                <input
                                    type="text"
                                    class="form-input"
                                    placeholder="예: 12345678"
                                    prop:value=kis_account_no
                                    on:input=move |ev| {
                                        account_verified.set(false);
                                        account_message.set(None);
                                        kis_account_no.set(event_target_value(&ev));
                                    }
                                />
                            </div>

                            <div class="form-group">
                                <label class="form-label">"상품코드"</label>
                                <input
                                    type="text"
                                    class="form-input"
                                    placeholder="01"
                                    prop:value=kis_account_product
                                    on:input=move |ev| {
                                        account_verified.set(false);
                                        account_message.set(None);
                                        kis_account_product.set(event_target_value(&ev));
                                    }
                                />
                            </div>
                        </div>

                        {move || account_message.get().map(|message| {
                            let class = if account_verified.get() {
                                "alert alert-success"
                            } else {
                                "alert alert-error"
                            };
                            view! {
                                <div class=class style="margin-bottom:16px;">{message}</div>
                            }
                        })}

                        <button
                            type="button"
                            class="btn btn-secondary btn-sm"
                            on:click=on_verify_account
                            prop:disabled=move || verifying_account.get() || submitting.get()
                        >
                            {move || if verifying_account.get() { "확인 중..." } else { "계좌 입력 확인" }}
                        </button>
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
