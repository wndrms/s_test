use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub label: String,
    pub masked_hint: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `encrypted_payload`를 포함한 내부 전용 구조체.
/// API 응답으로 직렬화되지 않도록 Serialize를 의도적으로 생략한다.
#[derive(Debug, Clone)]
pub struct SecretKeyRaw {
    pub key: SecretKey,
    pub encrypted_payload: Vec<u8>,
}
