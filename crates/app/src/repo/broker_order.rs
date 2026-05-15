use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BrokerOrder {
    pub id: Uuid,
    pub order_plan_id: Uuid,
    pub broker_connection_id: Uuid,
    pub external_order_id: Option<String>,
    pub external_org_no: Option<String>,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateBrokerOrderInput {
    pub order_plan_id: Uuid,
    pub broker_connection_id: Uuid,
    pub external_order_id: Option<String>,
    pub external_org_no: Option<String>,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub raw_response_json: Option<serde_json::Value>,
}

#[async_trait]
pub trait BrokerOrderRepository: Send + Sync {
    async fn create(&self, input: CreateBrokerOrderInput) -> Result<BrokerOrder>;
    async fn find_by_plan(&self, order_plan_id: Uuid) -> Result<Vec<BrokerOrder>>;
}
