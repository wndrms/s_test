use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_domain::model::scenario::{ScenarioAction, ScenarioItem, ScenarioRun, ScenarioType};
use rust_decimal::Decimal;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/runs", post(trigger_run).get(list_runs))
        .route("/runs/:run_id/items", get(list_items))
}

#[derive(Debug, Deserialize)]
pub struct TriggerRunRequest {
    pub symbol_id: Uuid,
    pub schedule_slot_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: String,
    pub base_price: String,
    /// 사용자가 등록한 LLM 키 ID. None이면 서버 기본 LLM 사용.
    #[serde(default)]
    pub llm_key_id: Option<Uuid>,
    /// OpenAI 호환 커스텀 엔드포인트 (로컬 LLM 등). llm_key_id와 함께 사용.
    #[serde(default)]
    pub base_url_override: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TriggerRunResponse {
    pub run_id: Uuid,
}

async fn trigger_run(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(manager_id): Path<Uuid>,
    Json(req): Json<TriggerRunRequest>,
) -> ApiResult<Json<TriggerRunResponse>> {
    let llm_override = if req.llm_key_id.is_some() {
        Some(
            state
                .llm_key_service
                .resolve_provider(
                    auth_user.user_id,
                    req.llm_key_id,
                    &req.model_name,
                    req.base_url_override.as_deref(),
                )
                .await
                .map_err(ApiError::from)?,
        )
    } else {
        None
    };

    let run_id = state
        .scenario_service
        .run_for_symbol(
            manager_id,
            req.symbol_id,
            req.schedule_slot_id,
            req.model_provider,
            req.model_name,
            req.prompt_version,
            req.base_price,
            vec![],
            llm_override,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(TriggerRunResponse { run_id }))
}

#[derive(Debug, Serialize)]
pub struct ScenarioRunResponse {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<ScenarioRun> for ScenarioRunResponse {
    fn from(r: ScenarioRun) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            model_provider: r.model_provider,
            model_name: r.model_name,
            prompt_version: r.prompt_version,
            status: format!("{:?}", r.status).to_lowercase(),
            created_at: r.created_at,
        }
    }
}

async fn list_runs(
    State(state): State<AppState>,
    Path(manager_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ScenarioRunResponse>>> {
    let runs = state
        .scenario_service
        .latest_runs(manager_id, 20)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        runs.into_iter().map(ScenarioRunResponse::from).collect(),
    ))
}

#[derive(Debug, Serialize)]
pub struct ScenarioItemResponse {
    pub id: Uuid,
    pub scenario_run_id: Uuid,
    pub analysis_report_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub scenario_type: String,
    pub action: String,
    pub probability_pct: Decimal,
    pub target_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,
    pub condition_text: String,
    pub strategy_text: String,
    pub risk_text: Option<String>,
    pub rank_order: i32,
}

fn item_to_response(item: ScenarioItem, symbol_code: String) -> ScenarioItemResponse {
    ScenarioItemResponse {
        id: item.id,
        scenario_run_id: item.scenario_run_id,
        analysis_report_id: item.analysis_report_id,
        symbol_id: item.symbol_id,
        symbol_code,
        scenario_type: scenario_type_str(&item.scenario_type),
        action: scenario_action_str(&item.action),
        probability_pct: item.probability_pct,
        target_price: item.target_price,
        stop_loss_price: item.stop_loss_price,
        condition_text: item.condition_text,
        strategy_text: item.strategy_text,
        risk_text: item.risk_text,
        rank_order: item.rank_order,
    }
}

fn scenario_type_str(t: &ScenarioType) -> String {
    match t {
        ScenarioType::Bullish => "bullish",
        ScenarioType::Sideways => "sideways",
        ScenarioType::Bearish => "bearish",
    }
    .to_string()
}

fn scenario_action_str(a: &ScenarioAction) -> String {
    match a {
        ScenarioAction::Buy => "buy",
        ScenarioAction::Sell => "sell",
        ScenarioAction::Hold => "hold",
        ScenarioAction::Watch => "watch",
    }
    .to_string()
}

async fn list_items(
    State(state): State<AppState>,
    Path((_manager_id, run_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<ScenarioItemResponse>>> {
    let items = state
        .scenario_service
        .list_items_for_run(run_id)
        .await
        .map_err(ApiError::from)?;

    let symbol_ids: Vec<Uuid> = items.iter().map(|i| i.symbol_id).collect();
    let symbols = state
        .symbol_repo
        .find_by_ids(&symbol_ids)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    let symbol_map: HashMap<Uuid, String> = symbols.into_iter().map(|s| (s.id, s.code)).collect();

    Ok(Json(
        items
            .into_iter()
            .map(|i| {
                let code = symbol_map.get(&i.symbol_id).cloned().unwrap_or_default();
                item_to_response(i, code)
            })
            .collect(),
    ))
}
