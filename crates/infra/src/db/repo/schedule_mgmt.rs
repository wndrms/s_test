use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveTime;
use sqlx::PgPool;
use uuid::Uuid;

use lumos_app::repo::schedule::ManagerScheduleWriteRepository;

pub struct PgManagerScheduleWriteRepository {
    pool: PgPool,
}

impl PgManagerScheduleWriteRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ManagerScheduleWriteRepository for PgManagerScheduleWriteRepository {
    async fn upsert_schedule(
        &self,
        manager_id: Uuid,
        market: &str,
        timezone: &str,
    ) -> Result<Uuid> {
        // manager_schedules에 UNIQUE(manager_id) 제약이 없을 수 있으므로
        // SELECT 후 없으면 INSERT, 있으면 UPDATE 방식으로 구현
        let existing: Option<(Uuid,)> =
            sqlx::query_as::<_, (Uuid,)>(
                "SELECT id FROM manager_schedules WHERE manager_id = $1 LIMIT 1",
            )
            .bind(manager_id)
            .fetch_optional(&self.pool)
            .await?;

        match existing {
            Some((id,)) => {
                sqlx::query(
                    "UPDATE manager_schedules SET market = $2, timezone = $3, updated_at = now() WHERE id = $1",
                )
                .bind(id)
                .bind(market)
                .bind(timezone)
                .execute(&self.pool)
                .await?;
                Ok(id)
            }
            None => {
                let (id,): (Uuid,) = sqlx::query_as::<_, (Uuid,)>(
                    r#"INSERT INTO manager_schedules (manager_id, market, timezone)
                       VALUES ($1, $2, $3)
                       RETURNING id"#,
                )
                .bind(manager_id)
                .bind(market)
                .bind(timezone)
                .fetch_one(&self.pool)
                .await?;
                Ok(id)
            }
        }
    }

    async fn upsert_slot(
        &self,
        schedule_id: Uuid,
        time_of_day: NaiveTime,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO schedule_slots (schedule_id, time_of_day, enabled)
               VALUES ($1, $2, $3)
               ON CONFLICT (schedule_id, time_of_day) DO UPDATE SET
                 enabled = EXCLUDED.enabled"#,
        )
        .bind(schedule_id)
        .bind(time_of_day)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn disable_slots_not_in(
        &self,
        schedule_id: Uuid,
        times: &[NaiveTime],
    ) -> Result<()> {
        sqlx::query(
            "UPDATE schedule_slots SET enabled = false WHERE schedule_id = $1 AND time_of_day != ALL($2)",
        )
        .bind(schedule_id)
        .bind(times)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
