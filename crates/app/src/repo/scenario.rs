use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::scenario::{
    EvidenceCard, EvidenceSourceType, ScenarioItem, ScenarioOutcome, ScenarioRun, ScenarioStatus,
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
    async fn find_latest_for_manager(&self, manager_id: Uuid, limit: u32) -> Result<Vec<ScenarioRun>>;
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

/// 평가 대상 시나리오 항목 (생성 run 시각 포함).
pub struct EvaluableItem {
    pub item: ScenarioItem,
    pub run_created_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait ScenarioOutcomeRepository: Send + Sync {
    /// 아직 평가되지 않았고, target/stop이 설정된 시나리오 항목을 반환한다.
    /// `created_before`보다 이전에 생성된 run의 항목만 (평가 가능 기간 경과분).
    async fn find_unevaluated(
        &self,
        created_before: chrono::DateTime<chrono::Utc>,
        limit: u32,
    ) -> Result<Vec<EvaluableItem>>;

    async fn create(&self, outcome: ScenarioOutcome) -> Result<ScenarioOutcome>;

    /// 심볼의 최근 평가 결과 (프롬프트 피드백용).
    async fn find_recent_for_symbol(
        &self,
        symbol_id: Uuid,
        limit: u32,
    ) -> Result<Vec<ScenarioOutcome>>;
}
