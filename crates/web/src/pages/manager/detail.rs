use leptos::prelude::*;
use leptos_router::{components::Outlet, hooks::use_params_map};

use crate::api::client::{get_manager, list_holdings};
use crate::api::types::{format_krw, ManagerDto};
use crate::components::badge::{ModeBadge, StatusBadge};
use crate::components::layout::AppLayout;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Scenario,
    Universe,
    Holdings,
    Trades,
    Analysis,
    Schedule,
    Settings,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Scenario => "시나리오",
            Tab::Universe => "종목",
            Tab::Holdings => "보유",
            Tab::Trades => "거래",
            Tab::Analysis => "분석",
            Tab::Schedule => "스케줄",
            Tab::Settings => "설정",
        }
    }
    fn path_suffix(self) -> &'static str {
        match self {
            Tab::Scenario => "",
            Tab::Universe => "/universe",
            Tab::Holdings => "/holdings",
            Tab::Trades => "/trades",
            Tab::Analysis => "/analysis",
            Tab::Schedule => "/schedule",
            Tab::Settings => "/settings",
        }
    }
}

#[component]
pub fn ManagerDetailPage() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let manager = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            get_manager(uuid).await.ok()
        }
    });

    view! {
        <AppLayout>
            <Suspense fallback=move || view! { <DetailSkeleton/> }>
                {move || match manager.get().map(|w| (*w).clone()) {
                    None => view! { <DetailSkeleton/> }.into_any(),
                    Some(None) => view! {
                        <div class="empty-state">
                            <span class="empty-icon">"⚠"</span>
                            <p>"매니저를 찾을 수 없습니다."</p>
                            <a href="/managers" class="btn btn-secondary">"목록으로"</a>
                        </div>
                    }.into_any(),
                    Some(Some(m)) => view! { <ManagerDetailInner manager=m/> }.into_any(),
                }}
            </Suspense>
        </AppLayout>
    }
}

#[component]
fn ManagerDetailInner(manager: ManagerDto) -> impl IntoView {
    let id = manager.id;
    let active_tab = RwSignal::new(Tab::Scenario);
    let base_currency = manager.base_currency.clone();
    let initial_capital_val = manager.initial_capital_val();

    // 보유 종목을 조회해 순자산(초기자본 + 미실현손익)과 종목 수를 계산한다.
    let holdings = LocalResource::new(move || async move {
        list_holdings(id).await.unwrap_or_default()
    });

    let tab_href = move |tab: Tab| format!("/managers/{id}{}", tab.path_suffix());

    view! {
        <div>
            <div class="manager-detail-header">
                <div class="manager-detail-top">
                    <div>
                        <h2 class="manager-detail-name">{manager.name.clone()}</h2>
                        <div class="manager-detail-meta">
                            <ModeBadge mode=manager.mode.clone()/>
                            <StatusBadge status=manager.status.clone()/>
                            <span class="badge badge-side">{manager.region.clone()}</span>
                        </div>
                    </div>
                    <div style="display:flex;gap:8px;">
                        <AutoTradeToggle manager_id=id enabled=manager.auto_trade_enabled/>
                    </div>
                </div>

                <div class="manager-stats">
                    {move || {
                        let cur = base_currency.clone();
                        let list = holdings.get().map(|w| (*w).clone());
                        let (net_worth, pnl_sum, count) = match &list {
                            Some(hs) => {
                                let pnl: f64 = hs.iter().map(|h| h.unrealized_pnl_val()).sum();
                                // 순자산 ≈ 초기자본 + 미실현손익 (모의/근사)
                                (initial_capital_val + pnl, pnl, hs.len())
                            }
                            None => (initial_capital_val, 0.0, 0),
                        };
                        let pnl_pct = if initial_capital_val > 0.0 {
                            pnl_sum / initial_capital_val * 100.0
                        } else {
                            0.0
                        };
                        let pnl_cls = if pnl_sum >= 0.0 { "stat-value text-green" } else { "stat-value text-red" };
                        let sign = if pnl_sum >= 0.0 { "+" } else { "" };
                        view! {
                            <div class="stat-item">
                                <div class="stat-label">"순자산"</div>
                                <div class="stat-value">{format!("{:.0} {}", net_worth, cur)}</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-label">"평가 손익"</div>
                                <div class=pnl_cls>{format!("{}{:.2}%", sign, pnl_pct)}</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-label">"보유 종목"</div>
                                <div class="stat-value">{count.to_string()}</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-label">"초기 자본"</div>
                                <div class="stat-value">{format_krw(initial_capital_val)}</div>
                            </div>
                        }
                    }}
                </div>
            </div>

            <div class="tabs">
                {[Tab::Scenario, Tab::Universe, Tab::Holdings, Tab::Trades, Tab::Analysis, Tab::Schedule, Tab::Settings]
                    .into_iter()
                    .map(|tab| {
                        let href = tab_href(tab);
                        view! {
                            <a
                                href=href
                                class=move || format!("tab{}", if active_tab.get() == tab { " active" } else { "" })
                                on:click=move |_| active_tab.set(tab)
                            >
                                {tab.label()}
                            </a>
                        }
                    })
                    .collect_view()
                }
            </div>

            <Outlet/>
        </div>
    }
}

#[component]
fn AutoTradeToggle(manager_id: uuid::Uuid, enabled: bool) -> impl IntoView {
    let on = RwSignal::new(enabled);
    let pending = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    view! {
        <div style="display:flex;align-items:center;gap:12px;">
            <span class="text-muted" style="font-size:0.85rem;">"자동매매"</span>
            {move || error.get().map(|e| view! {
                <span class="text-red" style="font-size:0.75rem;">{e}</span>
            })}
            <label
                class="toggle"
                style=move || if pending.get() { "opacity:0.6;pointer-events:none;" } else { "" }
            >
                <input
                    type="checkbox"
                    prop:checked=on
                    on:change=move |e| {
                        let checked = event_target_checked(&e);
                        on.set(checked);
                        pending.set(true);
                        error.set(None);
                        leptos::task::spawn_local(async move {
                            let url = format!("/api/managers/{}/auto-trade", manager_id);
                            let result = gloo_net::http::Request::post(&url)
                                .json(&serde_json::json!({"enabled": checked}))
                                .map(|req| async move { req.send().await })
                                .map_err(|e| e.to_string());
                            let ok = match result {
                                Err(e) => { error.set(Some(e)); false }
                                Ok(fut) => match fut.await {
                                    Err(e) => { error.set(Some(e.to_string())); false }
                                    Ok(resp) if !resp.ok() => {
                                        error.set(Some(format!("요청 실패 ({})", resp.status())));
                                        false
                                    }
                                    Ok(_) => true,
                                }
                            };
                            if !ok {
                                on.set(!checked);
                            }
                            pending.set(false);
                        });
                    }
                />
                <span class="toggle-slider"></span>
            </label>
        </div>
    }
}

#[component]
fn DetailSkeleton() -> impl IntoView {
    view! {
        <div>
            <div class="card" style="margin-bottom:20px;">
                <div class="skeleton" style="height:28px;width:40%;margin-bottom:12px;"></div>
                <div class="skeleton" style="height:14px;width:60%;margin-bottom:24px;"></div>
                <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:16px;">
                    {(0..4).map(|_| view! {
                        <div>
                            <div class="skeleton" style="height:12px;width:50%;margin-bottom:8px;"></div>
                            <div class="skeleton" style="height:20px;width:70%;"></div>
                        </div>
                    }).collect_view()}
                </div>
            </div>
            <div class="skeleton" style="height:40px;width:100%;margin-bottom:20px;"></div>
        </div>
    }
}
