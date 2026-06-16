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

// ─── Schedule ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduleSlotDto {
    pub id: Uuid,
    /// "HH:MM:SS" 형식
    pub time_of_day: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduleDto {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub market: String,
    pub timezone: String,
    pub enabled: bool,
    pub slots: Vec<ScheduleSlotDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlotRequest {
    /// "HH:MM:SS" 형식
    pub time_of_day: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpsertScheduleRequest {
    pub market: String,
    pub timezone: String,
    pub slots: Vec<SlotRequest>,
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
    #[serde(default)]
    pub initial_capital: Option<serde_json::Value>,
}

impl ManagerDto {
    pub fn is_live(&self) -> bool {
        self.mode == "live"
    }
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }
    /// 초기 자본금 문자열 — 쉼표 포맷 (없으면 "—")
    pub fn initial_capital_str(&self) -> String {
        let v = self.initial_capital_val();
        if v == 0.0 && self.initial_capital.is_none() {
            return "—".to_string();
        }
        format_krw(v)
    }
    pub fn initial_capital_val(&self) -> f64 {
        self.initial_capital.as_ref().map(json_to_f64).unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPolicyDto {
    pub max_single_order_amount_krw: String,
    pub max_daily_loss_pct: String,
    pub max_daily_trade_count: i32,
    pub allow_market_order: bool,
    pub min_ai_confidence_pct: String,
    pub min_evidence_count: i32,
    #[serde(default)]
    pub require_fresh_quote_seconds: i32,
    #[serde(default)]
    pub require_fresh_account_seconds: i32,
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
    pub fn market_value_val(&self) -> f64 {
        self.market_value.as_ref().map(json_to_f64).unwrap_or(0.0)
    }
    pub fn unrealized_pnl_val(&self) -> f64 {
        self.unrealized_pnl.as_ref().map(json_to_f64).unwrap_or(0.0)
    }
}

/// serde_json::Value(숫자 또는 숫자 문자열)를 f64로 파싱한다.
pub fn json_to_f64(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// f64를 KRW 쉼표 포맷으로 변환 (예: 10000000 → "10,000,000")
pub fn format_krw(v: f64) -> String {
    let neg = v < 0.0;
    let i = v.abs() as u64;
    let s = i.to_string();
    let mut result = String::new();
    for (idx, ch) in s.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let formatted: String = result.chars().rev().collect();
    if neg { format!("-{}", formatted) } else { formatted }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeCycleDto {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub symbol_code: String,
    pub status: String,
    pub open_quantity: serde_json::Value,
    pub total_buy_quantity: serde_json::Value,
    pub total_sell_quantity: serde_json::Value,
    pub avg_entry_price: serde_json::Value,
    pub avg_exit_price: serde_json::Value,
    pub realized_pnl: serde_json::Value,
    pub total_fee: serde_json::Value,
    pub total_tax: serde_json::Value,
    pub fill_count: i32,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

impl TradeCycleDto {
    pub fn realized_pnl_val(&self) -> f64 {
        json_to_f64(&self.realized_pnl)
    }
    pub fn avg_entry_str(&self) -> String {
        self.avg_entry_price.to_string()
    }
    pub fn avg_exit_str(&self) -> String {
        self.avg_exit_price.to_string()
    }
    pub fn is_open(&self) -> bool {
        self.status == "open"
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
