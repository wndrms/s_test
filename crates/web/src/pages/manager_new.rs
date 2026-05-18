use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::api::client::{create_manager, CreateManagerRequest, validate_kis_connection, verify_kis_auth, ValidateKisConnectionRequest, VerifyKisAuthResponse};
use crate::components::layout::{AppLayout, PageHeader};
use lumos_domain::model::broker::BrokerAccount;

#[component]
pub fn ManagerNewPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let mode = RwSignal::new("paper".to_string());
    let region = RwSignal::new("KR".to_string());
    let initial_capital = RwSignal::new(String::new());
    let kis_app_key = RwSignal::new(String::new());
    let kis_app_secret = RwSignal::new(String::new());
    let kis_account_no = RwSignal::new(String::new());
    let kis_account_product = RwSignal::new("01".to_string());
    let connection_result = RwSignal::new(Option::<BrokerAccount>::None);
    let auth_result = RwSignal::new(Option::<VerifyKisAuthResponse>::None);
    let checking_connection = RwSignal::new(false);
    let checking_auth = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let submitting = RwSignal::new(false);

    let navigate = use_navigate();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        let name_val = name.get();
        let mode_val = mode.get();
        let region_val = region.get();
        let capital_str = initial_capital.get();
        let kis_app_key_val = kis_app_key.get();
        let kis_app_secret_val = kis_app_secret.get();
        let kis_account_no_val = kis_account_no.get();
        let kis_account_product_val = kis_account_product.get();

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
                broker_connection_id: None,
                name: name_val.trim().to_string(),
                mode: mode_val,
                region: region_val,
                base_currency,
                initial_capital: capital,
                kis_app_key: (!kis_app_key_val.trim().is_empty()).then_some(kis_app_key_val.trim().to_string()),
                kis_app_secret: (!kis_app_secret_val.trim().is_empty()).then_some(kis_app_secret_val.trim().to_string()),
                kis_account_no: (!kis_account_no_val.trim().is_empty()).then_some(kis_account_no_val.trim().to_string()),
                kis_account_product: (!kis_account_product_val.trim().is_empty()).then_some(kis_account_product_val.trim().to_string()),
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

    let on_check_connection = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();

        let mode_val = mode.get();
        let region_val = region.get();
        let app_key_val = kis_app_key.get();
        let app_secret_val = kis_app_secret.get();
        let account_no_val = kis_account_no.get();
        let account_product_val = kis_account_product.get();

        if app_key_val.trim().is_empty()
            || app_secret_val.trim().is_empty()
            || account_no_val.trim().is_empty()
        {
            error.set(Some("KIS app key, app secret, 계좌번호를 모두 입력해주세요.".to_string()));
            return;
        }

        error.set(None);
        connection_result.set(None);
        auth_result.set(None);
        checking_connection.set(true);

        let err = error.clone();
        let result = connection_result.clone();
        let auth_res = auth_result.clone();
        let checking = checking_connection.clone();
        let checking_a = checking_auth.clone();

        leptos::task::spawn_local(async move {
            let req = ValidateKisConnectionRequest {
                app_key: app_key_val.trim().to_string(),
                app_secret: app_secret_val.trim().to_string(),
                account_no: account_no_val.trim().to_string(),
                account_product: (!account_product_val.trim().is_empty())
                    .then_some(account_product_val.trim().to_string()),
                mode: mode_val.clone(),
                region: region_val.clone(),
            };

            // Step 1: Verify KIS Auth Token
            checking_a.set(true);
            match verify_kis_auth(req.clone()).await {
                Ok(auth) => {
                    auth_res.set(Some(auth.clone()));
                    if !auth.success {
                        err.set(Some(auth.message));
                        checking.set(false);
                        checking_a.set(false);
                        return;
                    }
                }
                Err(e) => {
                    err.set(Some(format!("토큰 확인 실패: {e}")));
                    checking.set(false);
                    checking_a.set(false);
                    return;
                }
            }
            checking_a.set(false);

            // Step 2: Validate KIS Connection (get balance)
            match validate_kis_connection(req).await {
                Ok(account) => {
                    result.set(Some(account));
                    err.set(None);
                }
                Err(e) => {
                    err.set(Some(format!("계좌 확인 실패: {e}")));
                }
            }

            checking.set(false);
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

                    <div class="form-group">
                        <label class="form-label">"KIS App Key"</label>
                        <input
                            type="text"
                            class="form-input"
                            placeholder="KIS APP_KEY"
                            prop:value=kis_app_key
                            on:input=move |ev| kis_app_key.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-group">
                        <label class="form-label">"KIS App Secret"</label>
                        <input
                            type="password"
                            class="form-input"
                            placeholder="KIS APP_SECRET"
                            prop:value=kis_app_secret
                            on:input=move |ev| kis_app_secret.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-row">
                        <div class="form-group">
                            <label class="form-label">"KIS 계좌번호"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="계좌번호 앞 8자리"
                                prop:value=kis_account_no
                                on:input=move |ev| kis_account_no.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="form-group">
                            <label class="form-label">"상품코드"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="예: 01"
                                prop:value=kis_account_product
                                on:input=move |ev| kis_account_product.set(event_target_value(&ev))
                            />
                        </div>
                    </div>

                    <div class="form-group">
                        <button
                            type="button"
                            class="btn btn-secondary btn-block"
                            on:click=on_check_connection
                            prop:disabled=checking_connection
                        >
                            {move || if checking_connection.get() { "계좌 확인 중..." } else { "KIS 계좌 확인" }}
                        </button>
                    </div>

                    {move || auth_result.get().as_ref().map(|auth| view! {
                        <div class=if auth.success { "alert alert-success" } else { "alert alert-warning" }>
                            <p>{auth.message.clone()}</p>
                        </div>
                    })}

                    {move || connection_result.get().as_ref().map(|account| view! {
                        <div class="alert alert-success">
                            <p>{format!("계좌 연결 확인됨: 현금 {} {}, 총자산 {} {}", account.cash, account.currency, account.total_equity, account.currency)}</p>
                            <p>{format!("조회 시각: {}", account.as_of)}</p>
                        </div>
                    })}

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
