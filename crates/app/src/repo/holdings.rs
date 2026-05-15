use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PositionRow {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub quantity: Decimal,
    pub avg_price: Decimal,
    pub current_price: Option<Decimal>,
    pub market_value: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait HoldingsRepository: Send + Sync {
    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Vec<PositionRow>>;
    async fn upsert(&self, row: PositionRow) -> Result<PositionRow>;
}
