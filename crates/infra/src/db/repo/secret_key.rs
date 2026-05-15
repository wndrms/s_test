use anyhow::Result;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgSecretKeyRawRepository {
    pool: PgPool,
}

impl PgSecretKeyRawRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the raw encrypted_payload bytes for decryption.
    /// Only used internally — never expose this directly to API responses.
    pub async fn fetch_encrypted_payload(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query("SELECT encrypted_payload FROM secret_keys WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<Vec<u8>, _>("encrypted_payload")))
    }
}
