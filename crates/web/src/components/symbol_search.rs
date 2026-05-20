use leptos::prelude::*;

use crate::api::client::{search_symbols, SymbolDto};

#[component]
pub fn SymbolSearch<F>(
    on_select: F,
    #[prop(optional)] placeholder: Option<String>,
) -> impl IntoView
where
    F: Fn(SymbolDto) + 'static + Copy + Send,
{
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<SymbolDto>::new());
    let searching = RwSignal::new(false);
    let show_results = RwSignal::new(false);
    let selected_region = RwSignal::new(None::<String>);

    let search = move || {
        let q = query.get();
        if q.len() < 2 {
            results.set(vec![]);
            show_results.set(false);
            return;
        }

        searching.set(true);
        let region = selected_region.get();

        leptos::task::spawn_local(async move {
            match search_symbols(&q, region.as_deref()).await {
                Ok(symbols) => {
                    results.set(symbols);
                    show_results.set(true);
                }
                Err(_) => {
                    results.set(vec![]);
                }
            }
            searching.set(false);
        });
    };

    let on_input = move |ev| {
        query.set(event_target_value(&ev));
        search();
    };

    let on_symbol_click = move |symbol: SymbolDto| {
        on_select(symbol.clone());
        query.set(String::new());
        results.set(vec![]);
        show_results.set(false);
    };

    view! {
        <div style="position:relative;">
            <div style="display:flex;gap:8px;margin-bottom:8px;">
                <input
                    type="text"
                    class="form-input"
                    placeholder=placeholder.unwrap_or_else(|| "종목 검색 (코드 또는 이름)".to_string())
                    on:input=on_input
                    on:focus=move |_| {
                        if !results.get().is_empty() {
                            show_results.set(true);
                        }
                    }
                    prop:value=move || query.get()
                />
                <select
                    class="form-input"
                    style="width:120px;"
                    on:change=move |ev| {
                        let val = event_target_value(&ev);
                        selected_region.set(if val.is_empty() { None } else { Some(val) });
                        search();
                    }
                >
                    <option value="">"전체"</option>
                    <option value="KR">"한국"</option>
                    <option value="US">"미국"</option>
                </select>
            </div>

            {move || show_results.get().then(|| {
                let symbols = results.get();
                if searching.get() {
                    view! {
                        <div style="position:absolute;top:100%;left:0;right:0;background:var(--bg-1);border:1px solid var(--border);border-radius:4px;margin-top:4px;max-height:300px;overflow-y:auto;z-index:100;">
                            <div class="text-muted" style="padding:12px;">"검색 중..."</div>
                        </div>
                    }.into_any()
                } else if symbols.is_empty() {
                    view! {
                        <div style="position:absolute;top:100%;left:0;right:0;background:var(--bg-1);border:1px solid var(--border);border-radius:4px;margin-top:4px;max-height:300px;overflow-y:auto;z-index:100;">
                            <div class="text-muted" style="padding:12px;">"검색 결과가 없습니다"</div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div style="position:absolute;top:100%;left:0;right:0;background:var(--bg-1);border:1px solid var(--border);border-radius:4px;margin-top:4px;max-height:300px;overflow-y:auto;z-index:100;">
                            {symbols.into_iter().map(|symbol| {
                                view! {
                                    <div
                                        style="padding:12px;cursor:pointer;border-bottom:1px solid var(--border);display:flex;justify-content:space-between;align-items:center;"
                                        on:click=move |_| on_symbol_click(symbol.clone())
                                    >
                                        <div style="display:flex;align-items:center;gap:8px;">
                                            <span class=format!("badge badge-{}", if symbol.region == "KR" { "blue" } else { "green" })>
                                                {symbol.region.clone()}
                                            </span>
                                            <span class="font-semibold">{symbol.display_code.clone()}</span>
                                            <span class="text-muted" style="font-size:13px;">
                                                {symbol.name_ko.clone().or(symbol.name_en.clone()).unwrap_or_default()}
                                            </span>
                                        </div>
                                        <span class="text-muted" style="font-size:12px;">
                                            {symbol.market.clone()}
                                        </span>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            })}
        </div>
    }
}
