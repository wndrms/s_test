use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::symbol::Currency;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrokerEnvironment {
    Real,
    Paper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub broker: String,
    pub environment: BrokerEnvironment,
    pub account_no_masked: String,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerAccount {
    pub broker_connection_id: Uuid,
    pub total_equity: Decimal,
    pub cash: Decimal,
    pub currency: Currency,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerPosition {
    pub symbol_code: String,
    pub quantity: Decimal,
    pub avg_price: Decimal,
    pub current_price: Decimal,
    pub market_value: Decimal,
    pub unrealized_pnl: Decimal,
}

/// 특정 시점의 포트폴리오 스냅샷. portfolio_snapshots 테이블과 매핑된다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    /// 총 평가액 (현금 + 보유 평가액)
    pub equity: Decimal,
    pub cash: Decimal,
    /// 보유 종목 평가액 합계
    pub invested_value: Decimal,
    /// 보유 종목 평가손익 합계
    pub unrealized_pnl: Decimal,
    /// 누적 실현손익
    pub realized_pnl: Decimal,
    pub currency: Currency,
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyingPowerRequest {
    pub symbol_code: String,
    pub price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyingPower {
    pub max_quantity: Decimal,
    pub available_cash: Decimal,
    pub currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrderRequest {
    pub symbol_code: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    pub idempotency_key: String,
    /// 해외 거래소 코드 (NAS, NYS 등). None이면 국내 주문
    #[serde(default)]
    pub market: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrderResponse {
    pub external_order_id: Option<String>,
    pub external_org_no: Option<String>,
    pub status: BrokerOrderStatus,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrokerOrderStatus {
    Submitted,
    Filled,
    Partial,
    Canceled,
    Rejected,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderRequest {
    pub external_order_id: String,
    pub symbol_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFillQuery {
    pub trading_date: chrono::NaiveDate,
    pub symbol_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerFill {
    pub external_order_id: String,
    pub symbol_code: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
}
