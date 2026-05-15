use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::audit_log::{AuditLog, AuditLogRepository};

pub struct PgAuditLogRepository {
    pool: PgPool,
}

impl PgAuditLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct AuditLogRow {
    id: Uuid,
    user_id: Option<Uuid>,
    manager_id: Option<Uuid>,
    action: String,
    entity_type: Option<String>,
    entity_id: Option<Uuid>,
    payload_json: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

impl From<AuditLogRow> for AuditLog {
    fn from(r: AuditLogRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            manager_id: r.manager_id,
            action: r.action,
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            payload_json: r.payload_json,
            created_at: r.created_at,
        }
    }
}

#[async_trait]
impl AuditLogRepository for PgAuditLogRepository {
    async fn create(
        &self,
        user_id: Option<Uuid>,
        manager_id: Option<Uuid>,
        action: String,
        entity_type: Option<String>,
        entity_id: Option<Uuid>,
        payload_json: Option<serde_json::Value>,
    ) -> Result<AuditLog> {
        let row: AuditLogRow = sqlx::query_as::<_, AuditLogRow>(
            r#"INSERT INTO audit_logs (user_id, manager_id, action, entity_type, entity_id, payload_json)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, user_id, manager_id, action, entity_type, entity_id, payload_json, created_at"#,
        )
        .bind(user_id)
        .bind(manager_id)
        .bind(action)
        .bind(entity_type)
        .bind(entity_id)
        .bind(payload_json)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<AuditLog>> {
        let rows: Vec<AuditLogRow> = sqlx::query_as::<_, AuditLogRow>(
            r#"SELECT id, user_id, manager_id, action, entity_type, entity_id, payload_json, created_at
               FROM audit_logs WHERE manager_id = $1 ORDER BY created_at DESC LIMIT $2"#,
        )
        .bind(manager_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
