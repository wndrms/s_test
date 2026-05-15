use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::broker::OrderSide;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Limit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPlan {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub scenario_run_id: Option<Uuid>,
    pub scenario_item_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    pub estimated_amount: Decimal,
    pub ai_reason: Option<String>,
    pub risk_status: RiskStatus,
    pub risk_reject_reason: Option<String>,
    pub auto_execution: bool,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeFill {
    pub id: Uuid,
    pub broker_order_id: Uuid,
    pub symbol_id: Uuid,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LedgerType {
    Deposit,
    Withdraw,
    Buy,
    Sell,
    Fee,
    Tax,
    Dividend,
    Adjustment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashLedgerEntry {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub ledger_type: LedgerType,
    pub amount: Decimal,
    pub currency: String,
    pub ref_table: Option<String>,
    pub ref_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
