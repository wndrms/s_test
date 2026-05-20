use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// 요청 extensions에 삽입되는 인증된 사용자 정보
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
}

/// Axum extractor — `AuthUser`를 핸들러 인자로 직접 사용
#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, axum::Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthUser>().cloned().ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({"error": "unauthorized"})),
            )
        })
    }
}

/// 자동 인증 미들웨어 - 환경 변수 또는 기본 사용자로 자동 설정
pub async fn auto_auth_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, axum::Json<serde_json::Value>)> {
    // 환경 변수에서 기본 사용자 ID 읽기, 없으면 고정된 UUID 사용
    let default_user_id = std::env::var("DEFAULT_USER_ID")
        .ok()
        .and_then(|id| id.parse::<Uuid>().ok())
        .unwrap_or_else(|| {
            // 고정된 기본 사용자 ID (개발 환경용)
            Uuid::parse_str("00000000-0000-0000-0000-000000000001")
                .expect("valid hardcoded UUID")
        });

    req.extensions_mut().insert(AuthUser {
        user_id: default_user_id,
    });
    Ok(next.run(req).await)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_user_extractor_works() {
        let user_id = Uuid::new_v4();
        let auth_user = AuthUser { user_id };
        assert_eq!(auth_user.user_id, user_id);
    }
}
