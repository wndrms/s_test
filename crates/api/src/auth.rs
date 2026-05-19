use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


/// JWT 클레임
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,   // user_id (UUID 문자열)
    pub exp: i64,      // Unix timestamp
    pub iat: i64,
}

impl Claims {
    pub fn user_id(&self) -> Option<Uuid> {
        self.sub.parse().ok()
    }
}

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
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({"error": "unauthorized"})),
                )
            })
    }
}

/// JWT 검증 미들웨어
pub async fn jwt_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, axum::Json<serde_json::Value>)> {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-prod".to_string());

    let token = extract_bearer(req.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "missing authorization header"})),
        )
    })?;

    let claims = validate_token(token, &jwt_secret).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "invalid or expired token"})),
        )
    })?;

    let user_id = claims.user_id().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "invalid user id in token"})),
        )
    })?;

    req.extensions_mut().insert(AuthUser { user_id });
    Ok(next.run(req).await)
}

pub fn validate_token(token: &str, secret: &str) -> anyhow::Result<Claims> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<Claims>(token, &key, &validation)?;
    Ok(data.claims)
}

pub fn issue_token(user_id: Uuid, secret: &str, ttl_hours: i64) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + ttl_hours * 3600,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    let value = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret";

    #[test]
    fn issue_and_validate_roundtrip() {
        let user_id = Uuid::new_v4();
        let token = issue_token(user_id, SECRET, 1).unwrap();
        let claims = validate_token(&token, SECRET).unwrap();
        assert_eq!(claims.user_id().unwrap(), user_id);
    }

    #[test]
    fn wrong_secret_rejected() {
        let user_id = Uuid::new_v4();
        let token = issue_token(user_id, SECRET, 1).unwrap();
        assert!(validate_token(&token, "wrong-secret").is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let user_id = Uuid::new_v4();
        // ttl -1 → 이미 만료
        let token = issue_token(user_id, SECRET, -1).unwrap();
        assert!(validate_token(&token, SECRET).is_err());
    }
}
