use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::trades::{TradeFillRow, TradesRepository};

pub struct PgTradesRepository {
    pool: PgPool,
}

impl PgTradesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct TradeFillDbRow {
    id: Uuid,
    broker_order_id: Uuid,
    symbol_id: Uuid,
    side: String,
    quantity: Decimal,
    price: Decimal,
    fee: Decimal,
    tax: Decimal,
    filled_at: DateTime<Utc>,
    manager_id: Option<Uuid>,
}

impl From<TradeFillDbRow> for TradeFillRow {
    fn from(r: TradeFillDbRow) -> Self {
        Self {
            id: r.id,
            broker_order_id: r.broker_order_id,
            symbol_id: r.symbol_id,
            side: r.side,
            quantity: r.quantity,
            price: r.price,
            fee: r.fee,
            tax: r.tax,
            filled_at: r.filled_at,
            manager_id: r.manager_id,
        }
    }
}

#[async_trait]
impl TradesRepository for PgTradesRepository {
    async fn find_by_manager(
        &self,
        manager_id: Uuid,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        side: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TradeFillRow>> {
        let rows: Vec<TradeFillDbRow> = sqlx::query_as::<_, TradeFillDbRow>(
            r#"SELECT tf.id, tf.broker_order_id, tf.symbol_id, tf.side,
                      tf.quantity, tf.price, tf.fee, tf.tax, tf.filled_at,
                      op.manager_id
               FROM trade_fills tf
               JOIN broker_orders bo ON bo.id = tf.broker_order_id
               JOIN order_plans op ON op.id = bo.order_plan_id
               WHERE op.manager_id = $1
                 AND ($2::date IS NULL OR tf.filled_at::date >= $2)
                 AND ($3::date IS NULL OR tf.filled_at::date <= $3)
                 AND ($4::text IS NULL OR tf.side = $4)
               ORDER BY tf.filled_at DESC
               LIMIT $5"#,
        )
        .bind(manager_id)
        .bind(from)
        .bind(to)
        .bind(side)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
