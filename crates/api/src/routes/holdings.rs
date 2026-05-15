use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use lumos_app::error::AppError;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_holdings))
}

#[derive(Debug, Serialize)]
pub struct HoldingResponse {
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub symbol_name: String,
    pub quantity: Decimal,
    pub avg_price: Decimal,
    pub current_price: Option<Decimal>,
    pub market_value: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub unrealized_pnl_pct: Option<f64>,
    pub updated_at: DateTime<Utc>,
}

async fn list_holdings(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
) -> ApiResult<Json<Vec<HoldingResponse>>> {
    let positions = state
        .holdings_repo
        .find_by_manager(manager_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    let symbol_ids: Vec<Uuid> = positions.iter().map(|p| p.symbol_id).collect();
    let symbols = state
        .symbol_repo
        .find_by_ids(&symbol_ids)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    let symbol_map: HashMap<Uuid, _> = symbols.into_iter().map(|s| (s.id, s)).collect();

    let response: Vec<HoldingResponse> = positions
        .into_iter()
        .map(|p| {
            let sym = symbol_map.get(&p.symbol_id);
            let unrealized_pnl_pct = match (p.unrealized_pnl, p.market_value) {
                (Some(pnl), Some(mv)) if !mv.is_zero() => {
                    let cost = mv - pnl;
                    if cost.is_zero() {
                        None
                    } else {
                        (pnl / cost * Decimal::from(100)).try_into().ok()
                    }
                }
                _ => None,
            };

            HoldingResponse {
                symbol_id: p.symbol_id,
                symbol_code: sym.map(|s| s.code.clone()).unwrap_or_default(),
                symbol_name: sym
                    .and_then(|s| s.name_ko.clone().or(s.name_en.clone()))
                    .unwrap_or_default(),
                quantity: p.quantity,
                avg_price: p.avg_price,
                current_price: p.current_price,
                market_value: p.market_value,
                unrealized_pnl: p.unrealized_pnl,
                unrealized_pnl_pct,
                updated_at: p.updated_at,
            }
        })
        .collect();

    Ok(Json(response))
}
