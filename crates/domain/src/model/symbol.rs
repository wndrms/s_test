use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Region {
    Kr,
    Us,
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Region::Kr => write!(f, "KR"),
            Region::Us => write!(f, "US"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Currency {
    Krw,
    Usd,
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Krw => write!(f, "KRW"),
            Currency::Usd => write!(f, "USD"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentifierType {
    KisCode,
    DartCorpCode,
    Isin,
    Cik,
    Figi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: Uuid,
    pub region: Region,
    pub market: String,
    pub code: String,
    pub display_code: String,
    pub name_ko: Option<String>,
    pub name_en: Option<String>,
    pub currency: Currency,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIdentifier {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub id_type: IdentifierType,
    pub id_value: String,
    pub source: String,
}
