use chrono::{DateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketType {
    Krx,
    Us,
}

impl std::fmt::Display for MarketType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketType::Krx => write!(f, "KRX"),
            MarketType::Us => write!(f, "US"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSession {
    pub market: MarketType,
    pub timezone: String,
    pub open: NaiveTime,
    pub close: NaiveTime,
}

impl MarketSession {
    pub fn krx() -> Self {
        Self {
            market: MarketType::Krx,
            timezone: "Asia/Seoul".to_string(),
            open: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            close: NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
        }
    }

    pub fn us() -> Self {
        Self {
            market: MarketType::Us,
            timezone: "America/New_York".to_string(),
            open: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
            close: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteSnapshot {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub source: String,
    pub last_price: Decimal,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub volume: Option<Decimal>,
    pub as_of: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceBar {
    pub symbol_id: Uuid,
    pub timeframe: Timeframe,
    pub ts: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timeframe {
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "1d")]
    D1,
    #[serde(rename = "1w")]
    W1,
    #[serde(rename = "1mo")]
    Mo1,
}
