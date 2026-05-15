use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::symbol::SymbolRepository;
use lumos_domain::model::symbol::{Currency, IdentifierType, Region, Symbol, SymbolIdentifier};

pub struct PgSymbolRepository {
    pool: PgPool,
}

impl PgSymbolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct SymbolRow {
    id: Uuid,
    region: String,
    market: String,
    code: String,
    display_code: String,
    name_ko: Option<String>,
    name_en: Option<String>,
    currency: String,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SymbolRow> for Symbol {
    fn from(r: SymbolRow) -> Self {
        Self {
            id: r.id,
            region: match r.region.as_str() {
                "US" => Region::Us,
                _ => Region::Kr,
            },
            market: r.market,
            code: r.code,
            display_code: r.display_code,
            name_ko: r.name_ko,
            name_en: r.name_en,
            currency: match r.currency.as_str() {
                "USD" => Currency::Usd,
                _ => Currency::Krw,
            },
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
struct SymbolIdentifierRow {
    id: Uuid,
    symbol_id: Uuid,
    id_type: String,
    id_value: String,
    source: String,
}

impl From<SymbolIdentifierRow> for SymbolIdentifier {
    fn from(r: SymbolIdentifierRow) -> Self {
        Self {
            id: r.id,
            symbol_id: r.symbol_id,
            id_type: match r.id_type.as_str() {
                "DART_CORP_CODE" => IdentifierType::DartCorpCode,
                "ISIN" => IdentifierType::Isin,
                "CIK" => IdentifierType::Cik,
                "FIGI" => IdentifierType::Figi,
                _ => IdentifierType::KisCode,
            },
            id_value: r.id_value,
            source: r.source,
        }
    }
}

#[async_trait]
impl SymbolRepository for PgSymbolRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Symbol>> {
        let row: Option<SymbolRow> = sqlx::query_as::<_, SymbolRow>(
            r#"SELECT id, region, market, code, display_code, name_ko, name_en,
                      currency, active, created_at, updated_at
               FROM symbols WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_code(&self, region: &Region, code: &str) -> Result<Option<Symbol>> {
        let region_str = region.to_string();
        let row: Option<SymbolRow> = sqlx::query_as::<_, SymbolRow>(
            r#"SELECT id, region, market, code, display_code, name_ko, name_en,
                      currency, active, created_at, updated_at
               FROM symbols WHERE region = $1 AND code = $2"#,
        )
        .bind(region_str)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Symbol>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<SymbolRow> = sqlx::query_as::<_, SymbolRow>(
            r#"SELECT id, region, market, code, display_code, name_ko, name_en,
                      currency, active, created_at, updated_at
               FROM symbols WHERE id = ANY($1)"#,
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_active(&self) -> Result<Vec<Symbol>> {
        let rows: Vec<SymbolRow> = sqlx::query_as::<_, SymbolRow>(
            r#"SELECT id, region, market, code, display_code, name_ko, name_en,
                      currency, active, created_at, updated_at
               FROM symbols WHERE active = true ORDER BY region, code"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_identifiers(&self, symbol_id: Uuid) -> Result<Vec<SymbolIdentifier>> {
        let rows: Vec<SymbolIdentifierRow> = sqlx::query_as::<_, SymbolIdentifierRow>(
            r#"SELECT id, symbol_id, id_type, id_value, source
               FROM symbol_identifiers WHERE symbol_id = $1"#,
        )
        .bind(symbol_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_row_region_mapping() {
        let row = SymbolRow {
            id: Uuid::nil(),
            region: "US".to_string(),
            market: "NAS".to_string(),
            code: "AAPL".to_string(),
            display_code: "AAPL".to_string(),
            name_ko: None,
            name_en: Some("Apple".to_string()),
            currency: "USD".to_string(),
            active: true,
            created_at: DateTime::default(),
            updated_at: DateTime::default(),
        };
        let symbol: Symbol = row.into();
        assert_eq!(symbol.region, Region::Us);
        assert_eq!(symbol.currency, Currency::Usd);
    }

    #[test]
    fn symbol_row_kr_defaults() {
        let row = SymbolRow {
            id: Uuid::nil(),
            region: "KR".to_string(),
            market: "KOSPI".to_string(),
            code: "005930".to_string(),
            display_code: "005930".to_string(),
            name_ko: Some("삼성전자".to_string()),
            name_en: None,
            currency: "KRW".to_string(),
            active: true,
            created_at: DateTime::default(),
            updated_at: DateTime::default(),
        };
        let symbol: Symbol = row.into();
        assert_eq!(symbol.region, Region::Kr);
        assert_eq!(symbol.currency, Currency::Krw);
    }
}
