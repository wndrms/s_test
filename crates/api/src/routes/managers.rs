use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::repo::manager::CreateManagerInput;
use lumos_domain::model::manager::{Manager, ManagerMode};
use lumos_domain::model::symbol::{Currency, Region};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_managers).post(create_manager))
        .route("/:id", get(get_manager))
        .route("/:id/risk-policy", get(get_risk_policy))
        .route("/:id/auto-trade", post(set_auto_trade))
}

#[derive(Debug, Serialize)]
pub struct ManagerResponse {
    pub id: Uuid,
    pub broker_connection_id: Uuid,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub auto_trade_enabled: bool,
    pub status: String,
}

impl From<Manager> for ManagerResponse {
    fn from(m: Manager) -> Self {
        Self {
            id: m.id,
            broker_connection_id: m.broker_connection_id,
            name: m.name,
            mode: format!("{:?}", m.mode).to_lowercase(),
            region: m.region.to_string(),
            base_currency: m.base_currency.to_string(),
            auto_trade_enabled: m.auto_trade_enabled,
            status: format!("{:?}", m.status).to_lowercase(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateManagerRequest {
    pub broker_connection_id: Uuid,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub initial_capital: rust_decimal::Decimal,
    pub user_id: Uuid, // TODO: replace with JWT claim
}

async fn list_managers(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<ManagerResponse>>> {
    let user_id = Uuid::nil();
    let managers = state
        .manager_service
        .list_for_user(user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(managers.into_iter().map(ManagerResponse::from).collect()))
}

async fn get_manager(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ManagerResponse>> {
    let manager = state
        .manager_service
        .get(id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ManagerResponse::from(manager)))
}

async fn create_manager(
    State(state): State<AppState>,
    Json(req): Json<CreateManagerRequest>,
) -> ApiResult<Json<ManagerResponse>> {
    let mode = match req.mode.as_str() {
        "live" => ManagerMode::Live,
        _ => ManagerMode::Paper,
    };
    let region = match req.region.as_str() {
        "US" => Region::Us,
        _ => Region::Kr,
    };
    let currency = match req.base_currency.as_str() {
        "USD" => Currency::Usd,
        _ => Currency::Krw,
    };

    let input = CreateManagerInput {
        user_id: req.user_id,
        broker_connection_id: req.broker_connection_id,
        name: req.name,
        mode,
        region,
        base_currency: currency,
        initial_capital: req.initial_capital,
    };

    let manager = state
        .manager_service
        .create(input)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ManagerResponse::from(manager)))
}

async fn get_risk_policy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<lumos_domain::model::risk::RiskPolicy>> {
    let policy = state
        .manager_service
        .get_risk_policy(id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(policy))
}

#[derive(Debug, Deserialize)]
pub struct SetAutoTradeRequest {
    pub enabled: bool,
}

async fn set_auto_trade(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetAutoTradeRequest>,
) -> ApiResult<Json<ManagerResponse>> {
    let manager = state
        .manager_service
        .set_auto_trade(id, req.enabled)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ManagerResponse::from(manager)))
}
