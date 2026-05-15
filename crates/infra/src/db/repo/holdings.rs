use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::holdings::{HoldingsRepository, PositionRow};

pub struct PgHoldingsRepository {
    pool: PgPool,
}

impl PgHoldingsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct PositionDbRow {
    id: Uuid,
    manager_id: Uuid,
    symbol_id: Uuid,
    quantity: Decimal,
    avg_price: Decimal,
    current_price: Option<Decimal>,
    market_value: Option<Decimal>,
    unrealized_pnl: Option<Decimal>,
    updated_at: DateTime<Utc>,
}

impl From<PositionDbRow> for PositionRow {
    fn from(r: PositionDbRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            symbol_id: r.symbol_id,
            quantity: r.quantity,
            avg_price: r.avg_price,
            current_price: r.current_price,
            market_value: r.market_value,
            unrealized_pnl: r.unrealized_pnl,
            updated_at: r.updated_at,
        }
    }
}

#[async_trait]
impl HoldingsRepository for PgHoldingsRepository {
    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Vec<PositionRow>> {
        let rows: Vec<PositionDbRow> = sqlx::query_as::<_, PositionDbRow>(
            r#"SELECT id, manager_id, symbol_id, quantity, avg_price,
                      current_price, market_value, unrealized_pnl, updated_at
               FROM positions
               WHERE manager_id = $1
               ORDER BY market_value DESC NULLS LAST"#,
        )
        .bind(manager_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn upsert(&self, row: PositionRow) -> Result<PositionRow> {
        let result: PositionDbRow = sqlx::query_as::<_, PositionDbRow>(
            r#"INSERT INTO positions
               (manager_id, symbol_id, quantity, avg_price, current_price, market_value, unrealized_pnl)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               ON CONFLICT (manager_id, symbol_id) DO UPDATE SET
                 quantity = EXCLUDED.quantity,
                 avg_price = EXCLUDED.avg_price,
                 current_price = EXCLUDED.current_price,
                 market_value = EXCLUDED.market_value,
                 unrealized_pnl = EXCLUDED.unrealized_pnl,
                 updated_at = now()
               RETURNING id, manager_id, symbol_id, quantity, avg_price,
                         current_price, market_value, unrealized_pnl, updated_at"#,
        )
        .bind(row.manager_id)
        .bind(row.symbol_id)
        .bind(row.quantity)
        .bind(row.avg_price)
        .bind(row.current_price)
        .bind(row.market_value)
        .bind(row.unrealized_pnl)
        .fetch_one(&self.pool)
        .await?;
        Ok(result.into())
    }
}
