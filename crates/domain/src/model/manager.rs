use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::symbol::{Currency, Region};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManagerMode {
    Paper,
    Live,
}

impl std::fmt::Display for ManagerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerMode::Paper => write!(f, "paper"),
            ManagerMode::Live => write!(f, "live"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManagerStatus {
    Active,
    Paused,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manager {
    pub id: Uuid,
    pub user_id: Uuid,
    pub broker_connection_id: Uuid,
    pub name: String,
    pub mode: ManagerMode,
    pub region: Region,
    pub base_currency: Currency,
    pub initial_capital: Decimal,
    pub auto_trade_enabled: bool,
    pub status: ManagerStatus,
    /// 매니저에 연결된 LLM 키 ID. None이면 서버 기본 LLM(환경변수) 사용.
    pub llm_key_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Manager {
    pub fn is_active(&self) -> bool {
        self.status == ManagerStatus::Active
    }

    pub fn is_paper(&self) -> bool {
        self.mode == ManagerMode::Paper
    }

    pub fn is_live(&self) -> bool {
        self.mode == ManagerMode::Live
    }
}
