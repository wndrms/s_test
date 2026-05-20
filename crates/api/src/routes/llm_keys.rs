use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use lumos_domain::model::user::SecretKey;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_keys).post(store_key))
        .route("/:key_id", delete(delete_key))
}

#[derive(Debug, Deserialize)]
pub struct StoreKeyRequest {
    /// LLM 프로바이더 식별자. 현재 지원: "openai"
    pub provider: String,
    /// 키를 구분하는 사용자 정의 레이블 (e.g. "회사 계정", "개인 GPT-4")
    pub label: String,
    /// 평문 API 키 (HTTPS 전송, 서버에서 AES-GCM 암호화하여 저장)
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct LlmKeyResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub label: String,
    pub masked_hint: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<SecretKey> for LlmKeyResponse {
    fn from(k: SecretKey) -> Self {
        Self {
            id: k.id,
            user_id: k.user_id,
            provider: k.provider,
            label: k.label,
            masked_hint: k.masked_hint,
            verified_at: k.verified_at,
            created_at: k.created_at,
        }
    }
}

async fn store_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<StoreKeyRequest>,
) -> ApiResult<Json<LlmKeyResponse>> {
    let key = state
        .llm_key_service
        .store(auth_user.user_id, req.provider, req.label, &req.api_key)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(LlmKeyResponse::from(key)))
}

async fn list_keys(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> ApiResult<Json<Vec<LlmKeyResponse>>> {
    let keys = state
        .llm_key_service
        .list(auth_user.user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(keys.into_iter().map(LlmKeyResponse::from).collect()))
}

async fn delete_key(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(key_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state
        .llm_key_service
        .delete(auth_user.user_id, key_id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
