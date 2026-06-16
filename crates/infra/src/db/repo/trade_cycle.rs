use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use lumos_app::repo::trade_cycle::{FillQuery, TradeCycleRepository, TradeFillRow};
use lumos_domain::model::trade_cycle::{apply_fill, CycleFill, TradeCycle, TradeCycleStatus};

pub struct PgTradeCycleRepository {
    pool: PgPool,
}

impl PgTradeCycleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct TradeCycleRow {
    id: Uuid,
    manager_id: Uuid,
    symbol_id: Uuid,
    status: String,
    open_quantity: Decimal,
    total_buy_quantity: Decimal,
    total_sell_quantity: Decimal,
    total_buy_amount: Decimal,
    avg_entry_price: Decimal,
    total_sell_amount: Decimal,
    avg_exit_price: Decimal,
    total_fee: Decimal,
    total_tax: Decimal,
    realized_pnl: Decimal,
    fill_count: i32,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl From<TradeCycleRow> for TradeCycle {
    fn from(r: TradeCycleRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            symbol_id: r.symbol_id,
            status: parse_status(&r.status),
            open_quantity: r.open_quantity,
            total_buy_quantity: r.total_buy_quantity,
            total_sell_quantity: r.total_sell_quantity,
            total_buy_amount: r.total_buy_amount,
            avg_entry_price: r.avg_entry_price,
            total_sell_amount: r.total_sell_amount,
            avg_exit_price: r.avg_exit_price,
            total_fee: r.total_fee,
            total_tax: r.total_tax,
            realized_pnl: r.realized_pnl,
            fill_count: r.fill_count,
            opened_at: r.opened_at,
            closed_at: r.closed_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct TradeFillDbRow {
    id: Uuid,
    broker_order_id: Uuid,
    trade_cycle_id: Option<Uuid>,
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
            trade_cycle_id: r.trade_cycle_id,
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

fn parse_status(s: &str) -> TradeCycleStatus {
    match s {
        "closed" => TradeCycleStatus::Closed,
        _ => TradeCycleStatus::Open,
    }
}

fn status_str(s: TradeCycleStatus) -> &'static str {
    match s {
        TradeCycleStatus::Open => "open",
        TradeCycleStatus::Closed => "closed",
    }
}

fn side_str(side: lumos_domain::model::broker::OrderSide) -> &'static str {
    match side {
        lumos_domain::model::broker::OrderSide::Buy => "buy",
        lumos_domain::model::broker::OrderSide::Sell => "sell",
    }
}

const CYCLE_COLS: &str = r#"id, manager_id, symbol_id, status, open_quantity,
    total_buy_quantity, total_sell_quantity, total_buy_amount, avg_entry_price,
    total_sell_amount, avg_exit_price, total_fee, total_tax, realized_pnl,
    fill_count, opened_at, closed_at, updated_at"#;

impl PgTradeCycleRepository {
    /// 트랜잭션 안에서 매니저+종목의 open 사이클을 가져오거나 새로 연다.
    async fn get_or_open_tx(
        tx: &mut Transaction<'_, Postgres>,
        manager_id: Uuid,
        symbol_id: Uuid,
    ) -> Result<TradeCycle> {
        let existing: Option<TradeCycleRow> = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "SELECT {CYCLE_COLS} FROM trade_cycles
             WHERE manager_id = $1 AND symbol_id = $2 AND status = 'open'
             FOR UPDATE
             LIMIT 1"
        ))
        .bind(manager_id)
        .bind(symbol_id)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(row) = existing {
            return Ok(row.into());
        }

        let row: TradeCycleRow = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "INSERT INTO trade_cycles (manager_id, symbol_id)
             VALUES ($1, $2)
             RETURNING {CYCLE_COLS}"
        ))
        .bind(manager_id)
        .bind(symbol_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(row.into())
    }

    async fn update_cycle_tx(
        tx: &mut Transaction<'_, Postgres>,
        cycle: &TradeCycle,
    ) -> Result<TradeCycle> {
        let row: TradeCycleRow = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "UPDATE trade_cycles SET
                status = $2, open_quantity = $3, total_buy_quantity = $4,
                total_sell_quantity = $5, total_buy_amount = $6, avg_entry_price = $7,
                total_sell_amount = $8, avg_exit_price = $9, total_fee = $10,
                total_tax = $11, realized_pnl = $12, fill_count = $13,
                closed_at = $14, updated_at = now()
             WHERE id = $1
             RETURNING {CYCLE_COLS}"
        ))
        .bind(cycle.id)
        .bind(status_str(cycle.status))
        .bind(cycle.open_quantity)
        .bind(cycle.total_buy_quantity)
        .bind(cycle.total_sell_quantity)
        .bind(cycle.total_buy_amount)
        .bind(cycle.avg_entry_price)
        .bind(cycle.total_sell_amount)
        .bind(cycle.avg_exit_price)
        .bind(cycle.total_fee)
        .bind(cycle.total_tax)
        .bind(cycle.realized_pnl)
        .bind(cycle.fill_count)
        .bind(cycle.closed_at)
        .fetch_one(&mut **tx)
        .await?;
        Ok(row.into())
    }
}

