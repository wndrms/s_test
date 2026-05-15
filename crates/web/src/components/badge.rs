use leptos::prelude::*;

#[component]
pub fn Badge(#[prop(into)] label: String, #[prop(into)] class: String) -> impl IntoView {
    view! { <span class=format!("badge {}", class)>{label}</span> }
}

#[component]
pub fn ModeBadge(#[prop(into)] mode: String) -> impl IntoView {
    let (label, cls) = if mode == "live" {
        ("LIVE", "badge-live")
    } else {
        ("PAPER", "badge-paper")
    };
    view! { <span class=format!("badge {cls}")>{label}</span> }
}

#[component]
pub fn StatusBadge(#[prop(into)] status: String) -> impl IntoView {
    let (label, cls) = match status.as_str() {
        "active" => ("Active", "badge-active"),
        "paused" => ("Paused", "badge-paused"),
        _ => ("Deleted", "badge-bear"),
    };
    view! { <span class=format!("badge {cls}")>{label}</span> }
}

#[component]
pub fn ScenarioBadge(#[prop(into)] scenario_type: String) -> impl IntoView {
    let (label, cls) = match scenario_type.as_str() {
        "bullish" => ("▲ Bullish", "badge-bull"),
        "bearish" => ("▼ Bearish", "badge-bear"),
        _ => ("→ Sideways", "badge-side"),
    };
    view! { <span class=format!("badge {cls}")>{label}</span> }
}

#[component]
pub fn ActionLabel(#[prop(into)] action: String) -> impl IntoView {
    let (label, cls) = match action.as_str() {
        "buy" => ("BUY", "action-buy"),
        "sell" => ("SELL", "action-sell"),
        "hold" => ("HOLD", "action-hold"),
        _ => ("WATCH", "action-watch"),
    };
    view! { <span class=format!("font-weight: 700; {cls}")>{label}</span> }
}
