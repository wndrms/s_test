use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::broker_connection::BrokerConnectionRepository;
use lumos_domain::model::broker::{BrokerConnection, BrokerEnvironment};

pub struct PgBrokerConnectionRepository {
    pool: PgPool,
}

impl PgBrokerConnectionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct BrokerConnectionRow {
    id: Uuid,
    user_id: Uuid,
    broker: String,
    environment: String,
    account_no_masked: String,
    verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<BrokerConnectionRow> for BrokerConnection {
    fn from(r: BrokerConnectionRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            broker: r.broker,
            environment: match r.environment.as_str() {
                "real" => BrokerEnvironment::Real,
                _ => BrokerEnvironment::Paper,
            },
            account_no_masked: r.account_no_masked,
            verified_at: r.verified_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const SELECT_COLS: &str =
    "id, user_id, broker, environment, account_no_masked, verified_at, created_at, updated_at";

#[async_trait]
impl BrokerConnectionRepository for PgBrokerConnectionRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<BrokerConnection>> {
        let row: Option<BrokerConnectionRow> = sqlx::query_as::<_, BrokerConnectionRow>(&format!(
            "SELECT {SELECT_COLS} FROM broker_connections WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<BrokerConnection>> {
        let rows: Vec<BrokerConnectionRow> = sqlx::query_as::<_, BrokerConnectionRow>(
            &format!("SELECT {SELECT_COLS} FROM broker_connections WHERE user_id = $1 ORDER BY created_at DESC"),
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create(
        &self,
        user_id: Uuid,
        environment: BrokerEnvironment,
        account_no_masked: String,
        account_no_encrypted: Vec<u8>,
        app_key_secret_id: Uuid,
        app_secret_secret_id: Uuid,
    ) -> Result<BrokerConnection> {
        let env_str = match environment {
            BrokerEnvironment::Real => "real",
            BrokerEnvironment::Paper => "paper",
        };
        let row: BrokerConnectionRow = sqlx::query_as::<_, BrokerConnectionRow>(&format!(
            r#"INSERT INTO broker_connections
               (user_id, broker, environment, account_no_masked, account_no_encrypted,
                app_key_secret_id, app_secret_secret_id)
               VALUES ($1, 'kis', $2, $3, $4, $5, $6)
               RETURNING {SELECT_COLS}"#
        ))
        .bind(user_id)
        .bind(env_str)
        .bind(account_no_masked)
        .bind(account_no_encrypted)
        .bind(app_key_secret_id)
        .bind(app_secret_secret_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn set_verified(&self, id: Uuid) -> Result<BrokerConnection> {
        let row: BrokerConnectionRow = sqlx::query_as::<_, BrokerConnectionRow>(&format!(
            r#"UPDATE broker_connections
               SET verified_at = now(), updated_at = now()
               WHERE id = $1
               RETURNING {SELECT_COLS}"#
        ))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_mapping_real() {
        let row = BrokerConnectionRow {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            broker: "kis".to_string(),
            environment: "real".to_string(),
            account_no_masked: "****1234".to_string(),
            verified_at: None,
            created_at: DateTime::default(),
            updated_at: DateTime::default(),
        };
        let conn: BrokerConnection = row.into();
        assert_eq!(conn.environment, BrokerEnvironment::Real);
    }

    #[test]
    fn environment_mapping_paper() {
        let row = BrokerConnectionRow {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            broker: "kis".to_string(),
            environment: "paper".to_string(),
            account_no_masked: "****5678".to_string(),
            verified_at: None,
            created_at: DateTime::default(),
            updated_at: DateTime::default(),
        };
        let conn: BrokerConnection = row.into();
        assert_eq!(conn.environment, BrokerEnvironment::Paper);
    }
}
