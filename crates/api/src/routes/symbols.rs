use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use lumos_domain::model::symbol::{Region, Symbol};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search_symbols))
        .route("/", get(list_symbols))
}

#[derive(Debug, Deserialize)]
pub struct SearchSymbolsQuery {
    /// 검색어 (종목 코드 또는 이름)
    q: String,
    /// 지역 필터 (KR 또는 US)
    #[serde(default)]
    region: Option<String>,
    /// 결과 개수 제한 (기본 20, 최대 100)
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
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

impl From<Symbol> for SymbolDto {
    fn from(s: Symbol) -> Self {
        Self {
            id: s.id,
            region: s.region.to_string(),
            market: s.market,
            code: s.code,
            display_code: s.display_code,
            name_ko: s.name_ko,
            name_en: s.name_en,
            currency: s.currency.to_string(),
        }
    }
}

async fn search_symbols(
    State(state): State<AppState>,
    Query(query): Query<SearchSymbolsQuery>,
) -> ApiResult<Json<Vec<SymbolDto>>> {
    let region = query.region.as_ref().and_then(|r| match r.as_str() {
        "KR" => Some(Region::Kr),
        "US" => Some(Region::Us),
        _ => None,
    });

    let limit = query.limit.min(100).max(1);

    let symbols = state
        .symbol_repo
        .search(&query.q, region.as_ref(), limit)
        .await
        .map_err(|e| ApiError::from(lumos_app::error::AppError::Internal(e)))?;

    let dtos = symbols.into_iter().map(SymbolDto::from).collect();
    Ok(Json(dtos))
}

async fn list_symbols(State(state): State<AppState>) -> ApiResult<Json<Vec<SymbolDto>>> {
    let symbols = state
        .symbol_repo
        .find_active()
        .await
        .map_err(|e| ApiError::from(lumos_app::error::AppError::Internal(e)))?;

    let dtos = symbols.into_iter().map(SymbolDto::from).collect();
    Ok(Json(dtos))
}
