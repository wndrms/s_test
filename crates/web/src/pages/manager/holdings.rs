use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api::client::list_holdings;
use crate::api::types::HoldingDto;

#[component]
pub fn HoldingsTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let holdings = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            list_holdings(uuid).await.ok()
        }
    });

    view! {
        <Suspense fallback=move || view! { <HoldingsSkeleton/> }>
            {move || match holdings.get().map(|w| (*w).clone()) {
                None => view! { <HoldingsSkeleton/> }.into_any(),
                Some(None) => view! { <p class="text-muted">"보유 현황을 불러오지 못했습니다."</p> }.into_any(),
                Some(Some(list)) if list.is_empty() => view! {
                    <div class="empty-state">
                        <span class="empty-icon">"📊"</span>
                        <p>"보유 종목이 없습니다."</p>
                    </div>
                }.into_any(),
                Some(Some(list)) => view! {
                    <div class="holdings-table-wrap">
                        <table class="holdings-table">
                            <thead>
                                <tr>
                                    <th style="text-align:left;">"종목"</th>
                                    <th>"수량"</th>
                                    <th>"평균단가"</th>
                                    <th>"현재가"</th>
                                    <th>"평가금액"</th>
                                    <th>"손익"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {list.into_iter().map(|h: HoldingDto| view! { <HoldingRow item=h/> }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }.into_any(),
            }}
        </Suspense>
    }
}

#[component]
fn HoldingRow(item: HoldingDto) -> impl IntoView {
    let pct = item.unrealized_pnl_pct_val();
    let pnl_cls = if pct >= 0.0 { "text-green" } else { "text-red" };
    let pnl_sign = if pct >= 0.0 { "+" } else { "" };

    view! {
        <tr>
            <td>
                <div class="symbol-cell">{item.symbol_code.clone()}</div>
                <div class="name-cell">{item.symbol_name.clone()}</div>
            </td>
            <td>{item.quantity_str()}</td>
            <td>{item.avg_price_str()}</td>
            <td>{item.current_price_str()}</td>
            <td>{item.market_value_str()}</td>
            <td class=pnl_cls>
                {item.unrealized_pnl_str()}
                <div style="font-size:0.75rem;">
                    {format!("{}{:.2}%", pnl_sign, pct)}
                </div>
            </td>
        </tr>
    }
}

#[component]
fn HoldingsSkeleton() -> impl IntoView {
    view! {
        <div>
            {(0..4).map(|_| view! {
                <div class="skeleton" style="height:48px;width:100%;margin-bottom:4px;border-radius:4px;"></div>
            }).collect_view()}
        </div>
    }
}
