use leptos::prelude::*;

use crate::api::client::list_managers;
use crate::api::types::ManagerDto;
use crate::components::badge::{ModeBadge, StatusBadge};
use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn ManagersPage() -> impl IntoView {
    let managers = LocalResource::new(|| async { list_managers().await.ok() });

    view! {
        <AppLayout>
            <PageHeader title="매니저 목록">
                <a href="/managers/new" class="btn btn-primary btn-sm">"+ 새 매니저"</a>
            </PageHeader>

            <Suspense fallback=move || view! { <ManagerGridSkeleton/> }>
                {move || match managers.get().map(|w| (*w).clone()) {
                    None => view! { <ManagerGridSkeleton/> }.into_any(),
                    Some(None) => view! {
                        <div class="empty-state">
                            <span class="empty-icon">"⚠️"</span>
                            <p>"매니저 목록을 불러오지 못했습니다."</p>
                        </div>
                    }.into_any(),
                    Some(Some(list)) if list.is_empty() => view! {
                        <div class="empty-state">
                            <span class="empty-icon">"📊"</span>
                            <p>"아직 등록된 매니저가 없습니다."</p>
                            <a href="/managers/new" class="btn btn-primary">"첫 매니저 만들기"</a>
                        </div>
                    }.into_any(),
                    Some(Some(list)) => view! {
                        <div class="manager-grid">
                            {list.into_iter().map(|m: ManagerDto| view! { <ManagerCard manager=m/> }).collect_view()}
                        </div>
                    }.into_any(),
                }}
            </Suspense>
        </AppLayout>
    }
}

#[component]
fn ManagerCard(manager: ManagerDto) -> impl IntoView {
    let id = manager.id;
    let href = format!("/managers/{id}");

    view! {
        <a href=href class="card card-hover manager-card" style="display:block;text-decoration:none;">
            <div class="manager-card-header">
                <div>
                    <div class="manager-card-title">{manager.name.clone()}</div>
                    <div class="manager-card-account text-muted">{manager.region.clone()}</div>
                </div>
                <div class="manager-card-equity">
                    <div class="equity-value">"—"</div>
                    <div class="equity-change text-muted">"잔고 로딩 중"</div>
                </div>
            </div>

            <div class="manager-card-footer">
                <div style="display:flex;gap:8px;">
                    <ModeBadge mode=manager.mode.clone()/>
                    <StatusBadge status=manager.status.clone()/>
                </div>
                <div class="auto-trade-indicator">
                    {if manager.auto_trade_enabled {
                        view! {
                            <span class="dot dot-green"></span>
                            <span class="text-green">"자동매매"</span>
                        }.into_any()
                    } else {
                        view! {
                            <span class="dot dot-gray"></span>
                            <span class="text-muted">"수동"</span>
                        }.into_any()
                    }}
                </div>
            </div>
        </a>
    }
}

#[component]
fn ManagerGridSkeleton() -> impl IntoView {
    view! {
        <div class="manager-grid">
            {(0..3).map(|_| view! {
                <div class="card">
                    <div class="skeleton" style="height:20px;width:60%;margin-bottom:12px;"></div>
                    <div class="skeleton" style="height:14px;width:40%;margin-bottom:24px;"></div>
                    <div class="skeleton" style="height:32px;width:100%;"></div>
                </div>
            }).collect_view()}
        </div>
    }
}
