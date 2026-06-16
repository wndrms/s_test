use std::sync::Arc;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use lumos_domain::model::broker::{LimitOrderRequest, OrderFillQuery, OrderSide};
use lumos_domain::model::scenario::ScenarioAction;
use lumos_domain::model::trade_cycle::CycleFill;
use crate::error::{AppError, AppResult};
use crate::repo::broker_order::{BrokerOrderRepository, CreateBrokerOrderInput};
use crate::repo::manager::RiskPolicyRepository;
use crate::repo::order_plan::{CreateOrderPlanInput, OrderPlan, OrderPlanRepository, RiskStatus};
use crate::repo::scenario::ScenarioItemRepository;
use crate::repo::trade_cycle::TradeCycleRepository;
use crate::service::broker_factory::BrokerFactory;
use crate::service::notification::NotificationService;
use crate::service::risk::{evaluate, OrderContext};

pub struct OrderPlanService {
    order_plan_repo: Arc<dyn OrderPlanRepository>,
    scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    risk_policy_repo: Arc<dyn RiskPolicyRepository>,
    broker_order_repo: Option<Arc<dyn BrokerOrderRepository>>,
    trade_cycle_repo: Option<Arc<dyn TradeCycleRepository>>,
    broker_factory: Option<Arc<dyn BrokerFactory>>,
    notification: Option<Arc<NotificationService>>,
}

impl OrderPlanService {
    pub fn new(
        order_plan_repo: Arc<dyn OrderPlanRepository>,
        scenario_item_repo: Arc<dyn ScenarioItemRepository>,
        risk_policy_repo: Arc<dyn RiskPolicyRepository>,
    ) -> Self {
        Self {
            order_plan_repo,
            scenario_item_repo,
            risk_policy_repo,
            broker_order_repo: None,
            trade_cycle_repo: None,
            broker_factory: None,
            notification: None,
        }
    }

    pub fn with_notification(mut self, svc: Arc<NotificationService>) -> Self {
        self.notification = Some(svc);
        self
    }

    pub fn with_broker_factory(
        mut self,
        broker_factory: Arc<dyn BrokerFactory>,
        broker_order_repo: Arc<dyn BrokerOrderRepository>,
    ) -> Self {
        self.broker_factory = Some(broker_factory);
        self.broker_order_repo = Some(broker_order_repo);
        self
    }

    pub fn with_trade_cycle_repo(mut self, repo: Arc<dyn TradeCycleRepository>) -> Self {
        self.trade_cycle_repo = Some(repo);
        self
    }

