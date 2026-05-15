use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::manager::{Manager, ManagerMode, ManagerStatus};
use lumos_domain::model::risk::RiskPolicy;
use lumos_domain::model::symbol::{Currency, Region};

pub struct CreateManagerInput {
    pub user_id: Uuid,
    pub broker_connection_id: Uuid,
    pub name: String,
    pub mode: ManagerMode,
    pub region: Region,
    pub base_currency: Currency,
    pub initial_capital: rust_decimal::Decimal,
}

#[async_trait]
pub trait ManagerRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Manager>>;
    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<Manager>>;
    async fn find_active(&self) -> Result<Vec<Manager>>;
    async fn create(&self, input: CreateManagerInput) -> Result<Manager>;
    async fn update_status(&self, id: Uuid, status: ManagerStatus) -> Result<Manager>;
    async fn set_auto_trade(&self, id: Uuid, enabled: bool) -> Result<Manager>;
}

#[async_trait]
pub trait RiskPolicyRepository: Send + Sync {
    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Option<RiskPolicy>>;
    async fn upsert(&self, policy: RiskPolicy) -> Result<RiskPolicy>;
}
