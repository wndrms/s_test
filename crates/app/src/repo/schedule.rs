use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use chrono::NaiveTime;
use chrono::Utc;
use uuid::Uuid;

use lumos_domain::model::schedule::{
    ManagerSchedule, ScheduleRun, ScheduleRunStatus, ScheduleSlot,
};

#[async_trait]
pub trait ManagerScheduleRepository: Send + Sync {
    async fn find_enabled(&self) -> Result<Vec<ManagerSchedule>>;
    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Option<ManagerSchedule>>;
    async fn find_slots(&self, schedule_id: Uuid) -> Result<Vec<ScheduleSlot>>;
}

#[async_trait]
pub trait ScheduleRunRepository: Send + Sync {
    /// idempotency_key가 이미 존재하면 None 반환 (중복 실행 방지)
    async fn create_if_not_exists(
        &self,
        manager_id: Uuid,
        schedule_slot_id: Uuid,
        run_type: &str,
        scheduled_for: DateTime<Utc>,
        idempotency_key: &str,
    ) -> Result<Option<ScheduleRun>>;

    async fn update_status(
        &self,
        id: Uuid,
        status: ScheduleRunStatus,
        error_message: Option<String>,
    ) -> Result<ScheduleRun>;
}

#[async_trait]
pub trait ManagerScheduleWriteRepository: Send + Sync {
    /// schedule_id 반환. manager_id당 유일한 스케줄을 upsert.
    async fn upsert_schedule(&self, manager_id: Uuid, market: &str, timezone: &str)
        -> Result<Uuid>;

    async fn upsert_slot(
        &self,
        schedule_id: Uuid,
        time_of_day: NaiveTime,
        run_scenario: bool,
        run_trade: bool,
        enabled: bool,
    ) -> Result<()>;

    /// 주어진 times 목록에 없는 슬롯을 disabled로 처리
    async fn disable_slots_not_in(&self, schedule_id: Uuid, times: &[NaiveTime]) -> Result<()>;
}
