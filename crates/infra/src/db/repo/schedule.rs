use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::schedule::{ManagerScheduleRepository, ScheduleRunRepository};
use lumos_domain::model::schedule::{
    ManagerSchedule, Market, RunType, ScheduleRun, ScheduleRunStatus, ScheduleSlot,
};

// ─── ManagerSchedule ─────────────────────────────────────────────────────────

pub struct PgManagerScheduleRepository {
    pool: PgPool,
}

impl PgManagerScheduleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ManagerScheduleRow {
    id: Uuid,
    manager_id: Uuid,
    market: String,
    timezone: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ManagerScheduleRow> for ManagerSchedule {
    fn from(r: ManagerScheduleRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            market: match r.market.as_str() {
                "US" => Market::Us,
                _ => Market::Krx,
            },
            timezone: r.timezone,
            enabled: r.enabled,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct ScheduleSlotRow {
    id: Uuid,
    schedule_id: Uuid,
    time_of_day: NaiveTime,
    run_scenario: bool,
    run_trade: bool,
    enabled: bool,
}

impl From<ScheduleSlotRow> for ScheduleSlot {
    fn from(r: ScheduleSlotRow) -> Self {
        Self {
            id: r.id,
            schedule_id: r.schedule_id,
            time_of_day: r.time_of_day,
            run_scenario: r.run_scenario,
            run_trade: r.run_trade,
            enabled: r.enabled,
        }
    }
}

#[async_trait]
impl ManagerScheduleRepository for PgManagerScheduleRepository {
    async fn find_enabled(&self) -> Result<Vec<ManagerSchedule>> {
        let rows: Vec<ManagerScheduleRow> = sqlx::query_as::<_, ManagerScheduleRow>(
            r#"SELECT ms.id, ms.manager_id, ms.market, ms.timezone, ms.enabled,
                      ms.created_at, ms.updated_at
               FROM manager_schedules ms
               JOIN managers m ON m.id = ms.manager_id
               WHERE ms.enabled = true AND m.status = 'active' AND m.auto_trade_enabled = true"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Option<ManagerSchedule>> {
        let row: Option<ManagerScheduleRow> = sqlx::query_as::<_, ManagerScheduleRow>(
            r#"SELECT id, manager_id, market, timezone, enabled, created_at, updated_at
               FROM manager_schedules WHERE manager_id = $1 LIMIT 1"#,
        )
        .bind(manager_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_slots(&self, schedule_id: Uuid) -> Result<Vec<ScheduleSlot>> {
        let rows: Vec<ScheduleSlotRow> = sqlx::query_as::<_, ScheduleSlotRow>(
            r#"SELECT id, schedule_id, time_of_day, run_scenario, run_trade, enabled
               FROM schedule_slots WHERE schedule_id = $1 ORDER BY time_of_day"#,
        )
        .bind(schedule_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ─── ScheduleRun ─────────────────────────────────────────────────────────────

pub struct PgScheduleRunRepository {
    pool: PgPool,
}

impl PgScheduleRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ScheduleRunRow {
    id: Uuid,
    manager_id: Uuid,
    schedule_slot_id: Uuid,
    run_type: String,
    scheduled_for: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    status: String,
    error_message: Option<String>,
    idempotency_key: String,
}

impl From<ScheduleRunRow> for ScheduleRun {
    fn from(r: ScheduleRunRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            schedule_slot_id: r.schedule_slot_id,
            run_type: match r.run_type.as_str() {
                "trade" => RunType::Trade,
                _ => RunType::Scenario,
            },
            scheduled_for: r.scheduled_for,
            started_at: r.started_at,
            finished_at: r.finished_at,
            status: parse_status(&r.status),
            error_message: r.error_message,
            idempotency_key: r.idempotency_key,
        }
    }
}

fn parse_status(s: &str) -> ScheduleRunStatus {
    match s {
        "running" => ScheduleRunStatus::Running,
        "success" => ScheduleRunStatus::Success,
        "failed" => ScheduleRunStatus::Failed,
        "skipped" => ScheduleRunStatus::Skipped,
        _ => ScheduleRunStatus::Pending,
    }
}

fn status_str(s: &ScheduleRunStatus) -> &'static str {
    match s {
        ScheduleRunStatus::Pending => "pending",
        ScheduleRunStatus::Running => "running",
        ScheduleRunStatus::Success => "success",
        ScheduleRunStatus::Failed => "failed",
        ScheduleRunStatus::Skipped => "skipped",
    }
}

#[async_trait]
impl ScheduleRunRepository for PgScheduleRunRepository {
    async fn create_if_not_exists(
        &self,
        manager_id: Uuid,
        schedule_slot_id: Uuid,
        run_type: &str,
        scheduled_for: DateTime<Utc>,
        idempotency_key: &str,
    ) -> Result<Option<ScheduleRun>> {
        let row: Option<ScheduleRunRow> = sqlx::query_as::<_, ScheduleRunRow>(
            r#"INSERT INTO schedule_runs
               (manager_id, schedule_slot_id, run_type, scheduled_for, idempotency_key)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (idempotency_key) DO NOTHING
               RETURNING id, manager_id, schedule_slot_id, run_type, scheduled_for,
                         started_at, finished_at, status, error_message, idempotency_key"#,
        )
        .bind(manager_id)
        .bind(schedule_slot_id)
        .bind(run_type)
        .bind(scheduled_for)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: ScheduleRunStatus,
        error_message: Option<String>,
    ) -> Result<ScheduleRun> {
        let is_terminal = matches!(
            status,
            ScheduleRunStatus::Success | ScheduleRunStatus::Failed | ScheduleRunStatus::Skipped
        );
        let row: ScheduleRunRow = sqlx::query_as::<_, ScheduleRunRow>(
            r#"UPDATE schedule_runs
               SET status = $2,
                   error_message = $3,
                   started_at  = CASE WHEN $4 THEN COALESCE(started_at, now()) ELSE started_at END,
                   finished_at = CASE WHEN $5 THEN now() ELSE finished_at END
               WHERE id = $1
               RETURNING id, manager_id, schedule_slot_id, run_type, scheduled_for,
                         started_at, finished_at, status, error_message, idempotency_key"#,
        )
        .bind(id)
        .bind(status_str(&status))
        .bind(error_message)
        .bind(matches!(status, ScheduleRunStatus::Running))
        .bind(is_terminal)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}
