use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::issue_token;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use lumos_app::error::AppError;

pub fn router() -> Router<AppState> {
    Router::new().route("/token", post(issue_dev_token))
}

#[derive(Debug, Deserialize)]
pub struct IssueTokenRequest {
    /// 개발용: 원하는 user_id를 직접 전달. 프로덕션에서는 이 엔드포인트를 비활성화해야 함.
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub user_id: Uuid,
    pub expires_in_hours: i64,
}

async fn issue_dev_token(
    State(state): State<AppState>,
    Json(req): Json<IssueTokenRequest>,
) -> ApiResult<Json<TokenResponse>> {
    // 프로덕션 환경에서는 이 엔드포인트를 제거하고 OAuth2/OIDC로 교체
    if std::env::var("APP_ENV").as_deref() == Ok("production") {
        return Err(ApiError::from(AppError::Forbidden(
            "dev token endpoint disabled in production".to_string(),
        )));
    }

    let user_id = req.user_id.unwrap_or_else(Uuid::new_v4);
    ensure_dev_user(&state, user_id).await?;

    let secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-prod".to_string());
    let ttl_hours: i64 = 24;

    let token = issue_token(user_id, &secret, ttl_hours)
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(Json(TokenResponse {
        token,
        user_id,
        expires_in_hours: ttl_hours,
    }))
}

async fn ensure_dev_user(state: &AppState, user_id: Uuid) -> ApiResult<()> {
    sqlx::query(
        r#"INSERT INTO users (id, display_name)
           VALUES ($1, $2)
           ON CONFLICT (id) DO NOTHING"#,
    )
    .bind(user_id)
    .bind("Development User")
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::from(AppError::Internal(e.into())))?;

    Ok(())
}
