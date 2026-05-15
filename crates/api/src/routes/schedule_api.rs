use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_domain::model::schedule::ScheduleSlot;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_schedule).patch(upsert_schedule))
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ScheduleSlotResponse {
    pub id: Uuid,
    pub time_of_day: NaiveTime,
    pub run_scenario: bool,
    pub run_trade: bool,
    pub enabled: bool,
}

impl From<ScheduleSlot> for ScheduleSlotResponse {
    fn from(s: ScheduleSlot) -> Self {
        Self {
            id: s.id,
            time_of_day: s.time_of_day,
            run_scenario: s.run_scenario,
            run_trade: s.run_trade,
            enabled: s.enabled,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub market: String,
    pub timezone: String,
    pub enabled: bool,
    pub slots: Vec<ScheduleSlotResponse>,
}

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SlotRequest {
    pub time_of_day: NaiveTime,
    pub run_scenario: bool,
    pub run_trade: bool,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertScheduleRequest {
    pub market: String,
    pub timezone: String,
    pub slots: Vec<SlotRequest>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn get_schedule(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
) -> ApiResult<Json<Option<ScheduleResponse>>> {
    let schedule = state
        .schedule_read_repo
        .find_by_manager(manager_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    match schedule {
        None => Ok(Json(None)),
        Some(sched) => {
            let slots = state
                .schedule_read_repo
                .find_slots(sched.id)
                .await
                .map_err(|e| ApiError::from(AppError::Internal(e)))?;

            let market_str = match sched.market {
                lumos_domain::model::schedule::Market::Krx => "KRX",
                lumos_domain::model::schedule::Market::Us => "US",
            };

            Ok(Json(Some(ScheduleResponse {
                id: sched.id,
                manager_id: sched.manager_id,
                market: market_str.to_string(),
                timezone: sched.timezone,
                enabled: sched.enabled,
                slots: slots.into_iter().map(ScheduleSlotResponse::from).collect(),
            })))
        }
    }
}

async fn upsert_schedule(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    Json(req): Json<UpsertScheduleRequest>,
) -> ApiResult<Json<()>> {
    let schedule_id = state
        .schedule_write_repo
        .upsert_schedule(manager_id, &req.market, &req.timezone)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    let times: Vec<NaiveTime> = req.slots.iter().map(|s| s.time_of_day).collect();

    for slot in &req.slots {
        state
            .schedule_write_repo
            .upsert_slot(
                schedule_id,
                slot.time_of_day,
                slot.run_scenario,
                slot.run_trade,
                slot.enabled,
            )
            .await
            .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    }

    state
        .schedule_write_repo
        .disable_slots_not_in(schedule_id, &times)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(Json(()))
}
