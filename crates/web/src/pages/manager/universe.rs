use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::client::{
    fetch_manager_symbols, list_symbols, set_manager_symbols, ManagerSymbolDtoExport as ManagerSymbolDto, SymbolDto,
};

#[component]
pub fn UniverseTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());
    let refresh_trigger = RwSignal::new(0);

    let manager_symbols = LocalResource::new(move || {
        refresh_trigger.get();
        let id = id_str();
        async move {
            let uuid = Uuid::parse_str(&id).ok()?;
            fetch_manager_symbols(uuid).await.ok()
        }
    });

    let all_symbols = LocalResource::new(|| async move { list_symbols().await.ok() });

    let selected_symbol_ids = RwSignal::new(Vec::<Uuid>::new());
    let saving = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    // 기존 선택된 종목 ID 초기화
    Effect::new(move || {
        if let Some(Some(symbols)) = manager_symbols.get().map(|w| (*w).clone()) {
            let ids = symbols.into_iter().map(|s| s.symbol_id).collect();
            selected_symbol_ids.set(ids);
        }
    });

    let on_save = move |_| {
        let Some(manager_id) = Uuid::parse_str(&id_str()).ok() else {
            return;
        };

        let symbol_ids = selected_symbol_ids.get();
        saving.set(true);
        error.set(None);

        leptos::task::spawn_local(async move {
            match set_manager_symbols(manager_id, symbol_ids).await {
                Ok(_) => {
                    refresh_trigger.update(|n| *n += 1);
                }
                Err(e) => {
                    error.set(Some(format!("저장 실패: {}", e)));
                }
            }
            saving.set(false);
        });
    };

    let toggle_symbol = move |symbol_id: Uuid| {
        selected_symbol_ids.update(|ids| {
            if ids.contains(&symbol_id) {
                ids.retain(|id| *id != symbol_id);
            } else {
                ids.push(symbol_id);
            }
        });
    };

    view! {
        <div style="max-width:900px;">
            {move || error.get().map(|e| view! {
                <div class="alert alert-error" style="margin-bottom:16px;">{e}</div>
            })}

            <div class="card" style="margin-bottom:24px;">
                <div class="card-section-title">"종목 선택"</div>
                <p class="text-muted" style="margin-bottom:16px;font-size:14px;">
                    "이 매니저가 분석하고 거래할 종목을 선택하세요."
                </p>

                <Suspense fallback=|| view! { <p class="text-muted">"로딩 중..."</p> }>
                    {move || match all_symbols.get().map(|w| (*w).clone()) {
                        None => view! { <p class="text-muted">"로딩 중..."</p> }.into_any(),
                        Some(None) => view! {
                            <div class="alert alert-error">"종목 목록 로딩 실패"</div>
                        }.into_any(),
                        Some(Some(symbols)) => {
                            let kr_symbols: Vec<_> = symbols.iter().filter(|s| s.region == "KR").cloned().collect();
                            let us_symbols: Vec<_> = symbols.iter().filter(|s| s.region == "US").cloned().collect();

                            view! {
                                <div>
                                    <div style="margin-bottom:24px;">
                                        <div class="font-semibold" style="margin-bottom:12px;display:flex;align-items:center;gap:8px;">
                                            <span class="badge badge-blue">"KR"</span>
                                            "한국 종목"
                                        </div>
                                        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:8px;">
                                            {kr_symbols.into_iter().map(|symbol| {
                                                let symbol_id = symbol.id;
                                                let is_selected = move || selected_symbol_ids.get().contains(&symbol_id);
                                                view! {
                                                    <label
                                                        class="symbol-checkbox"
                                                        class:selected=is_selected
                                                        style="display:flex;align-items:center;gap:8px;padding:12px;border:1px solid var(--border);border-radius:4px;cursor:pointer;"
                                                    >
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=is_selected
                                                            on:change=move |_| toggle_symbol(symbol_id)
                                                        />
                                                        <span class="font-semibold">{symbol.display_code.clone()}</span>
                                                        <span class="text-muted" style="font-size:13px;">
                                                            {symbol.name_ko.clone().or(symbol.name_en.clone()).unwrap_or_default()}
                                                        </span>
                                                    </label>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>

                                    <div>
                                        <div class="font-semibold" style="margin-bottom:12px;display:flex;align-items:center;gap:8px;">
                                            <span class="badge badge-green">"US"</span>
                                            "미국 종목"
                                        </div>
                                        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:8px;">
                                            {us_symbols.into_iter().map(|symbol| {
                                                let symbol_id = symbol.id;
                                                let is_selected = move || selected_symbol_ids.get().contains(&symbol_id);
                                                view! {
                                                    <label
                                                        class="symbol-checkbox"
                                                        class:selected=is_selected
                                                        style="display:flex;align-items:center;gap:8px;padding:12px;border:1px solid var(--border);border-radius:4px;cursor:pointer;"
                                                    >
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=is_selected
                                                            on:change=move |_| toggle_symbol(symbol_id)
                                                        />
                                                        <span class="font-semibold">{symbol.display_code.clone()}</span>
                                                        <span class="text-muted" style="font-size:13px;">
                                                            {symbol.name_en.clone().unwrap_or_default()}
                                                        </span>
                                                    </label>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    }}
                </Suspense>

                <div style="margin-top:24px;padding-top:16px;border-top:1px solid var(--border);">
                    <button
                        class="btn btn-primary btn-sm"
                        prop:disabled=saving
                        on:click=on_save
                    >
                        {move || if saving.get() { "저장 중..." } else { "변경사항 저장" }}
                    </button>
                    <span class="text-muted" style="margin-left:12px;font-size:13px;">
                        {move || format!("{}개 종목 선택됨", selected_symbol_ids.get().len())}
                    </span>
                </div>
            </div>
        </div>
    }
}
