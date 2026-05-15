use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskStatus {
    Pending,
    Approved,
    Rejected,
}

impl std::fmt::Display for RiskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskStatus::Pending => write!(f, "pending"),
            RiskStatus::Approved => write!(f, "approved"),
            RiskStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderPlan {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub scenario_run_id: Option<Uuid>,
    pub scenario_item_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub side: String,
    pub order_type: String,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    pub estimated_amount: Decimal,
    pub ai_reason: Option<String>,
    pub risk_status: RiskStatus,
    pub risk_reject_reason: Option<String>,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateOrderPlanInput {
    pub manager_id: Uuid,
    pub scenario_run_id: Option<Uuid>,
    pub scenario_item_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    pub ai_reason: Option<String>,
    pub risk_status: RiskStatus,
    pub risk_reject_reason: Option<String>,
    pub idempotency_key: String,
}

#[async_trait]
pub trait OrderPlanRepository: Send + Sync {
    async fn create_if_not_exists(&self, input: CreateOrderPlanInput) -> Result<Option<OrderPlan>>;
    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<OrderPlan>>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<OrderPlan>>;
}
