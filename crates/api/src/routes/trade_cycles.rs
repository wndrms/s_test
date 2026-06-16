use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_domain::model::trade_cycle::TradeCycleStatus;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_cycles))
}

#[derive(Debug, Deserialize)]
pub struct CyclesQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TradeCycleResponse {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub status: String,
    pub open_quantity: Decimal,
    pub total_buy_quantity: Decimal,
    pub total_sell_quantity: Decimal,
    pub avg_entry_price: Decimal,
    pub avg_exit_price: Decimal,
    pub realized_pnl: Decimal,
    pub total_fee: Decimal,
    pub total_tax: Decimal,
    pub fill_count: i32,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

async fn list_cycles(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    Query(query): Query<CyclesQuery>,
) -> ApiResult<Json<Vec<TradeCycleResponse>>> {
    let limit = query.limit.unwrap_or(50).min(200);

    let cycles = state
        .trade_cycle_repo
        .find_by_manager(manager_id, limit)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    let symbol_ids: Vec<Uuid> = cycles.iter().map(|c| c.symbol_id).collect();
    let symbols = state
        .symbol_repo
        .find_by_ids(&symbol_ids)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    let symbol_map: HashMap<Uuid, _> = symbols.into_iter().map(|s| (s.id, s)).collect();

    let response: Vec<TradeCycleResponse> = cycles
        .into_iter()
        .map(|c| TradeCycleResponse {
            id: c.id,
            symbol_id: c.symbol_id,
            symbol_code: symbol_map
                .get(&c.symbol_id)
                .map(|s| s.code.clone())
                .unwrap_or_default(),
            status: match c.status {
                TradeCycleStatus::Open => "open".to_string(),
                TradeCycleStatus::Closed => "closed".to_string(),
            },
            open_quantity: c.open_quantity,
            total_buy_quantity: c.total_buy_quantity,
            total_sell_quantity: c.total_sell_quantity,
            avg_entry_price: c.avg_entry_price,
            avg_exit_price: c.avg_exit_price,
            realized_pnl: c.realized_pnl,
            total_fee: c.total_fee,
            total_tax: c.total_tax,
            fill_count: c.fill_count,
            opened_at: c.opened_at,
            closed_at: c.closed_at,
        })
        .collect();

    Ok(Json(response))
}
