use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::client::{fetch_llm_keys, list_scenarios, trigger_scenario_run, LlmKeyDto, SymbolDto};
use crate::api::types::ScenarioItemDto;
use crate::components::badge::{ActionLabel, ScenarioBadge};
use crate::components::symbol_search::SymbolSearch;

const OPENAI_MODELS: &[(&str, &str)] = &[
    ("gpt-4o", "GPT-4o"),
    ("gpt-4o-mini", "GPT-4o Mini"),
    ("gpt-4-turbo", "GPT-4 Turbo"),
];

const GEMINI_MODELS: &[(&str, &str)] = &[
    ("gemini-2.0-flash-exp", "Gemini 2.0 Flash (Exp)"),
    ("gemini-1.5-flash", "Gemini 1.5 Flash"),
    ("gemini-1.5-flash-8b", "Gemini 1.5 Flash 8B"),
    ("gemini-1.5-pro", "Gemini 1.5 Pro"),
];

#[component]
pub fn ScenariosTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());
    let show_generate_form = RwSignal::new(false);

    let scenarios = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = Uuid::parse_str(&id).ok()?;
            list_scenarios(uuid).await.ok()
        }
    });

    view! {
        <div style="margin-bottom:24px;display:flex;gap:12px;flex-wrap:wrap;">
            <button
                class="btn btn-primary btn-sm"
                on:click=move |_| show_generate_form.set(!show_generate_form.get())
            >
                {move || if show_generate_form.get() { "취소" } else { "+ 시나리오 생성" }}
            </button>
        </div>

        {move || show_generate_form.get().then(|| {
            let manager_id = Uuid::parse_str(&id_str()).ok();
            manager_id.map(|mid| view! {
                <GenerateScenarioForm
                    manager_id=mid
                    on_success=move || {
                        show_generate_form.set(false);
                    }
                />
            })
        })}

        <Suspense fallback=move || view! { <ScenarioSkeleton/> }>
            {move || match scenarios.get().map(|w| (*w).clone()) {
                None => view! { <ScenarioSkeleton/> }.into_any(),
                Some(None) => view! {
                    <p class="text-muted">"시나리오를 불러오지 못했습니다."</p>
                }.into_any(),
                Some(Some(list)) if list.is_empty() => view! {
                    <div class="empty-state">
                        <span class="empty-icon">"◈"</span>
                        <p>"생성된 시나리오가 없습니다."</p>
                        <p class="text-muted">"위 버튼으로 시나리오를 생성하거나 스케줄을 설정하세요."</p>
                    </div>
                }.into_any(),
                Some(Some(list)) => view! {
                    <div class="scenario-grid">
                        {list.into_iter().map(|s| view! { <ScenarioCard item=s/> }).collect_view()}
                    </div>
                }.into_any(),
            }}
        </Suspense>
    }
}

