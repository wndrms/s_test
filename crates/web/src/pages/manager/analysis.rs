use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api::client::{get_analysis_report, list_scenarios};
use crate::api::types::{AnalysisReportDto, ScenarioItemDto};

#[component]
pub fn AnalysisTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let scenarios = LocalResource::new(move || {
        let id = id_str();
        async move {
            let uuid = uuid::Uuid::parse_str(&id).ok()?;
            list_scenarios(uuid).await.ok()
        }
    });

    view! {
        <Suspense fallback=move || view! { <AnalysisSkeleton/> }>
            {move || match scenarios.get().map(|w| (*w).clone()) {
                None => view! { <AnalysisSkeleton/> }.into_any(),
                Some(None) => view! {
                    <div class="empty-state">
                        <span class="empty-icon">"📈"</span>
                        <p>"분석 리포트가 없습니다."</p>
                        <p class="text-muted">"시나리오가 생성되면 분석 리포트를 확인할 수 있습니다."</p>
                    </div>
                }.into_any(),
                Some(Some(list)) if list.is_empty() => view! {
                    <div class="empty-state">
                        <span class="empty-icon">"📈"</span>
                        <p>"분석 리포트가 없습니다."</p>
                        <p class="text-muted">"시나리오가 생성되면 분석 리포트를 확인할 수 있습니다."</p>
                    </div>
                }.into_any(),
                Some(Some(list)) => {
                    let manager_id_str = id_str();
                    // analysis_report_id 기준으로 중복 제거 후 표시
                    let mut seen = std::collections::HashSet::new();
                    let unique: Vec<ScenarioItemDto> = list
                        .into_iter()
                        .filter(|i| {
                            if let Some(rid) = i.analysis_report_id {
                                seen.insert(rid)
                            } else {
                                false
                            }
                        })
                        .collect();

                    if unique.is_empty() {
                        view! {
                            <div class="empty-state">
                                <span class="empty-icon">"📈"</span>
                                <p>"분석 리포트가 없습니다."</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="analysis-list">
                                {unique.into_iter().map(|item| {
                                    let manager_id = manager_id_str.clone();
                                    view! { <AnalysisCard item=item manager_id_str=manager_id/> }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }
            }}
        </Suspense>
    }
}

#[component]
fn AnalysisCard(item: ScenarioItemDto, manager_id_str: String) -> impl IntoView {
    let report_id = item.analysis_report_id.unwrap();
    let manager_id = uuid::Uuid::parse_str(&manager_id_str).ok();
    let open = RwSignal::new(false);
    let report: RwSignal<Option<AnalysisReportDto>> = RwSignal::new(None);

    let on_open = move |_| {
        open.set(true);
        if report.get_untracked().is_none() {
            if let Some(mid) = manager_id {
                leptos::task::spawn_local(async move {
                    if let Ok(r) = get_analysis_report(mid, report_id).await {
                        report.set(Some(r));
                    }
                });
            }
        }
    };

    view! {
        <div class="card card-hover" style="margin-bottom:12px;">
            <div style="display:flex;justify-content:space-between;align-items:center;">
                <div>
                    <span class="badge badge-side" style="margin-right:8px;">{item.symbol_code.clone()}</span>
                    <span style="font-size:0.85rem;color:var(--text-2);">
                        {item.condition_text.chars().take(60).collect::<String>()}
                        {if item.condition_text.len() > 60 { "…" } else { "" }}
                    </span>
                </div>
                <button class="btn btn-sm btn-secondary" on:click=on_open>
                    "분석 보기"
                </button>
            </div>
        </div>

        {move || if open.get() {
            view! {
                <ReportModal
                    report=report
                    on_close=move |_| open.set(false)
                />
            }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
    }
}

#[component]
fn ReportModal(
    report: RwSignal<Option<AnalysisReportDto>>,
    on_close: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <div
            class="modal-backdrop"
            on:click=on_close
            style="position:fixed;inset:0;background:rgba(0,0,0,0.6);z-index:100;display:flex;align-items:center;justify-content:center;padding:16px;"
        >
            <div
                class="card"
                on:click=|e| e.stop_propagation()
                style="max-width:680px;width:100%;max-height:85vh;overflow-y:auto;position:relative;"
            >
                {move || match report.get() {
                    None => view! {
                        <div style="padding:32px;text-align:center;">
                            <div class="skeleton" style="height:20px;width:60%;margin:0 auto 12px;"></div>
                            <div class="skeleton" style="height:80px;width:100%;margin-bottom:12px;"></div>
                            <div class="skeleton" style="height:40px;width:100%;"></div>
                        </div>
                    }.into_any(),
                    Some(r) => view! {
                        <div style="padding:20px;">
                            <div style="display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:16px;">
                                <h3 style="margin:0;font-size:1.1rem;">"분석 리포트"</h3>
                                <div style="font-size:0.75rem;color:var(--text-3);">
                                    {r.analyzed_at.chars().take(16).collect::<String>()}
                                </div>
                            </div>

                            {r.report_summary.clone().map(|s| view! {
                                <div class="card" style="background:var(--surface-2);margin-bottom:16px;padding:12px;">
                                    <p style="margin:0;font-size:0.85rem;line-height:1.6;">{s}</p>
                                </div>
                            })}

                            <div style="margin-bottom:16px;">
                                <h4 style="font-size:0.85rem;color:var(--text-2);margin-bottom:8px;">"상세 분석"</h4>
                                <p style="font-size:0.82rem;line-height:1.7;white-space:pre-wrap;">{r.report_text.clone()}</p>
                            </div>

                            {if !r.annotations.is_empty() {
                                view! {
                                    <div style="margin-bottom:16px;">
                                        <h4 style="font-size:0.85rem;color:var(--text-2);margin-bottom:8px;">"가격 레벨"</h4>
                                        <div style="display:flex;flex-wrap:wrap;gap:8px;">
                                            {r.annotations.iter().map(|a| {
                                                let cls = match a.annotation_type.as_str() {
                                                    "target" => "text-green",
                                                    "stop_loss" => "text-red",
                                                    _ => "",
                                                };
                                                view! {
                                                    <div class="card" style="padding:8px 12px;flex:0 0 auto;">
                                                        <div style="font-size:0.7rem;color:var(--text-3);">{a.label.clone()}</div>
                                                        <div class=cls style="font-size:0.9rem;font-weight:600;">{a.price.to_string()}</div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}

                            {if !r.evidence.is_empty() {
                                view! {
                                    <div>
                                        <h4 style="font-size:0.85rem;color:var(--text-2);margin-bottom:8px;">
                                            {format!("근거 카드 ({})", r.evidence.len())}
                                        </h4>
                                        <div style="display:flex;flex-direction:column;gap:6px;">
                                            {r.evidence.iter().take(5).map(|e| view! {
                                                <div class="card" style="background:var(--surface-2);padding:10px 12px;">
                                                    <div style="display:flex;justify-content:space-between;margin-bottom:4px;">
                                                        <span style="font-size:0.7rem;color:var(--text-3);">{e.source_name.clone()}</span>
                                                        {e.sentiment_label.clone().map(|s| {
                                                            let cls = match s.as_str() {
                                                                "positive" => "text-green",
                                                                "negative" => "text-red",
                                                                _ => "text-muted",
                                                            };
                                                            view! { <span class=cls style="font-size:0.7rem;">{s}</span> }
                                                        })}
                                                    </div>
                                                    <div style="font-size:0.8rem;font-weight:500;">{e.title.clone()}</div>
                                                    <div style="font-size:0.75rem;color:var(--text-2);margin-top:2px;">
                                                        {e.summary.chars().take(80).collect::<String>()}
                                                        {if e.summary.len() > 80 { "…" } else { "" }}
                                                    </div>
                                                </div>
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                        </div>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn AnalysisSkeleton() -> impl IntoView {
    view! {
        <div>
            {(0..3).map(|_| view! {
                <div class="skeleton" style="height:64px;width:100%;border-radius:8px;margin-bottom:12px;"></div>
            }).collect_view()}
        </div>
    }
}
