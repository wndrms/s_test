use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api::client::list_trades;
use crate::api::types::TradeDto;

#[component]
pub fn TradesTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let filter = RwSignal::new("all".to_string());

    let trades = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            list_trades(uuid).await.ok()
        }
    });

    view! {
        <div>
            <div style="display:flex;gap:8px;margin-bottom:16px;">
                <FilterBtn label="전체" value="all" active=filter/>
                <FilterBtn label="매수" value="buy"  active=filter/>
                <FilterBtn label="매도" value="sell" active=filter/>
            </div>

            <Suspense fallback=move || view! { <TradesSkeleton/> }>
                {move || match trades.get().map(|w| (*w).clone()) {
                    None => view! { <TradesSkeleton/> }.into_any(),
                    Some(None) => view! { <p class="text-muted">"거래 내역을 불러오지 못했습니다."</p> }.into_any(),
                    Some(Some(list)) if list.is_empty() => view! {
                        <div class="empty-state">
                            <span class="empty-icon">"📋"</span>
                            <p>"거래 내역이 없습니다."</p>
                        </div>
                    }.into_any(),
                    Some(Some(list)) => {
                        let cur_filter = filter.get();
                        let filtered: Vec<TradeDto> = list.into_iter()
                            .filter(|t| cur_filter == "all" || t.side == cur_filter)
                            .collect();
                        view! {
                            <div class="trade-list">
                                {filtered.into_iter().map(|t: TradeDto| view! { <TradeItem item=t/> }).collect_view()}
                            </div>
                        }.into_any()
                    },
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn FilterBtn(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    active: RwSignal<String>,
) -> impl IntoView {
    let val_clone = value.clone();
    view! {
        <button
            class=move || format!("btn btn-sm {}", if active.get() == val_clone { "btn-primary" } else { "btn-secondary" })
            on:click=move |_| active.set(value.clone())
        >
            {label}
        </button>
    }
}

#[component]
fn TradeItem(item: TradeDto) -> impl IntoView {
    let filled_at = item.filled_at.chars().take(16).collect::<String>();

    view! {
        <div class="trade-item">
            <div class=format!("trade-side {}", item.side.clone())>
                {if item.side == "buy" { "B" } else { "S" }}
            </div>
            <div class="trade-info">
                <div class="trade-symbol">{item.symbol_code.clone()}</div>
                <div class="trade-meta">
                    {format!("{} × {}", item.quantity_str(), item.price_str())}
                    {" · "}
                    {filled_at}
                </div>
            </div>
            <div class="trade-numbers">
                <div class="trade-amount">{item.amount_str()}</div>
            </div>
        </div>
    }
}

#[component]
fn TradesSkeleton() -> impl IntoView {
    view! {
        <div class="trade-list">
            {(0..5).map(|_| view! {
                <div class="skeleton" style="height:64px;width:100%;border-radius:6px;"></div>
            }).collect_view()}
        </div>
    }
}
