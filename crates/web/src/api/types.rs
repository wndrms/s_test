use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolDto {
    pub id: Uuid,
    pub region: String,
    pub market: String,
    pub code: String,
    pub display_code: String,
    pub name_ko: Option<String>,
    pub name_en: Option<String>,
    pub currency: String,
}

impl SymbolDto {
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.name_ko {
            format!("{} ({})", name, self.code)
        } else if let Some(name) = &self.name_en {
            format!("{} ({})", name, self.code)
        } else {
            self.code.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmKeyDto {
    pub id: Uuid,
    pub provider: String,
    pub label: String,
    pub masked_hint: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManagerDto {
    pub id: Uuid,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub auto_trade_enabled: bool,
    pub status: String,
}

impl ManagerDto {
    pub fn is_live(&self) -> bool {
        self.mode == "live"
    }
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPolicyDto {
    pub max_position_pct: String,
    pub max_single_order_amount_krw: String,
    pub max_daily_loss_pct: String,
    pub max_daily_trade_count: i32,
    pub allow_market_order: bool,
    pub min_ai_confidence_pct: String,
    pub min_evidence_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioRunDto {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioItemDto {
    pub id: Uuid,
    pub scenario_run_id: Uuid,
    #[serde(default)]
    pub analysis_report_id: Option<Uuid>,
    pub symbol_id: Uuid,
    #[serde(default)]
    pub symbol_code: String,
    pub scenario_type: String,
    pub action: String,
    pub probability_pct: serde_json::Value,
    pub target_price: Option<serde_json::Value>,
    pub stop_loss_price: Option<serde_json::Value>,
    pub condition_text: String,
    pub strategy_text: String,
    pub risk_text: Option<String>,
    pub rank_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HoldingDto {
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub symbol_name: String,
    pub quantity: serde_json::Value,
    pub avg_price: serde_json::Value,
    pub current_price: Option<serde_json::Value>,
    pub market_value: Option<serde_json::Value>,
    pub unrealized_pnl: Option<serde_json::Value>,
    pub unrealized_pnl_pct: Option<f64>,
    pub updated_at: String,
}

impl HoldingDto {
    pub fn quantity_str(&self) -> String {
        self.quantity.to_string()
    }
    pub fn avg_price_str(&self) -> String {
        self.avg_price.to_string()
    }
    pub fn current_price_str(&self) -> String {
        self.current_price.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())
    }
    pub fn market_value_str(&self) -> String {
        self.market_value.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())
    }
    pub fn unrealized_pnl_str(&self) -> String {
        self.unrealized_pnl.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())
    }
    pub fn unrealized_pnl_pct_val(&self) -> f64 {
        self.unrealized_pnl_pct.unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeDto {
    pub id: Uuid,
    pub side: String,
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub quantity: serde_json::Value,
    pub price: serde_json::Value,
    pub amount: serde_json::Value,
    pub fee: serde_json::Value,
    pub tax: serde_json::Value,
    pub filled_at: String,
}

impl TradeDto {
    pub fn quantity_str(&self) -> String { self.quantity.to_string() }
    pub fn price_str(&self) -> String { self.price.to_string() }
    pub fn amount_str(&self) -> String { self.amount.to_string() }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceCardDto {
    pub id: Uuid,
    pub source_type: String,
    pub source_name: String,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub sentiment_label: Option<String>,
    pub importance_score: serde_json::Value,
    pub reliability_score: serde_json::Value,
    pub as_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChartAnnotationDto {
    pub id: Uuid,
    pub annotation_type: String,
    pub price: serde_json::Value,
    pub label: String,
    pub color_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisReportDto {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub base_price: serde_json::Value,
    pub analyzed_at: String,
    pub report_text: String,
    pub report_summary: Option<String>,
    pub data_freshness_level: Option<String>,
    pub evidence: Vec<EvidenceCardDto>,
    pub annotations: Vec<ChartAnnotationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManagerSymbolDto {
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub enabled: bool,
    pub created_at: String,
}
