use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use lumos_app::repo::manager_universe::ManagerUniverseRepository;
use lumos_domain::model::manager_universe::ManagerSymbol;

pub struct PgManagerUniverseRepository {
    pool: PgPool,
}

impl PgManagerUniverseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ManagerUniverseRepository for PgManagerUniverseRepository {
    async fn list_by_manager(&self, manager_id: Uuid) -> Result<Vec<ManagerSymbol>> {
        let rows = sqlx::query(
            r#"
            SELECT manager_id, symbol_id, enabled, created_at
            FROM manager_universe
            WHERE manager_id = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(manager_id)
        .fetch_all(&self.pool)
        .await?;

        let symbols = rows
            .into_iter()
            .map(|row| ManagerSymbol {
                manager_id: row.get("manager_id"),
                symbol_id: row.get("symbol_id"),
                enabled: row.get("enabled"),
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
            })
            .collect();

        Ok(symbols)
    }

    async fn add_symbol(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO manager_universe (manager_id, symbol_id, enabled)
            VALUES ($1, $2, true)
            ON CONFLICT (manager_id, symbol_id) DO UPDATE SET enabled = true
            "#
        )
        .bind(manager_id)
        .bind(symbol_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn remove_symbol(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM manager_universe
            WHERE manager_id = $1 AND symbol_id = $2
            "#
        )
        .bind(manager_id)
        .bind(symbol_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn set_symbols(&self, manager_id: Uuid, symbol_ids: Vec<Uuid>) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // 기존 종목 모두 삭제
        sqlx::query(
            r#"
            DELETE FROM manager_universe
            WHERE manager_id = $1
            "#
        )
        .bind(manager_id)
        .execute(&mut *tx)
        .await?;

        // 새 종목 추가
        for symbol_id in symbol_ids {
            sqlx::query(
                r#"
                INSERT INTO manager_universe (manager_id, symbol_id, enabled)
                VALUES ($1, $2, true)
                "#
            )
            .bind(manager_id)
            .bind(symbol_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
