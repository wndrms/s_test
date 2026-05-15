use leptos::prelude::*;

#[derive(Clone)]
struct SlotState {
    time: String,
    scenario: RwSignal<bool>,
    trade: RwSignal<bool>,
}

fn generate_krx_slots() -> Vec<String> {
    let mut slots = vec![];
    let mut h = 9u32;
    let mut m = 0u32;
    loop {
        slots.push(format!("{:02}:{:02}", h, m));
        m += 5;
        if m >= 60 {
            m = 0;
            h += 1;
        }
        if h > 15 || (h == 15 && m > 30) {
            break;
        }
    }
    slots
}

fn generate_us_slots() -> Vec<String> {
    let mut slots = vec![];
    let mut h = 9u32;
    let mut m = 30u32;
    loop {
        slots.push(format!("{:02}:{:02} ET", h, m));
        m += 5;
        if m >= 60 {
            m = 0;
            h += 1;
        }
        if h > 16 || (h == 16 && m > 0) {
            break;
        }
    }
    slots
}

#[component]
pub fn ScheduleTab() -> impl IntoView {
    let market = RwSignal::new("KRX".to_string());

    let krx_slots: Vec<SlotState> = generate_krx_slots()
        .into_iter()
        .map(|t| SlotState {
            time: t,
            scenario: RwSignal::new(false),
            trade: RwSignal::new(false),
        })
        .collect();

    let us_slots: Vec<SlotState> = generate_us_slots()
        .into_iter()
        .map(|t| SlotState {
            time: t,
            scenario: RwSignal::new(false),
            trade: RwSignal::new(false),
        })
        .collect();

    view! {
        <div>
            <div style="display:flex;gap:8px;margin-bottom:20px;align-items:center;flex-wrap:wrap;">
                <MarketTabBtn label="KRX (서울)" value="KRX" active=market/>
                <MarketTabBtn label="US (뉴욕)" value="US" active=market/>
                <span class="text-muted" style="margin-left:auto;font-size:0.8rem;">
                    "스케줄은 각 거래소 정규장 시간 기준입니다."
                </span>
            </div>

            {move || if market.get() == "KRX" {
                view! { <SlotGrid slots=krx_slots.clone() market="KRX"/> }.into_any()
            } else {
                view! { <SlotGrid slots=us_slots.clone() market="US"/> }.into_any()
            }}

            <div style="margin-top:20px;display:flex;justify-content:flex-end;">
                <button class="btn btn-primary">"저장"</button>
            </div>
        </div>
    }
}

#[component]
fn MarketTabBtn(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    active: RwSignal<String>,
) -> impl IntoView {
    let val = value.clone();
    view! {
        <button
            class=move || format!("btn btn-sm {}", if active.get() == val { "btn-primary" } else { "btn-secondary" })
            on:click=move |_| active.set(value.clone())
        >
            {label}
        </button>
    }
}

#[component]
fn SlotGrid(slots: Vec<SlotState>, #[prop(into)] market: String) -> impl IntoView {
    let _ = market;
    view! {
        <div style="overflow-y:auto;max-height:480px;">
            <div class="schedule-grid">
                <div class="schedule-header">"시간"</div>
                <div class="schedule-header">"시나리오"</div>
                <div class="schedule-header">"매매"</div>

                {slots.into_iter().map(|slot| {
                    let time = slot.time.clone();
                    view! {
                        <div class="schedule-time">{time}</div>
                        <div class="schedule-cell">
                            <input type="checkbox"
                                prop:checked=slot.scenario
                                on:change=move |e| slot.scenario.set(event_target_checked(&e))
                            />
                        </div>
                        <div class="schedule-cell">
                            <input type="checkbox"
                                prop:checked=slot.trade
                                on:change=move |e| slot.trade.set(event_target_checked(&e))
                            />
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
