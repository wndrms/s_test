use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_app::repo::order_plan::OrderPlan;
use lumos_app::service::order_plan::OrderPlanService;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_order_plans).post(create_order_plan))
        .route("/{plan_id}/execute", post(execute_order_plan))
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderPlanRequest {
    pub scenario_item_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct OrderPlanResponse {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub scenario_item_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    pub estimated_amount: Decimal,
    pub risk_status: String,
    pub risk_reject_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<OrderPlan> for OrderPlanResponse {
    fn from(p: OrderPlan) -> Self {
        Self {
            id: p.id,
            manager_id: p.manager_id,
            scenario_item_id: p.scenario_item_id,
            symbol_id: p.symbol_id,
            side: p.side,
            quantity: p.quantity,
            limit_price: p.limit_price,
            estimated_amount: p.estimated_amount,
            risk_status: p.risk_status.to_string(),
            risk_reject_reason: p.risk_reject_reason,
            created_at: p.created_at,
        }
    }
}

async fn create_order_plan(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    Json(req): Json<CreateOrderPlanRequest>,
) -> ApiResult<Json<OrderPlanResponse>> {
    let svc = build_service(&state);
    let plan = svc
        .create_from_scenario_item(manager_id, req.scenario_item_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(plan.into()))
}

async fn list_order_plans(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
) -> ApiResult<Json<Vec<OrderPlanResponse>>> {
    let svc = build_service(&state);
    let plans = svc
        .list_for_manager(manager_id, 20)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(plans.into_iter().map(Into::into).collect()))
}

async fn execute_order_plan(
    State(state): State<AppState>,
    Path((manager_id, plan_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan = state
        .order_plan_repo
        .find_by_id(plan_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?
        .ok_or_else(|| ApiError::from(AppError::NotFound("order_plan".to_string())))?;

    if plan.manager_id != manager_id {
        return Err(ApiError::from(AppError::Forbidden(
            "manager mismatch".to_string(),
        )));
    }

    let svc = build_service(&state);
    // broker_connection_id는 매니저의 연결 정보에서 가져오는 것이 정석이나
    // 현재 MVP에서는 nil UUID로 placeholder를 사용합니다.
    svc.execute_approved(&plan, uuid::Uuid::nil())
        .await
        .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn build_service(state: &AppState) -> OrderPlanService {
    OrderPlanService::new(
        state.order_plan_repo.clone(),
        state.scenario_item_repo.clone(),
        state.risk_policy_repo.clone(),
    )
    .with_notification(state.notification_service.clone())
}
