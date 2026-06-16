use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::client::list_scenarios;
use crate::api::types::ScenarioItemDto;
use crate::components::badge::{ActionLabel, ScenarioBadge};

#[component]
pub fn ScenariosTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let scenarios = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = Uuid::parse_str(&id).ok()?;
            list_scenarios(uuid).await.ok()
        }
    });

    view! {
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
                        <p class="text-muted">"시나리오는 스케줄에 따라 AI가 자동으로 생성합니다. 스케줄을 설정하세요."</p>
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
