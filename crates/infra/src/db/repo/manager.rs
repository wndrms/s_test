use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::manager::{CreateManagerInput, ManagerRepository, RiskPolicyRepository};
use lumos_domain::model::manager::{Manager, ManagerMode, ManagerStatus};
use lumos_domain::model::risk::RiskPolicy;
use lumos_domain::model::symbol::{Currency, Region};

pub struct PgManagerRepository {
    pool: PgPool,
}

impl PgManagerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ManagerRow {
    id: Uuid,
    user_id: Uuid,
    broker_connection_id: Uuid,
    name: String,
    mode: String,
    region: String,
    base_currency: String,
    initial_capital: Decimal,
    auto_trade_enabled: bool,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ManagerRow> for Manager {
    fn from(r: ManagerRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            broker_connection_id: r.broker_connection_id,
            name: r.name,
            mode: match r.mode.as_str() {
                "live" => ManagerMode::Live,
                _ => ManagerMode::Paper,
            },
            region: match r.region.as_str() {
                "US" => Region::Us,
                _ => Region::Kr,
            },
            base_currency: match r.base_currency.as_str() {
                "USD" => Currency::Usd,
                _ => Currency::Krw,
            },
            initial_capital: r.initial_capital,
            auto_trade_enabled: r.auto_trade_enabled,
            status: match r.status.as_str() {
                "paused" => ManagerStatus::Paused,
                "deleted" => ManagerStatus::Deleted,
                _ => ManagerStatus::Active,
            },
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[async_trait]
impl ManagerRepository for PgManagerRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Manager>> {
        let row: Option<ManagerRow> = sqlx::query_as::<_, ManagerRow>(
            r#"SELECT id, user_id, broker_connection_id, name, mode, region,
                      base_currency, initial_capital, auto_trade_enabled, status,
                      created_at, updated_at
               FROM managers WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<Manager>> {
        let rows: Vec<ManagerRow> = sqlx::query_as::<_, ManagerRow>(
            r#"SELECT id, user_id, broker_connection_id, name, mode, region,
                      base_currency, initial_capital, auto_trade_enabled, status,
                      created_at, updated_at
               FROM managers WHERE user_id = $1 AND status != 'deleted' ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_active(&self) -> Result<Vec<Manager>> {
        let rows: Vec<ManagerRow> = sqlx::query_as::<_, ManagerRow>(
            r#"SELECT id, user_id, broker_connection_id, name, mode, region,
                      base_currency, initial_capital, auto_trade_enabled, status,
                      created_at, updated_at
               FROM managers WHERE status = 'active' ORDER BY created_at"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create(&self, input: CreateManagerInput) -> Result<Manager> {
        let row: ManagerRow = sqlx::query_as::<_, ManagerRow>(
            r#"INSERT INTO managers
               (user_id, broker_connection_id, name, mode, region, base_currency, initial_capital)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, user_id, broker_connection_id, name, mode, region,
                         base_currency, initial_capital, auto_trade_enabled, status,
                         created_at, updated_at"#,
        )
        .bind(input.user_id)
        .bind(input.broker_connection_id)
        .bind(input.name)
        .bind(input.mode.to_string().to_lowercase())
        .bind(input.region.to_string())
        .bind(input.base_currency.to_string())
        .bind(input.initial_capital)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn update_status(&self, id: Uuid, status: ManagerStatus) -> Result<Manager> {
        let status_str: String = match status {
            ManagerStatus::Active => "active".to_string(),
            ManagerStatus::Paused => "paused".to_string(),
            ManagerStatus::Deleted => "deleted".to_string(),
        };
        let row: ManagerRow = sqlx::query_as::<_, ManagerRow>(
            r#"UPDATE managers SET status = $2, updated_at = now()
               WHERE id = $1
               RETURNING id, user_id, broker_connection_id, name, mode, region,
                         base_currency, initial_capital, auto_trade_enabled, status,
                         created_at, updated_at"#,
        )
        .bind(id)
        .bind(status_str)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn set_auto_trade(&self, id: Uuid, enabled: bool) -> Result<Manager> {
        let row: ManagerRow = sqlx::query_as::<_, ManagerRow>(
            r#"UPDATE managers SET auto_trade_enabled = $2, updated_at = now()
               WHERE id = $1
               RETURNING id, user_id, broker_connection_id, name, mode, region,
                         base_currency, initial_capital, auto_trade_enabled, status,
                         created_at, updated_at"#,
        )
        .bind(id)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}

pub struct PgRiskPolicyRepository {
    pool: PgPool,
}

impl PgRiskPolicyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct RiskPolicyRow {
    manager_id: Uuid,
    max_position_pct: Decimal,
    max_single_order_amount_krw: Decimal,
    max_daily_loss_pct: Decimal,
    max_daily_trade_count: i32,
    allow_market_order: bool,
    allow_pre_market: bool,
    allow_after_hours: bool,
    require_fresh_quote_seconds: i32,
    require_fresh_account_seconds: i32,
    min_ai_confidence_pct: Decimal,
    min_evidence_count: i32,
    updated_at: DateTime<Utc>,
}

impl From<RiskPolicyRow> for RiskPolicy {
    fn from(r: RiskPolicyRow) -> Self {
        Self {
            manager_id: r.manager_id,
            max_position_pct: r.max_position_pct,
            max_single_order_amount_krw: r.max_single_order_amount_krw,
            max_daily_loss_pct: r.max_daily_loss_pct,
            max_daily_trade_count: r.max_daily_trade_count,
            allow_market_order: r.allow_market_order,
            allow_pre_market: r.allow_pre_market,
            allow_after_hours: r.allow_after_hours,
            require_fresh_quote_seconds: r.require_fresh_quote_seconds,
            require_fresh_account_seconds: r.require_fresh_account_seconds,
            min_ai_confidence_pct: r.min_ai_confidence_pct,
            min_evidence_count: r.min_evidence_count,
            updated_at: r.updated_at,
        }
    }
}

#[async_trait]
impl RiskPolicyRepository for PgRiskPolicyRepository {
    async fn find_by_manager(&self, manager_id: Uuid) -> Result<Option<RiskPolicy>> {
        let row: Option<RiskPolicyRow> = sqlx::query_as::<_, RiskPolicyRow>(
            r#"SELECT manager_id, max_position_pct, max_single_order_amount_krw,
                      max_daily_loss_pct, max_daily_trade_count, allow_market_order,
                      allow_pre_market, allow_after_hours, require_fresh_quote_seconds,
                      require_fresh_account_seconds, min_ai_confidence_pct, min_evidence_count,
                      updated_at
               FROM risk_policies WHERE manager_id = $1"#,
        )
        .bind(manager_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn upsert(&self, p: RiskPolicy) -> Result<RiskPolicy> {
        let row: RiskPolicyRow = sqlx::query_as::<_, RiskPolicyRow>(
            r#"INSERT INTO risk_policies
               (manager_id, max_position_pct, max_single_order_amount_krw,
                max_daily_loss_pct, max_daily_trade_count, allow_market_order,
                allow_pre_market, allow_after_hours, require_fresh_quote_seconds,
                require_fresh_account_seconds, min_ai_confidence_pct, min_evidence_count)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
               ON CONFLICT (manager_id) DO UPDATE SET
                 max_position_pct = EXCLUDED.max_position_pct,
                 max_single_order_amount_krw = EXCLUDED.max_single_order_amount_krw,
                 max_daily_loss_pct = EXCLUDED.max_daily_loss_pct,
                 max_daily_trade_count = EXCLUDED.max_daily_trade_count,
                 allow_market_order = EXCLUDED.allow_market_order,
                 allow_pre_market = EXCLUDED.allow_pre_market,
                 allow_after_hours = EXCLUDED.allow_after_hours,
                 require_fresh_quote_seconds = EXCLUDED.require_fresh_quote_seconds,
                 require_fresh_account_seconds = EXCLUDED.require_fresh_account_seconds,
                 min_ai_confidence_pct = EXCLUDED.min_ai_confidence_pct,
                 min_evidence_count = EXCLUDED.min_evidence_count,
                 updated_at = now()
               RETURNING manager_id, max_position_pct, max_single_order_amount_krw,
                         max_daily_loss_pct, max_daily_trade_count, allow_market_order,
                         allow_pre_market, allow_after_hours, require_fresh_quote_seconds,
                         require_fresh_account_seconds, min_ai_confidence_pct, min_evidence_count,
                         updated_at"#,
        )
        .bind(p.manager_id)
        .bind(p.max_position_pct)
        .bind(p.max_single_order_amount_krw)
        .bind(p.max_daily_loss_pct)
        .bind(p.max_daily_trade_count)
        .bind(p.allow_market_order)
        .bind(p.allow_pre_market)
        .bind(p.allow_after_hours)
        .bind(p.require_fresh_quote_seconds)
        .bind(p.require_fresh_account_seconds)
        .bind(p.min_ai_confidence_pct)
        .bind(p.min_evidence_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}