#[async_trait]
impl TradeCycleRepository for PgTradeCycleRepository {
    async fn record_fill(
        &self,
        manager_id: Uuid,
        symbol_id: Uuid,
        broker_order_id: Uuid,
        fill: CycleFill,
    ) -> Result<(TradeCycle, TradeFillRow)> {
        let mut tx = self.pool.begin().await?;

        // 1. open 사이클 확보 (행 잠금)
        let cycle = Self::get_or_open_tx(&mut tx, manager_id, symbol_id).await?;

        // 2. 도메인 규칙으로 갱신 후 저장
        let updated = apply_fill(&cycle, &fill);
        let saved_cycle = Self::update_cycle_tx(&mut tx, &updated).await?;

        // 3. trade_fill을 사이클에 연결해 insert
        let fill_row: TradeFillDbRow = sqlx::query_as::<_, TradeFillDbRow>(
            r#"WITH inserted AS (
                   INSERT INTO trade_fills
                       (broker_order_id, symbol_id, side, quantity, price, fee, tax,
                        filled_at, trade_cycle_id)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   RETURNING id, broker_order_id, trade_cycle_id, symbol_id, side,
                             quantity, price, fee, tax, filled_at
               )
               SELECT i.id, i.broker_order_id, i.trade_cycle_id, i.symbol_id, i.side,
                      i.quantity, i.price, i.fee, i.tax, i.filled_at,
                      op.manager_id
               FROM inserted i
               JOIN broker_orders bo ON bo.id = i.broker_order_id
               JOIN order_plans op ON op.id = bo.order_plan_id"#,
        )
        .bind(broker_order_id)
        .bind(symbol_id)
        .bind(side_str(fill.side))
        .bind(fill.quantity)
        .bind(fill.price)
        .bind(fill.fee)
        .bind(fill.tax)
        .bind(fill.filled_at)
        .bind(saved_cycle.id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((saved_cycle, fill_row.into()))
    }

    async fn find_open(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<Option<TradeCycle>> {
        let row: Option<TradeCycleRow> = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "SELECT {CYCLE_COLS} FROM trade_cycles
             WHERE manager_id = $1 AND symbol_id = $2 AND status = 'open'
             LIMIT 1"
        ))
        .bind(manager_id)
        .bind(symbol_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<TradeCycle>> {
        let rows: Vec<TradeCycleRow> = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "SELECT {CYCLE_COLS} FROM trade_cycles
             WHERE manager_id = $1
             ORDER BY opened_at DESC
             LIMIT $2"
        ))
        .bind(manager_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<TradeCycle>> {
        let row: Option<TradeCycleRow> = sqlx::query_as::<_, TradeCycleRow>(&format!(
            "SELECT {CYCLE_COLS} FROM trade_cycles WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn list_fills(&self, manager_id: Uuid, query: FillQuery) -> Result<Vec<TradeFillRow>> {
        let rows: Vec<TradeFillDbRow> = sqlx::query_as::<_, TradeFillDbRow>(
            r#"SELECT tf.id, tf.broker_order_id, tf.trade_cycle_id, tf.symbol_id, tf.side,
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
        .bind(query.from)
        .bind(query.to)
        .bind(query.side)
        .bind(query.limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