    /// Approved 상태인 OrderPlan을 실제 브로커에 주문 제출한다.
    /// `live-trading` feature가 없으면 항상 에러를 반환한다.
    pub async fn execute_approved(
        &self,
        plan: &OrderPlan,
        #[allow(unused_variables)]
        broker_connection_id: Uuid,
    ) -> AppResult<()> {
        #[cfg(not(feature = "live-trading"))]
        return Err(AppError::Validation(
            "live-trading feature is not enabled".to_string(),
        ));

        #[cfg(feature = "live-trading")]
        {
            let broker_factory = self
                .broker_factory
                .as_ref()
                .ok_or_else(|| AppError::Validation("broker_factory not configured".to_string()))?;
            let broker_order_repo = self
                .broker_order_repo
                .as_ref()
                .ok_or_else(|| AppError::Validation("broker_order_repo not configured".to_string()))?;
            // 매니저의 broker_connection으로 실행 broker(Real/Paper)를 동적 생성한다.
            let broker = broker_factory.create(broker_connection_id).await?;

            if plan.risk_status != RiskStatus::Approved {
                return Err(AppError::Validation(
                    "only approved plans can be executed".to_string(),
                ));
            }

            let symbol_code = plan.symbol_code.clone().filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Validation("symbol_code not resolved for order plan".to_string()))?;

            let side = if plan.side == "buy" { OrderSide::Buy } else { OrderSide::Sell };
            let req = LimitOrderRequest {
                symbol_code,
                side,
                quantity: plan.quantity,
                limit_price: plan.limit_price,
                idempotency_key: plan.idempotency_key.clone(),
                market: None,
            };

            let broker_resp = broker
                .place_limit_order(req)
                .await
                .map_err(|e| AppError::BrokerError(e.to_string()))?;

            let broker_order = broker_order_repo
                .create(CreateBrokerOrderInput {
                    order_plan_id: plan.id,
                    broker_connection_id,
                    external_order_id: broker_resp.external_order_id.clone(),
                    external_org_no: broker_resp.external_org_no.clone(),
                    status: format!("{:?}", broker_resp.status).to_lowercase(),
                    submitted_at: Some(broker_resp.submitted_at),
                    raw_response_json: None,
                })
                .await
                .map_err(AppError::Internal)?;

            // 체결 내역 조회 후 매매 사이클에 기록한다.
            // record_fill이 한 트랜잭션 안에서 사이클 갱신 + trade_fill insert를 모두 처리한다.
            // 외부 주문 ID로 이 주문에 해당하는 체결만 필터링한다.
            if let Some(cycle_repo) = &self.trade_cycle_repo {
                let fills = broker
                    .get_order_fills(OrderFillQuery {
                        trading_date: broker_resp.submitted_at.date_naive(),
                        symbol_code: plan.symbol_code.clone(),
                    })
                    .await
                    .map_err(|e| AppError::BrokerError(e.to_string()))?;

                for fill in fills {
                    if let Some(ext_id) = &broker_resp.external_order_id {
                        if &fill.external_order_id != ext_id {
                            continue;
                        }
                    }

                    cycle_repo
                        .record_fill(
                            plan.manager_id,
                            plan.symbol_id,
                            broker_order.id,
                            CycleFill {
                                side: fill.side.clone(),
                                quantity: fill.quantity,
                                price: fill.price,
                                fee: fill.fee,
                                tax: fill.tax,
                                filled_at: fill.filled_at,
                            },
                        )
                        .await
                        .map_err(AppError::Internal)?;
                }
            }

            Ok(())
        }
    }

    pub async fn create_from_scenario_item(
        &self,
        manager_id: Uuid,
        scenario_item_id: Uuid,
    ) -> AppResult<OrderPlan> {
        let item = self
            .scenario_item_repo
            .find_by_run_and_id(scenario_item_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound("scenario_item".to_string()))?;

        let side = match &item.action {
            ScenarioAction::Buy => OrderSide::Buy,
            ScenarioAction::Sell => OrderSide::Sell,
            ScenarioAction::Hold | ScenarioAction::Watch => {
                return Err(AppError::Validation(
                    "hold/watch actions do not generate order plans".to_string(),
                ))
            }
        };

        let limit_price = item
            .target_price
            .unwrap_or(dec!(0));
        if limit_price.is_zero() {
            return Err(AppError::Validation(
                "scenario item has no target price".to_string(),
            ));
        }

        let policy = self
            .risk_policy_repo
            .find_by_manager(manager_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| lumos_domain::model::risk::RiskPolicy::default_for(manager_id));

        // MVP: quantity = max_single_order / limit_price, 소수점 내림
        let quantity = (policy.max_single_order_amount_krw / limit_price)
            .floor()
            .max(dec!(1));

        let ctx = OrderContext::new_simple(
            manager_id,
            item.symbol_id,
            side.clone(),
            quantity,
            limit_price,
        );
        let risk_result = evaluate(&policy, &ctx);
        let risk_passed = risk_result.passed;

        let (risk_status, risk_reject_reason) = if risk_passed {
            (RiskStatus::Approved, None)
        } else {
            (RiskStatus::Rejected, risk_result.reject_reason.clone())
        };

        let side_str = match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
        .to_string();

        let idempotency_key = format!(
            "{}:{}:{}",
            item.scenario_run_id, scenario_item_id, side_str
        );

        let plan = self
            .order_plan_repo
            .create_if_not_exists(CreateOrderPlanInput {
                manager_id,
                scenario_run_id: Some(item.scenario_run_id),
                scenario_item_id: Some(scenario_item_id),
                symbol_id: item.symbol_id,
                side: side_str,
                quantity,
                limit_price,
                ai_reason: Some(item.condition_text.clone()),
                risk_status,
                risk_reject_reason,
                idempotency_key,
            })
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::Validation("duplicate order plan".to_string()))?;

        if !risk_passed {
            if let Some(svc) = &self.notification {
                svc.notify_risk_rejected(manager_id, &plan).await;
            }
        }

        Ok(plan)
    }

    pub async fn list_for_manager(&self, manager_id: Uuid, limit: i64) -> AppResult<Vec<OrderPlan>> {
        self.order_plan_repo
            .find_by_manager(manager_id, limit)
            .await
            .map_err(AppError::Internal)
    }

    /// Paper 모드에서 직접 주문 계획을 생성한다 (시나리오 없이).
    pub async fn create_paper_order(
        &self,
        manager_id: Uuid,
        symbol_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        limit_price: Decimal,
    ) -> AppResult<OrderPlan> {
        let policy = self
            .risk_policy_repo
            .find_by_manager(manager_id)
            .await
            .map_err(AppError::Internal)?
            .unwrap_or_else(|| lumos_domain::model::risk::RiskPolicy::default_for(manager_id));

        let ctx = OrderContext::new_simple(manager_id, symbol_id, side.clone(), quantity, limit_price);
        let risk_result = evaluate(&policy, &ctx);

        let (risk_status, risk_reject_reason) = if risk_result.passed {
            (RiskStatus::Approved, None)
        } else {
            (RiskStatus::Rejected, risk_result.reject_reason.clone())
        };

        let side_str = match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
        .to_string();

        let idempotency_key = uuid::Uuid::new_v4().to_string();

        let plan = self
            .order_plan_repo
            .create_if_not_exists(CreateOrderPlanInput {
                manager_id,
                scenario_run_id: None,
                scenario_item_id: None,
                symbol_id,
                side: side_str,
                quantity,
                limit_price,
                ai_reason: None,
                risk_status,
                risk_reject_reason,
                idempotency_key,
            })
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::Validation("duplicate paper order".to_string()))?;

        Ok(plan)
    }
}

