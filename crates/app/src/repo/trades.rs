use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TradeFillRow {
    pub id: Uuid,
    pub broker_order_id: Uuid,
    pub symbol_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
    // join된 필드
    pub manager_id: Option<Uuid>,
}

#[async_trait]
pub trait TradesRepository: Send + Sync {
    async fn find_by_manager(
        &self,
        manager_id: Uuid,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        side: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TradeFillRow>>;
}
