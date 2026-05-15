use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::broker::OrderSide;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioType {
    Bullish,
    Sideways,
    Bearish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioAction {
    Buy,
    Sell,
    Hold,
    Watch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioStatus {
    Generated,
    Validated,
    Rejected,
    Executed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataFreshnessLevel {
    Fresh,
    Stale,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SentimentLabel {
    Positive,
    Neutral,
    Negative,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceSourceType {
    Price,
    Technical,
    News,
    Disclosure,
    Financial,
    Community,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCard {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub source_type: EvidenceSourceType,
    pub source_name: String,
    pub source_ref_table: Option<String>,
    pub source_ref_id: Option<Uuid>,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub sentiment_label: Option<SentimentLabel>,
    pub importance_score: Decimal,
    pub reliability_score: Decimal,
    pub as_of: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRun {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub schedule_slot_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: Option<String>,
    pub status: ScenarioStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioItem {
    pub id: Uuid,
    pub scenario_run_id: Uuid,
    pub analysis_report_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub scenario_type: ScenarioType,
    pub action: ScenarioAction,
    pub probability_pct: Decimal,
    pub target_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,
    pub condition_text: String,
    pub strategy_text: String,
    pub risk_text: Option<String>,
    pub rank_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderIntent {
    pub side: OrderSide,
    pub limit_price: Decimal,
    pub max_position_pct_hint: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedAction {
    pub action: ScenarioAction,
    pub reason: String,
    pub confidence_pct: Decimal,
    pub order_intent: Option<OrderIntent>,
}
