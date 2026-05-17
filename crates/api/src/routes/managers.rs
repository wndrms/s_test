use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthUser;
use lumos_app::error::AppError;
use lumos_app::repo::manager::CreateManagerInput;
use lumos_domain::model::broker::BrokerEnvironment;
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
    #[serde(default)]
    pub broker_connection_id: Option<Uuid>,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub initial_capital: rust_decimal::Decimal,
}

async fn list_managers(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> ApiResult<Json<Vec<ManagerResponse>>> {
    let managers = state
        .manager_service
        .list_for_user(auth_user.user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        managers.into_iter().map(ManagerResponse::from).collect(),
    ))
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
    auth_user: AuthUser,
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
    let broker_connection_id =
        resolve_broker_connection_id(&state, auth_user.user_id, req.broker_connection_id).await?;

    let input = CreateManagerInput {
        user_id: auth_user.user_id,
        broker_connection_id,
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

async fn resolve_broker_connection_id(
    state: &AppState,
    user_id: Uuid,
    requested_id: Option<Uuid>,
) -> ApiResult<Uuid> {
    if let Some(id) = requested_id.filter(|id| *id != Uuid::nil()) {
        return Ok(id);
    }

    let existing = state
        .broker_connection_repo
        .find_by_user(user_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    if let Some(conn) = existing.into_iter().next() {
        return Ok(conn.id);
    }

    create_env_broker_connection(state, user_id).await
}

async fn create_env_broker_connection(state: &AppState, user_id: Uuid) -> ApiResult<Uuid> {
    let app_key = required_env("KIS_APP_KEY")?;
    let app_secret = required_env("KIS_APP_SECRET")?;
    let account_no = required_env("KIS_ACCOUNT_NO")?;
    let account_product = std::env::var("KIS_ACCOUNT_PRODUCT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "01".to_string());
    let environment = match std::env::var("KIS_ENV")
        .unwrap_or_else(|_| "paper".to_string())
        .to_lowercase()
        .as_str()
    {
        "real" => BrokerEnvironment::Real,
        _ => BrokerEnvironment::Paper,
    };

    let app_key_secret_id = ensure_env_secret(state, user_id, "app_key", &app_key).await?;
    let app_secret_secret_id = ensure_env_secret(state, user_id, "app_secret", &app_secret).await?;
    let account_no_encrypted = state
        .secret_service
        .encrypt_payload(account_no.as_bytes())
        .map_err(ApiError::from)?;
    let account_no_masked = format!(
        "{}-{}",
        state.secret_service.mask(&account_no),
        account_product
    );

    let conn = state
        .broker_connection_repo
        .create(
            user_id,
            environment,
            account_no_masked,
            account_no_encrypted,
            app_key_secret_id,
            app_secret_secret_id,
        )
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(conn.id)
}

async fn ensure_env_secret(
    state: &AppState,
    user_id: Uuid,
    label: &str,
    raw_value: &str,
) -> ApiResult<Uuid> {
    let existing = state.secret_service.list_for_user(user_id).await?;
    if let Some(secret) = existing
        .into_iter()
        .find(|secret| secret.provider == "kis" && secret.label == label)
    {
        return Ok(secret.id);
    }

    let secret = state
        .secret_service
        .store(user_id, "kis".to_string(), label.to_string(), raw_value)
        .await?;
    Ok(secret.id)
}

fn required_env(key: &str) -> ApiResult<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::from(AppError::Validation(format!(
                "{key} is required to create the default KIS broker connection"
            )))
        })
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
