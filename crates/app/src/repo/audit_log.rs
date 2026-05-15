use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub manager_id: Option<Uuid>,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub payload_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn create(
        &self,
        user_id: Option<Uuid>,
        manager_id: Option<Uuid>,
        action: String,
        entity_type: Option<String>,
        entity_id: Option<Uuid>,
        payload_json: Option<serde_json::Value>,
    ) -> Result<AuditLog>;

    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<AuditLog>>;
}
