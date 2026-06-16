use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use lumos_domain::model::trade_cycle::{CycleFill, TradeCycle};

/// 체결(trade_fill) 한 건의 영속 표현. 매니저/사이클 join 정보 포함.
#[derive(Debug, Clone)]
pub struct TradeFillRow {
    pub id: Uuid,
    pub broker_order_id: Uuid,
    pub trade_cycle_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
    pub manager_id: Option<Uuid>,
}

/// 체결 조회 필터.
#[derive(Debug, Clone, Default)]
pub struct FillQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub side: Option<String>,
    pub limit: i64,
}

/// 매매 사이클 추적과 체결 기록을 한 곳에서 책임지는 저장소.
///
/// 체결 발생 시 `record_fill`이 단일 트랜잭션 안에서
/// (1) 해당 매니저+종목의 open 사이클을 가져오거나 새로 열고,
/// (2) 도메인 규칙(`apply_fill`)으로 사이클 상태를 갱신하고,
/// (3) trade_fill을 사이클에 연결해 insert 한다.
#[async_trait]
pub trait TradeCycleRepository: Send + Sync {
    /// 체결 한 건을 기록하고 사이클을 갱신한다. 갱신된 사이클과 기록된 체결을 반환한다.
    async fn record_fill(
        &self,
        manager_id: Uuid,
        symbol_id: Uuid,
        broker_order_id: Uuid,
        fill: CycleFill,
    ) -> Result<(TradeCycle, TradeFillRow)>;

    /// 매니저+종목의 현재 open 사이클 (없으면 None).
    async fn find_open(&self, manager_id: Uuid, symbol_id: Uuid) -> Result<Option<TradeCycle>>;

    /// 매니저의 사이클 목록 (최신순).
    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<TradeCycle>>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<TradeCycle>>;

    /// 매니저의 체결 목록 조회.
    async fn list_fills(&self, manager_id: Uuid, query: FillQuery) -> Result<Vec<TradeFillRow>>;
}