/// 최대 주문 금액을 quantity로 변환하는 헬퍼 (테스트용 노출)
pub fn calc_quantity(max_amount_krw: Decimal, limit_price: Decimal) -> Decimal {
    if limit_price.is_zero() {
        return Decimal::ZERO;
    }
    (max_amount_krw / limit_price).floor().max(dec!(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn quantity_rounds_down() {
        assert_eq!(calc_quantity(dec!(1_000_000), dec!(35_000)), dec!(28));
    }

    #[test]
    fn quantity_min_one() {
        assert_eq!(calc_quantity(dec!(1_000), dec!(2_000_000)), dec!(1));
    }

    #[test]
    fn zero_price_returns_zero() {
        assert_eq!(calc_quantity(dec!(1_000_000), dec!(0)), dec!(0));
    }

    // ── execute_approved 가 체결을 trade_fills로 기록하는지 검증 ────────────────
    #[cfg(feature = "live-trading")]
    mod execute_records_fills {
        use super::*;
        use anyhow::Result;
        use async_trait::async_trait;
        use chrono::{NaiveDate, Utc};
        use std::sync::Mutex;
        use lumos_domain::model::broker::{
            BrokerAccount, BrokerFill, BrokerOrderResponse, BrokerOrderStatus, BrokerPosition,
            BuyingPower, BuyingPowerRequest, CancelOrderRequest, OrderFillQuery,
        };
        use lumos_domain::model::risk::RiskPolicy;
        use lumos_domain::port::broker::Broker;
        use crate::service::broker_factory::BrokerFactory;
        use crate::repo::broker_order::BrokerOrder;
        use crate::repo::manager::RiskPolicyRepository;
        use crate::repo::order_plan::OrderPlan;
        use crate::repo::scenario::{
            CreateScenarioItemInput, ScenarioItemRepository,
        };
        use crate::repo::trade_cycle::{FillQuery, TradeCycleRepository, TradeFillRow};
        use lumos_domain::model::scenario::ScenarioItem;
        use lumos_domain::model::trade_cycle::{apply_fill, TradeCycle, TradeCycleStatus};

        struct StubOrderPlanRepo;
        #[async_trait]
        impl OrderPlanRepository for StubOrderPlanRepo {
            async fn create_if_not_exists(
                &self,
                _i: crate::repo::order_plan::CreateOrderPlanInput,
            ) -> Result<Option<OrderPlan>> {
                Ok(None)
            }
            async fn find_by_manager(&self, _m: Uuid, _l: i64) -> Result<Vec<OrderPlan>> {
                Ok(vec![])
            }
            async fn find_by_id(&self, _id: Uuid) -> Result<Option<OrderPlan>> {
                Ok(None)
            }
        }

        struct StubItemRepo;
        #[async_trait]
        impl ScenarioItemRepository for StubItemRepo {
            async fn create_batch(
                &self,
                _i: Vec<CreateScenarioItemInput>,
            ) -> Result<Vec<ScenarioItem>> {
                Ok(vec![])
            }
            async fn find_by_run(&self, _r: Uuid) -> Result<Vec<ScenarioItem>> {
                Ok(vec![])
            }
            async fn find_by_run_and_id(&self, _id: Uuid) -> Result<Option<ScenarioItem>> {
                Ok(None)
            }
            async fn find_pending_for_manager(&self, _m: Uuid) -> Result<Vec<ScenarioItem>> {
                Ok(vec![])
            }
        }

        struct StubRiskRepo;
        #[async_trait]
        impl RiskPolicyRepository for StubRiskRepo {
            async fn find_by_manager(&self, m: Uuid) -> Result<Option<RiskPolicy>> {
                Ok(Some(RiskPolicy::default_for(m)))
            }
            async fn upsert(&self, p: RiskPolicy) -> Result<RiskPolicy> {
                Ok(p)
            }
        }

        struct StubBrokerOrderRepo {
            id: Uuid,
        }
        #[async_trait]
        impl BrokerOrderRepository for StubBrokerOrderRepo {
            async fn create(&self, input: CreateBrokerOrderInput) -> Result<BrokerOrder> {
                Ok(BrokerOrder {
                    id: self.id,
                    order_plan_id: input.order_plan_id,
                    broker_connection_id: input.broker_connection_id,
                    external_order_id: input.external_order_id,
                    external_org_no: input.external_org_no,
                    status: input.status,
                    submitted_at: input.submitted_at,
                })
            }
            async fn find_by_plan(&self, _p: Uuid) -> Result<Vec<BrokerOrder>> {
                Ok(vec![])
            }
        }

        /// fill 기록(사이클 갱신 포함)과 사이클 조회를 한 곳에서 처리하는 in-memory 구현.
        struct InMemoryCycleRepo {
            cycles: Mutex<Vec<TradeCycle>>,
            fills: Mutex<Vec<TradeFillRow>>,
        }
        impl InMemoryCycleRepo {
            fn new() -> Self {
                Self { cycles: Mutex::new(vec![]), fills: Mutex::new(vec![]) }
            }
        }
        #[async_trait]
        impl TradeCycleRepository for InMemoryCycleRepo {
            async fn record_fill(
                &self,
                manager_id: Uuid,
                symbol_id: Uuid,
                broker_order_id: Uuid,
                fill: lumos_domain::model::trade_cycle::CycleFill,
            ) -> Result<(TradeCycle, TradeFillRow)> {
                // get_or_open
                let cycle = {
                    let mut guard = self.cycles.lock().unwrap();
                    match guard.iter().find(|c| {
                        c.manager_id == manager_id
                            && c.symbol_id == symbol_id
                            && c.status == TradeCycleStatus::Open
                    }) {
                        Some(c) => c.clone(),
                        None => {
                            let c = TradeCycle::new(Uuid::new_v4(), manager_id, symbol_id, Utc::now());
                            guard.push(c.clone());
                            c
                        }
                    }
                };
                let updated = apply_fill(&cycle, &fill);
                {
                    let mut guard = self.cycles.lock().unwrap();
                    if let Some(slot) = guard.iter_mut().find(|c| c.id == updated.id) {
                        *slot = updated.clone();
                    }
                }
                let side = match fill.side {
                    OrderSide::Buy => "buy",
                    OrderSide::Sell => "sell",
                }
                .to_string();
                let row = TradeFillRow {
                    id: Uuid::new_v4(),
                    broker_order_id,
                    trade_cycle_id: Some(updated.id),
                    symbol_id,
                    side,
                    quantity: fill.quantity,
                    price: fill.price,
                    fee: fill.fee,
                    tax: fill.tax,
                    filled_at: fill.filled_at,
                    manager_id: Some(manager_id),
                };
                self.fills.lock().unwrap().push(row.clone());
                Ok((updated, row))
            }
            async fn find_open(&self, m: Uuid, s: Uuid) -> Result<Option<TradeCycle>> {
                Ok(self
                    .cycles
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|c| c.manager_id == m && c.symbol_id == s && c.status == TradeCycleStatus::Open)
                    .cloned())
            }
            async fn find_by_manager(&self, m: Uuid, _l: i64) -> Result<Vec<TradeCycle>> {
                Ok(self
                    .cycles
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|c| c.manager_id == m)
                    .cloned()
                    .collect())
            }
            async fn find_by_id(&self, id: Uuid) -> Result<Option<TradeCycle>> {
                Ok(self.cycles.lock().unwrap().iter().find(|c| c.id == id).cloned())
            }
            async fn list_fills(&self, m: Uuid, _q: FillQuery) -> Result<Vec<TradeFillRow>> {
                Ok(self
                    .fills
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|f| f.manager_id == Some(m))
                    .cloned()
                    .collect())
            }
        }

        struct FakeBroker {
            ext_id: String,
        }
        #[async_trait]
        impl Broker for FakeBroker {
            async fn get_account(&self) -> Result<BrokerAccount> {
                unimplemented!()
            }
            async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
                unimplemented!()
            }
            async fn get_buying_power(&self, _r: BuyingPowerRequest) -> Result<BuyingPower> {
                unimplemented!()
            }
            async fn place_limit_order(
                &self,
                _r: LimitOrderRequest,
            ) -> Result<BrokerOrderResponse> {
                Ok(BrokerOrderResponse {
                    external_order_id: Some(self.ext_id.clone()),
                    external_org_no: None,
                    status: BrokerOrderStatus::Filled,
                    submitted_at: Utc::now(),
                })
            }
            async fn cancel_order(&self, _r: CancelOrderRequest) -> Result<BrokerOrderResponse> {
                unimplemented!()
            }
            async fn get_order_fills(&self, _q: OrderFillQuery) -> Result<Vec<BrokerFill>> {
                // 우리 주문 체결 1건 + 무관한 주문 체결 1건 (필터링 검증용)
                Ok(vec![
                    BrokerFill {
                        external_order_id: self.ext_id.clone(),
                        symbol_code: "005930".into(),
                        side: OrderSide::Buy,
                        quantity: dec!(10),
                        price: dec!(70_000),
                        fee: dec!(105),
                        tax: dec!(0),
                        filled_at: Utc::now(),
                    },
                    BrokerFill {
                        external_order_id: "OTHER-ORDER".into(),
                        symbol_code: "005930".into(),
                        side: OrderSide::Buy,
                        quantity: dec!(5),
                        price: dec!(70_000),
                        fee: dec!(50),
                        tax: dec!(0),
                        filled_at: Utc::now(),
                    },
                ])
            }
        }

        struct FakeBrokerFactory {
            ext_id: String,
        }
        #[async_trait]
        impl BrokerFactory for FakeBrokerFactory {
            async fn create(
                &self,
                _broker_connection_id: Uuid,
            ) -> AppResult<Arc<dyn Broker>> {
                Ok(Arc::new(FakeBroker {
                    ext_id: self.ext_id.clone(),
                }))
            }
        }

        fn approved_plan() -> OrderPlan {
            OrderPlan {
                id: Uuid::new_v4(),
                manager_id: Uuid::new_v4(),
                scenario_run_id: None,
                scenario_item_id: None,
                symbol_id: Uuid::new_v4(),
                symbol_code: Some("005930".into()),
                side: "buy".into(),
                order_type: "limit".into(),
                quantity: dec!(10),
                limit_price: dec!(70_000),
                estimated_amount: dec!(700_000),
                ai_reason: None,
                risk_status: RiskStatus::Approved,
                risk_reject_reason: None,
                idempotency_key: "k".into(),
                created_at: Utc::now(),
            }
        }

        /// 우리 주문 체결만 기록되고(필터링), 사이클을 open하며 수량/평균가를 갱신하고,
        /// fill이 사이클에 연결된다.
        #[tokio::test]
        async fn records_only_matching_fill_and_updates_cycle() {
            let broker_order_id = Uuid::new_v4();
            let cycles = Arc::new(InMemoryCycleRepo::new());
            let plan = approved_plan();
            let manager_id = plan.manager_id;

            let svc = OrderPlanService::new(
                Arc::new(StubOrderPlanRepo),
                Arc::new(StubItemRepo),
                Arc::new(StubRiskRepo),
            )
            .with_broker_factory(
                Arc::new(FakeBrokerFactory { ext_id: "MY-ORDER".into() }),
                Arc::new(StubBrokerOrderRepo { id: broker_order_id }),
            )
            .with_trade_cycle_repo(cycles.clone());

            svc.execute_approved(&plan, Uuid::new_v4())
                .await
                .expect("execute should succeed");

            // 무관한 주문(OTHER-ORDER) 체결은 제외되고 우리 체결 1건만 기록
            let fills = cycles.list_fills(manager_id, FillQuery::default()).await.unwrap();
            assert_eq!(fills.len(), 1, "오직 우리 주문 체결만 기록되어야 함");
            assert_eq!(fills[0].broker_order_id, broker_order_id);
            assert_eq!(fills[0].quantity, dec!(10));
            assert_eq!(fills[0].fee, dec!(105));
            assert_eq!(fills[0].side, "buy");

            // 사이클이 열리고 갱신됨
            let all = cycles.find_by_manager(manager_id, 10).await.unwrap();
            assert_eq!(all.len(), 1, "사이클이 하나 열려야 함");
            let cycle = &all[0];
            assert_eq!(cycle.status, TradeCycleStatus::Open);
            assert_eq!(cycle.open_quantity, dec!(10));
            assert_eq!(cycle.avg_entry_price, dec!(70_000));
            assert_eq!(cycle.fill_count, 1);

            // fill이 해당 사이클에 연결됨
            assert_eq!(fills[0].trade_cycle_id, Some(cycle.id));
        }
    }
}
