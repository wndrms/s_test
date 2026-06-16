use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPolicy {
    pub manager_id: Uuid,
    pub max_single_order_amount_krw: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub max_daily_trade_count: i32,
    pub allow_market_order: bool,
    pub allow_pre_market: bool,
    pub allow_after_hours: bool,
    pub require_fresh_quote_seconds: i32,
    pub require_fresh_account_seconds: i32,
    pub min_ai_confidence_pct: Decimal,
    pub min_evidence_count: i32,
    pub updated_at: DateTime<Utc>,
}

impl RiskPolicy {
    pub fn default_for(manager_id: Uuid) -> Self {
        use rust_decimal_macros::dec;
        Self {
            manager_id,
            max_single_order_amount_krw: dec!(1000000),
            max_daily_loss_pct: dec!(2.0),
            max_daily_trade_count: 20,
            allow_market_order: false,
            allow_pre_market: false,
            allow_after_hours: false,
            require_fresh_quote_seconds: 60,
            require_fresh_account_seconds: 60,
            min_ai_confidence_pct: dec!(40.0),
            min_evidence_count: 2,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheckResult {
    pub passed: bool,
    pub reject_reason: Option<String>,
    pub checks: Vec<RiskCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheck {
    pub name: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl RiskCheckResult {
    pub fn pass(checks: Vec<RiskCheck>) -> Self {
        Self {
            passed: true,
            reject_reason: None,
            checks,
        }
    }

    pub fn reject(reason: impl Into<String>, checks: Vec<RiskCheck>) -> Self {
        Self {
            passed: false,
            reject_reason: Some(reason.into()),
            checks,
        }
    }
}
