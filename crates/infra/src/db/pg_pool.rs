use anyhow::{Context, Result};
pub use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")
}
