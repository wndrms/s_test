use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Market {
    Krx,
    Us,
}

impl Market {
    pub fn timezone(&self) -> &'static str {
        match self {
            Market::Krx => "Asia/Seoul",
            Market::Us => "America/New_York",
        }
    }

    /// 거래 시간 (NaiveTime, 현지 기준)
    pub fn trading_hours(&self) -> (NaiveTime, NaiveTime) {
        match self {
            Market::Krx => (
                NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
            ),
            Market::Us => (
                NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleRunStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerSchedule {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub market: Market,
    pub timezone: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleSlot {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub time_of_day: NaiveTime,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRun {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub schedule_slot_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ScheduleRunStatus,
    pub error_message: Option<String>,
    pub idempotency_key: String,
}
