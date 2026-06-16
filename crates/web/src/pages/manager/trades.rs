use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api::client::{list_trade_cycles, list_trades};
use crate::api::types::{format_krw, TradeCycleDto, TradeDto};

#[derive(Clone, Copy, PartialEq)]
enum TradeTab {
    Fills,
    Cycles,
}

#[component]
pub fn TradesTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());
    let active_tab = RwSignal::new(TradeTab::Fills);
    let fill_filter = RwSignal::new("all".to_string());

    let trades = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            list_trades(uuid).await.ok()
        }
    });

    let cycles = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            list_trade_cycles(uuid).await.ok()
        }
    });

    view! {
        <div>
            <div style="display:flex;gap:8px;margin-bottom:16px;">
                <SubTabBtn label="체결 내역" tab=TradeTab::Fills active=active_tab/>
                <SubTabBtn label="매매 사이클" tab=TradeTab::Cycles active=active_tab/>
            </div>

            {move || match active_tab.get() {
                TradeTab::Fills => view! {
                    <div>
                        <div style="display:flex;gap:8px;margin-bottom:12px;">
                            <FilterBtn label="전체" value="all" active=fill_filter/>
                            <FilterBtn label="매수" value="buy"  active=fill_filter/>
                            <FilterBtn label="매도" value="sell" active=fill_filter/>
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
                                    let cur_filter = fill_filter.get();
                                    let filtered: Vec<TradeDto> = list.into_iter()
                                        .filter(|t| cur_filter == "all" || t.side == cur_filter)
                                        .collect();
                                    view! {
                                        <div class="trade-list">
                                            {filtered.into_iter().map(|t| view! { <TradeItem item=t/> }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </Suspense>
                    </div>
                }.into_any(),

                TradeTab::Cycles => view! {
                    <Suspense fallback=move || view! { <TradesSkeleton/> }>
                        {move || match cycles.get().map(|w| (*w).clone()) {
                            None => view! { <TradesSkeleton/> }.into_any(),
                            Some(None) => view! { <p class="text-muted">"매매 사이클을 불러오지 못했습니다."</p> }.into_any(),
                            Some(Some(list)) if list.is_empty() => view! {
                                <div class="empty-state">
                                    <span class="empty-icon">"🔄"</span>
                                    <p>"매매 사이클이 없습니다."</p>
                                    <p class="text-muted">"자동매매가 실행되면 종목별 매수→매도 사이클이 기록됩니다."</p>
                                </div>
                            }.into_any(),
                            Some(Some(list)) => view! {
                                <div class="trade-list">
                                    {list.into_iter().map(|c| view! { <CycleItem item=c/> }).collect_view()}
                                </div>
                            }.into_any(),
                        }}
                    </Suspense>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn SubTabBtn(
    #[prop(into)] label: String,
    tab: TradeTab,
    active: RwSignal<TradeTab>,
) -> impl IntoView {
    view! {
        <button
            class=move || format!("btn btn-sm {}", if active.get() == tab { "btn-primary" } else { "btn-secondary" })
            on:click=move |_| active.set(tab)
        >
            {label}
        </button>
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
fn CycleItem(item: TradeCycleDto) -> impl IntoView {
    let pnl = item.realized_pnl_val();
    let pnl_cls = if pnl >= 0.0 { "text-green" } else { "text-red" };
    let pnl_sign = if pnl >= 0.0 { "+" } else { "" };
    let status_cls = if item.is_open() { "dot dot-green" } else { "dot dot-gray" };
    let opened = item.opened_at.chars().take(16).collect::<String>();
    let closed = item.closed_at.as_deref().map(|s| s.chars().take(16).collect::<String>());

    view! {
        <div class="trade-item">
            <div style="display:flex;align-items:center;gap:6px;min-width:48px;">
                <span class=status_cls></span>
                <span style="font-size:0.75rem;color:var(--text-3);">
                    {if item.is_open() { "진행" } else { "종료" }}
                </span>
            </div>
            <div class="trade-info" style="flex:1;">
                <div class="trade-symbol">{item.symbol_code.clone()}</div>
                <div class="trade-meta">
                    {format!("진입 {} · 청산 {} · {}회 체결",
                        item.avg_entry_str(),
                        if item.is_open() { "—".to_string() } else { item.avg_exit_str() },
                        item.fill_count
                    )}
                </div>
                <div class="trade-meta">
                    {format!("시작 {}", opened)}
                    {closed.map(|c| format!(" → {}", c)).unwrap_or_default()}
                </div>
            </div>
            <div class="trade-numbers">
                <div class=pnl_cls style="font-weight:600;">
                    {format!("{}{}", pnl_sign, format_krw(pnl))}
                </div>
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
