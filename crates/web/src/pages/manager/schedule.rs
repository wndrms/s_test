use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::client::{get_schedule, save_schedule};
use crate::api::types::{SlotRequest, UpsertScheduleRequest};

#[derive(Clone)]
struct SlotState {
    /// 표시용 라벨 ("09:00", "09:30 ET")
    label: String,
    /// API용 "HH:MM:SS"
    time_value: String,
    // 활성화 시 시나리오 생성 → 매매를 하나의 사이클로 실행한다.
    enabled: RwSignal<bool>,
}

/// "09:05" → "09:05:00"
fn to_api_time(hhmm: &str) -> String {
    format!("{hhmm}:00")
}

fn generate_krx_slots() -> Vec<SlotState> {
    let mut slots = vec![];
    let mut h = 9u32;
    let mut m = 0u32;
    loop {
        let hhmm = format!("{:02}:{:02}", h, m);
        slots.push(SlotState {
            label: hhmm.clone(),
            time_value: to_api_time(&hhmm),
            enabled: RwSignal::new(false),
        });
        m += 5;
        if m >= 60 { m = 0; h += 1; }
        if h > 15 || (h == 15 && m > 30) { break; }
    }
    slots
}

#[component]
pub fn ScheduleTab() -> impl IntoView {
    let params = use_params_map();
    let id_str = move || params.with(|p| p.get("id").unwrap_or_default());

    let saving = RwSignal::new(false);
    let message = RwSignal::new(Option::<(bool, String)>::None);

    let krx_slots: Vec<SlotState> = generate_krx_slots();

    // 기존 스케줄을 로드해 체크박스를 초기화한다.
    {
        let krx = krx_slots.clone();
        let id = id_str();
        leptos::task::spawn_local(async move {
            let Ok(uuid) = Uuid::parse_str(&id) else { return };
            if let Ok(Some(sched)) = get_schedule(uuid).await {
                for slot in &sched.slots {
                    if slot.enabled {
                        if let Some(s) = krx.iter().find(|s| s.time_value == slot.time_of_day) {
                            s.enabled.set(true);
                        }
                    }
                }
            }
        });
    }

    let on_save = {
        let krx = krx_slots.clone();
        move |_| {
            let Ok(uuid) = Uuid::parse_str(&id_str()) else { return };
            let slots: Vec<SlotRequest> = krx
                .iter()
                .map(|s| SlotRequest {
                    time_of_day: s.time_value.clone(),
                    enabled: s.enabled.get(),
                })
                .collect();
            let req = UpsertScheduleRequest {
                market: "KRX".to_string(),
                timezone: "Asia/Seoul".to_string(),
                slots,
            };

            saving.set(true);
            message.set(None);
            leptos::task::spawn_local(async move {
                match save_schedule(uuid, &req).await {
                    Ok(_) => message.set(Some((true, "스케줄이 저장되었습니다.".to_string()))),
                    Err(e) => message.set(Some((false, format!("저장 실패: {e}")))),
                }
                saving.set(false);
            });
        }
    };

    view! {
        <div>
            <div style="display:flex;gap:8px;margin-bottom:20px;align-items:center;flex-wrap:wrap;">
                <span class="text-muted" style="font-size:0.8rem;">
                    "스케줄은 KRX 정규장 시간(Asia/Seoul) 기준입니다."
                </span>
            </div>

            <SlotGrid slots=krx_slots.clone()/>

            <div style="margin-top:20px;display:flex;justify-content:flex-end;align-items:center;gap:12px;">
                {move || message.get().map(|(ok, msg)| {
                    let cls = if ok { "alert alert-success" } else { "alert alert-error" };
                    view! { <div class=cls style="margin:0;padding:8px 12px;">{msg}</div> }
                })}
                <button
                    class="btn btn-primary"
                    on:click=on_save
                    prop:disabled=move || saving.get()
                >
                    {move || if saving.get() { "저장 중..." } else { "저장" }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn SlotGrid(slots: Vec<SlotState>) -> impl IntoView {
    view! {
        <div style="overflow-y:auto;max-height:480px;">
            <div class="schedule-grid">
                <div class="schedule-header">"시간"</div>
                <div class="schedule-header">"실행"</div>

                {slots.into_iter().map(|slot| {
                    let label = slot.label.clone();
                    view! {
                        <div class="schedule-time">{label}</div>
                        <div class="schedule-cell">
                            <input type="checkbox"
                                prop:checked=slot.enabled
                                on:change=move |e| slot.enabled.set(event_target_checked(&e))
                            />
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
