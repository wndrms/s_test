use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::error::AppError;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_trades))
}

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub side: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TradeResponse {
    pub id: Uuid,
    pub side: String,
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub amount: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
}

async fn list_trades(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    Query(query): Query<TradesQuery>,
) -> ApiResult<Json<Vec<TradeResponse>>> {
    let limit = query.limit.unwrap_or(50).min(200);
    let side = query.side.as_deref();

    let fills = state
        .trades_repo
        .find_by_manager(manager_id, query.from, query.to, side, limit)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    let symbol_ids: Vec<Uuid> = fills.iter().map(|f| f.symbol_id).collect();
    let symbols = state
        .symbol_repo
        .find_by_ids(&symbol_ids)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    let symbol_map: HashMap<Uuid, _> = symbols.into_iter().map(|s| (s.id, s)).collect();

    let response: Vec<TradeResponse> = fills
        .into_iter()
        .map(|f| TradeResponse {
            id: f.id,
            side: f.side.clone(),
            symbol_id: f.symbol_id,
            symbol_code: symbol_map
                .get(&f.symbol_id)
                .map(|s| s.code.clone())
                .unwrap_or_default(),
            quantity: f.quantity,
            price: f.price,
            amount: f.quantity * f.price,
            fee: f.fee,
            tax: f.tax,
            filled_at: f.filled_at,
        })
        .collect();

    Ok(Json(response))
}
