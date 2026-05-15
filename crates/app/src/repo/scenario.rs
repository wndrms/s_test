use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::scenario::{
    EvidenceCard, EvidenceSourceType, ScenarioItem, ScenarioRun, ScenarioStatus,
};

#[async_trait]
pub trait EvidenceCardRepository: Send + Sync {
    async fn find_for_symbol(
        &self,
        symbol_id: Uuid,
        source_types: &[EvidenceSourceType],
        limit_per_type: u32,
    ) -> Result<Vec<EvidenceCard>>;
    async fn create(&self, card: EvidenceCard) -> Result<EvidenceCard>;
}

pub struct CreateScenarioRunInput {
    pub manager_id: Uuid,
    pub schedule_slot_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: String,
}

#[async_trait]
pub trait ScenarioRunRepository: Send + Sync {
    async fn create(&self, input: CreateScenarioRunInput) -> Result<ScenarioRun>;
    async fn update_status(&self, id: Uuid, status: ScenarioStatus) -> Result<ScenarioRun>;
    async fn find_latest_for_manager(
        &self,
        manager_id: Uuid,
        limit: u32,
    ) -> Result<Vec<ScenarioRun>>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScenarioRun>>;
}

pub struct CreateScenarioItemInput {
    pub scenario_run_id: Uuid,
    pub symbol_id: Uuid,
    pub item: ScenarioItem,
}

#[async_trait]
pub trait ScenarioItemRepository: Send + Sync {
    async fn create_batch(&self, items: Vec<CreateScenarioItemInput>) -> Result<Vec<ScenarioItem>>;
    async fn find_by_run(&self, run_id: Uuid) -> Result<Vec<ScenarioItem>>;
    async fn find_by_run_and_id(&self, item_id: Uuid) -> Result<Option<ScenarioItem>>;
    /// 매니저의 가장 최근 scenario_run에서 Buy/Sell 액션인 항목을 반환
    async fn find_pending_for_manager(&self, manager_id: Uuid) -> Result<Vec<ScenarioItem>>;
}
