use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/managers/:id/universe", get(list_manager_symbols))
        .route("/managers/:id/universe", post(set_manager_symbols))
}

#[derive(Debug, Serialize)]
struct ManagerSymbolDto {
    manager_id: Uuid,
    symbol_id: Uuid,
    enabled: bool,
    created_at: String,
}

async fn list_manager_symbols(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    _user: AuthUser,
) -> Result<Json<Vec<ManagerSymbolDto>>, ApiError> {
    let symbols = state
        .manager_universe_repo
        .list_by_manager(manager_id)
        .await
        .map_err(|e| ApiError::from(lumos_app::error::AppError::Internal(e)))?;

    let dtos = symbols
        .into_iter()
        .map(|s| ManagerSymbolDto {
            manager_id: s.manager_id,
            symbol_id: s.symbol_id,
            enabled: s.enabled,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(dtos))
}

#[derive(Debug, Deserialize)]
struct SetSymbolsRequest {
    symbol_ids: Vec<Uuid>,
}

async fn set_manager_symbols(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
    _user: AuthUser,
    Json(req): Json<SetSymbolsRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .manager_universe_repo
        .set_symbols(manager_id, req.symbol_ids)
        .await
        .map_err(|e| ApiError::from(lumos_app::error::AppError::Internal(e)))?;

    Ok(StatusCode::OK)
}
