use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::user::{SecretKeyRepository, UserRepository};
use lumos_domain::model::user::{SecretKey, SecretKeyRaw, User};

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn create(
        &self,
        email: Option<String>,
        display_name: Option<String>,
        password_hash: Option<String>,
    ) -> Result<User> {
        let row: UserRow = sqlx::query_as::<_, UserRow>(
            r#"INSERT INTO users (email, display_name, password_hash)
               VALUES ($1, $2, $3)
               RETURNING id, email, display_name, created_at, updated_at"#,
        )
        .bind(email)
        .bind(display_name)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}

#[derive(FromRow)]
struct UserRow {
    id: Uuid,
    email: Option<String>,
    display_name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        Self {
            id: r.id,
            email: r.email,
            display_name: r.display_name,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub struct PgSecretKeyRepository {
    pool: PgPool,
}

impl PgSecretKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SecretKeyRepository for PgSecretKeyRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<SecretKey>> {
        let row: Option<SecretKeyRow> = sqlx::query_as::<_, SecretKeyRow>(
            r#"SELECT id, user_id, provider, label, masked_hint, verified_at, created_at, updated_at
               FROM secret_keys WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<SecretKey>> {
        let rows: Vec<SecretKeyRow> = sqlx::query_as::<_, SecretKeyRow>(
            r#"SELECT id, user_id, provider, label, masked_hint, verified_at, created_at, updated_at
               FROM secret_keys WHERE user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_provider(&self, user_id: Uuid, provider: &str) -> Result<Vec<SecretKey>> {
        let rows: Vec<SecretKeyRow> = sqlx::query_as::<_, SecretKeyRow>(
            r#"SELECT id, user_id, provider, label, masked_hint, verified_at, created_at, updated_at
               FROM secret_keys WHERE user_id = $1 AND provider = $2 ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .bind(provider)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create(
        &self,
        user_id: Uuid,
        provider: String,
        label: String,
        encrypted_payload: Vec<u8>,
        masked_hint: Option<String>,
    ) -> Result<SecretKey> {
        let row: SecretKeyRow = sqlx::query_as::<_, SecretKeyRow>(
            r#"INSERT INTO secret_keys (user_id, provider, label, encrypted_payload, masked_hint)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, user_id, provider, label, masked_hint, verified_at, created_at, updated_at"#,
        )
        .bind(user_id)
        .bind(provider)
        .bind(label)
        .bind(encrypted_payload)
        .bind(masked_hint)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM secret_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_raw_by_id(&self, id: Uuid) -> Result<Option<SecretKeyRaw>> {
        #[derive(FromRow)]
        struct RawRow {
            id: Uuid,
            user_id: Uuid,
            provider: String,
            label: String,
            masked_hint: Option<String>,
            verified_at: Option<DateTime<Utc>>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            encrypted_payload: Vec<u8>,
        }

        let row: Option<RawRow> = sqlx::query_as::<_, RawRow>(
            r#"SELECT id, user_id, provider, label, masked_hint, verified_at,
                      created_at, updated_at, encrypted_payload
               FROM secret_keys WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SecretKeyRaw {
            key: SecretKey {
                id: r.id,
                user_id: r.user_id,
                provider: r.provider,
                label: r.label,
                masked_hint: r.masked_hint,
                verified_at: r.verified_at,
                created_at: r.created_at,
                updated_at: r.updated_at,
            },
            encrypted_payload: r.encrypted_payload,
        }))
    }
}

#[derive(FromRow)]
struct SecretKeyRow {
    id: Uuid,
    user_id: Uuid,
    provider: String,
    label: String,
    masked_hint: Option<String>,
    verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SecretKeyRow> for SecretKey {
    fn from(r: SecretKeyRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            provider: r.provider,
            label: r.label,
            masked_hint: r.masked_hint,
            verified_at: r.verified_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