#[component]
fn GenerateScenarioForm(manager_id: Uuid, on_success: impl Fn() + 'static + Copy) -> impl IntoView {
    let selected_key = RwSignal::new(Option::<Uuid>::None);
    let selected_provider = RwSignal::new("gemini".to_string());
    let selected_model = RwSignal::new("gemini-1.5-flash".to_string());
    let selected_symbol = RwSignal::new(Option::<SymbolDto>::None);
    let base_price = RwSignal::new(String::new());
    let submitting = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let llm_keys = RwSignal::new(Vec::<LlmKeyDto>::new());

    // LLM 키 목록 로드
    leptos::task::spawn_local(async move {
        if let Ok(keys) = fetch_llm_keys().await {
            llm_keys.set(keys);
        }
    });

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let Some(symbol) = selected_symbol.get() else {
            error.set(Some("종목을 선택하세요".to_string()));
            return;
        };

        let key_id = selected_key.get();
        let model = selected_model.get();
        let price = base_price.get();

        if price.is_empty() {
            error.set(Some("기준 가격을 입력하세요".to_string()));
            return;
        }

        submitting.set(true);
        error.set(None);

        leptos::task::spawn_local(async move {
            match trigger_scenario_run(manager_id, symbol.id, key_id, &model, &price).await {
                Ok(_) => {
                    on_success();
                }
                Err(e) => error.set(Some(format!("생성 실패: {e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="card" style="margin-bottom:24px;">
            <div class="card-section-title">"새 시나리오 생성"</div>

            {move || error.get().map(|e| view! {
                <div class="alert alert-error" style="margin-bottom:16px;">{e}</div>
            })}

            <form on:submit=on_submit>
                <div class="form-group">
                    <label class="form-label">"종목 선택"</label>
                    <SymbolSearch on_select=move |symbol: SymbolDto| {
                        selected_symbol.set(Some(symbol));
                    }/>
                    {move || selected_symbol.get().map(|symbol| view! {
                        <div style="margin-top:8px;padding:8px;background:var(--bg-2);border-radius:4px;">
                            <div style="display:flex;align-items:center;justify-content:space-between;">
                                <div style="display:flex;align-items:center;gap:8px;">
                                    <span class=format!("badge badge-{}", if symbol.region == "KR" { "blue" } else { "green" })>
                                        {symbol.region.clone()}
                                    </span>
                                    <span class="font-semibold">{symbol.display_code.clone()}</span>
                                    <span class="text-muted" style="font-size:13px;">
                                        {symbol.name_ko.clone().or(symbol.name_en.clone()).unwrap_or_default()}
                                    </span>
                                </div>
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-sm"
                                    on:click=move |_| selected_symbol.set(None)
                                >
                                    "✕"
                                </button>
                            </div>
                        </div>
                    })}
                </div>

                <div class="form-group">
                    <label class="form-label">"기준 가격"</label>
                    <input
                        type="text"
                        class="form-input"
                        placeholder="예: 70000"
                        prop:value=move || base_price.get()
                        on:input=move |ev| base_price.set(event_target_value(&ev))
                        required
                    />
                </div>

                <div class="form-group">
                    <label class="form-label">"LLM API 키"</label>
                    {move || {
                        let keys = llm_keys.get();
                        if keys.is_empty() {
                            view! {
                                <div class="alert" style="background:var(--bg-2);padding:12px;">
                                    <p class="text-muted" style="font-size:13px;">
                                        "등록된 LLM 키가 없습니다. "
                                        <a href="/llm-keys" style="color:var(--primary);">"LLM 키 관리"</a>
                                        " 페이지에서 키를 등록하세요."
                                    </p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <select
                                    class="form-input"
                                    on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        if val.is_empty() {
                                            selected_key.set(None);
                                            selected_provider.set("gemini".to_string());
                                        } else if let Ok(key_id) = Uuid::parse_str(&val) {
                                            selected_key.set(Some(key_id));
                                            if let Some(key) = keys.iter().find(|k| k.id == key_id) {
                                                selected_provider.set(key.provider.clone());
                                            }
                                        }
                                    }
                                >
                                    <option value="">"서버 기본 LLM 사용"</option>
                                    {keys.iter().map(|key| {
                                        let provider_badge = match key.provider.as_str() {
                                            "gemini" => "🔵",
                                            _ => "🟢",
                                        };
                                        let key_id_str = key.id.to_string();
                                        let display = format!("{} {} ({})", provider_badge, key.label, key.provider);
                                        view! {
                                            <option value=key_id_str>{display}</option>
                                        }
                                    }).collect_view()}
                                </select>
                            }.into_any()
                        }
                    }}
                </div>

                <div class="form-group">
                    <label class="form-label">"모델 선택"</label>
                    <select
                        class="form-input"
                        prop:value=move || selected_model.get()
                        on:change=move |ev| selected_model.set(event_target_value(&ev))
                    >
                        {move || {
                            let models = match selected_provider.get().as_str() {
                                "openai" => OPENAI_MODELS,
                                _ => GEMINI_MODELS,
                            };
                            models.iter().map(|(value, label)| {
                                view! {
                                    <option value=*value>{*label}</option>
                                }
                            }).collect_view()
                        }}
                    </select>
                    <div class="form-hint">
                        {move || match selected_provider.get().as_str() {
                            "openai" => "OpenAI 모델 선택",
                            _ => "Google Gemini 모델 선택",
                        }}
                    </div>
                </div>

                <button
                    type="submit"
                    class="btn btn-primary btn-sm"
                    prop:disabled=submitting
                >
                    {move || if submitting.get() { "생성 중..." } else { "생성" }}
                </button>
            </form>
        </div>
    }
}

#[component]
fn ScenarioCard(item: ScenarioItemDto) -> impl IntoView {
    let prob_f: f64 = item.probability_pct.as_f64().unwrap_or(0.0);
    let type_cls = item.scenario_type.clone();

    view! {
        <div class=format!("card card-hover scenario-card {}", type_cls)>
            <div class="scenario-card-header">
                <ScenarioBadge scenario_type=item.scenario_type.clone()/>
                <span style="font-size:0.75rem;color:var(--text-3);">{item.symbol_code.clone()}</span>
                <span class="scenario-rank">
                    {format!("#{}", item.rank_order)}
                </span>
            </div>

            <div class="scenario-prob-bar">
                <div
                    class=format!("scenario-prob-fill {}", type_cls)
                    style=format!("width: {}%", prob_f.min(100.0))
                ></div>
            </div>

            <div class="scenario-prices">
                {item.target_price.clone().map(|p| view! {
                    <div class="scenario-price-item">
                        <div class="scenario-price-label">"목표가"</div>
                        <div class="scenario-price-value text-green">{p.to_string()}</div>
                    </div>
                })}
                {item.stop_loss_price.clone().map(|p| view! {
                    <div class="scenario-price-item">
                        <div class="scenario-price-label">"손절가"</div>
                        <div class="scenario-price-value text-red">{p.to_string()}</div>
                    </div>
                })}
                <div class="scenario-price-item">
                    <div class="scenario-price-label">"확률"</div>
                    <div class="scenario-price-value">{format!("{:.0}%", prob_f)}</div>
                </div>
            </div>

            <p style="font-size:0.8rem;color:var(--text-2);margin-bottom:12px;line-height:1.5;">
                {item.condition_text.chars().take(120).collect::<String>()}
                {if item.condition_text.len() > 120 { "…" } else { "" }}
            </p>

            <p style="font-size:0.78rem;color:var(--text-3);margin-bottom:16px;line-height:1.4;">
                {item.strategy_text.chars().take(80).collect::<String>()}
                {if item.strategy_text.len() > 80 { "…" } else { "" }}
            </p>

            {item.risk_text.clone().map(|risk| view! {
                <div style="font-size:0.75rem;color:var(--red);margin-bottom:12px;">
                    "⚠ " {risk.chars().take(80).collect::<String>()}
                </div>
            })}

            <div class="scenario-action-row">
                <ActionLabel action=item.action.clone()/>
            </div>
        </div>
    }
}

#[component]
fn ScenarioSkeleton() -> impl IntoView {
    view! {
        <div class="scenario-grid">
            {(0..3).map(|_| view! {
                <div class="card">
                    <div class="skeleton" style="height:20px;width:50%;margin-bottom:16px;"></div>
                    <div class="skeleton" style="height:4px;width:100%;margin-bottom:16px;"></div>
                    <div class="skeleton" style="height:60px;width:100%;margin-bottom:16px;"></div>
                    <div class="skeleton" style="height:20px;width:30%;"></div>
                </div>
            }).collect_view()}
        </div>
    }
}
