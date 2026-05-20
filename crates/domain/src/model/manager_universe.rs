use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerSymbol {
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
