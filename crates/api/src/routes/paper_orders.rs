use axum::{extract::State, routing::post, Json, Router};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_domain::model::broker::OrderSide;

use crate::error::{ApiError, ApiResult};
use crate::routes::order_plans::{build_service, OrderPlanResponse};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(create_paper_order))
}

#[derive(Debug, Deserialize)]
pub struct PaperOrderRequest {
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub limit_price: Decimal,
}

async fn create_paper_order(
    State(state): State<AppState>,
    Json(req): Json<PaperOrderRequest>,
) -> ApiResult<Json<OrderPlanResponse>> {
    let side = match req.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        other => {
            return Err(ApiError::from(AppError::Validation(format!(
                "invalid side '{}': must be 'buy' or 'sell'",
                other
            ))))
        }
    };

    let svc = build_service(&state);
    let plan = svc
        .create_paper_order(req.manager_id, req.symbol_id, side, req.quantity, req.limit_price)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(plan.into()))
}
