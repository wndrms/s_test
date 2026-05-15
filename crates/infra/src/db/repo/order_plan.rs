use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::order_plan::{
    CreateOrderPlanInput, OrderPlan, OrderPlanRepository, RiskStatus,
};

pub struct PgOrderPlanRepository {
    pool: PgPool,
}

impl PgOrderPlanRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct OrderPlanRow {
    id: Uuid,
    manager_id: Uuid,
    scenario_run_id: Option<Uuid>,
    scenario_item_id: Option<Uuid>,
    symbol_id: Uuid,
    side: String,
    order_type: String,
    quantity: Decimal,
    limit_price: Decimal,
    estimated_amount: Decimal,
    ai_reason: Option<String>,
    risk_status: String,
    risk_reject_reason: Option<String>,
    idempotency_key: String,
    created_at: DateTime<Utc>,
}

impl From<OrderPlanRow> for OrderPlan {
    fn from(r: OrderPlanRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            scenario_run_id: r.scenario_run_id,
            scenario_item_id: r.scenario_item_id,
            symbol_id: r.symbol_id,
            side: r.side,
            order_type: r.order_type,
            quantity: r.quantity,
            limit_price: r.limit_price,
            estimated_amount: r.estimated_amount,
            ai_reason: r.ai_reason,
            risk_status: match r.risk_status.as_str() {
                "approved" => RiskStatus::Approved,
                "rejected" => RiskStatus::Rejected,
                _ => RiskStatus::Pending,
            },
            risk_reject_reason: r.risk_reject_reason,
            idempotency_key: r.idempotency_key,
            created_at: r.created_at,
        }
    }
}

#[async_trait]
impl OrderPlanRepository for PgOrderPlanRepository {
    async fn create_if_not_exists(&self, input: CreateOrderPlanInput) -> Result<Option<OrderPlan>> {
        let estimated = input.quantity * input.limit_price;
        let row: Option<OrderPlanRow> = sqlx::query_as::<_, OrderPlanRow>(
            r#"INSERT INTO order_plans
               (manager_id, scenario_run_id, scenario_item_id, symbol_id,
                side, order_type, quantity, limit_price, estimated_amount,
                ai_reason, risk_status, risk_reject_reason, idempotency_key)
               VALUES ($1,$2,$3,$4,$5,'limit',$6,$7,$8,$9,$10,$11,$12)
               ON CONFLICT (idempotency_key) DO NOTHING
               RETURNING id, manager_id, scenario_run_id, scenario_item_id, symbol_id,
                         side, order_type, quantity, limit_price, estimated_amount,
                         ai_reason, risk_status, risk_reject_reason, idempotency_key, created_at"#,
        )
        .bind(input.manager_id)
        .bind(input.scenario_run_id)
        .bind(input.scenario_item_id)
        .bind(input.symbol_id)
        .bind(&input.side)
        .bind(input.quantity)
        .bind(input.limit_price)
        .bind(estimated)
        .bind(input.ai_reason)
        .bind(input.risk_status.to_string())
        .bind(input.risk_reject_reason)
        .bind(&input.idempotency_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_by_manager(&self, manager_id: Uuid, limit: i64) -> Result<Vec<OrderPlan>> {
        let rows: Vec<OrderPlanRow> = sqlx::query_as::<_, OrderPlanRow>(
            r#"SELECT id, manager_id, scenario_run_id, scenario_item_id, symbol_id,
                      side, order_type, quantity, limit_price, estimated_amount,
                      ai_reason, risk_status, risk_reject_reason, idempotency_key, created_at
               FROM order_plans WHERE manager_id = $1
               ORDER BY created_at DESC LIMIT $2"#,
        )
        .bind(manager_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<OrderPlan>> {
        let row: Option<OrderPlanRow> = sqlx::query_as::<_, OrderPlanRow>(
            r#"SELECT id, manager_id, scenario_run_id, scenario_item_id, symbol_id,
                      side, order_type, quantity, limit_price, estimated_amount,
                      ai_reason, risk_status, risk_reject_reason, idempotency_key, created_at
               FROM order_plans WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }
}
