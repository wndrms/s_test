use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::broker_order::{BrokerOrder, BrokerOrderRepository, CreateBrokerOrderInput};

pub struct PgBrokerOrderRepository {
    pool: PgPool,
}

impl PgBrokerOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct BrokerOrderRow {
    id: Uuid,
    order_plan_id: Uuid,
    broker_connection_id: Uuid,
    external_order_id: Option<String>,
    external_org_no: Option<String>,
    status: String,
    submitted_at: Option<DateTime<Utc>>,
}

impl From<BrokerOrderRow> for BrokerOrder {
    fn from(r: BrokerOrderRow) -> Self {
        Self {
            id: r.id,
            order_plan_id: r.order_plan_id,
            broker_connection_id: r.broker_connection_id,
            external_order_id: r.external_order_id,
            external_org_no: r.external_org_no,
            status: r.status,
            submitted_at: r.submitted_at,
        }
    }
}

#[async_trait]
impl BrokerOrderRepository for PgBrokerOrderRepository {
    async fn create(&self, input: CreateBrokerOrderInput) -> Result<BrokerOrder> {
        let row: BrokerOrderRow = sqlx::query_as::<_, BrokerOrderRow>(
            r#"INSERT INTO broker_orders
               (order_plan_id, broker_connection_id, external_order_id, external_org_no,
                status, submitted_at, raw_response_json)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, order_plan_id, broker_connection_id, external_order_id,
                         external_org_no, status, submitted_at"#,
        )
        .bind(input.order_plan_id)
        .bind(input.broker_connection_id)
        .bind(input.external_order_id)
        .bind(input.external_org_no)
        .bind(&input.status)
        .bind(input.submitted_at)
        .bind(input.raw_response_json)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn find_by_plan(&self, order_plan_id: Uuid) -> Result<Vec<BrokerOrder>> {
        let rows: Vec<BrokerOrderRow> = sqlx::query_as::<_, BrokerOrderRow>(
            r#"SELECT id, order_plan_id, broker_connection_id, external_order_id,
                      external_org_no, status, submitted_at
               FROM broker_orders WHERE order_plan_id = $1
               ORDER BY id DESC"#,
        )
        .bind(order_plan_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
