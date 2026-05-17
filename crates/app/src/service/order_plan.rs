use std::sync::Arc;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use lumos_domain::model::broker::{LimitOrderRequest, OrderSide};
use lumos_domain::model::scenario::ScenarioAction;
use lumos_domain::port::broker::Broker;

use crate::error::{AppError, AppResult};
use crate::repo::broker_order::{BrokerOrderRepository, CreateBrokerOrderInput};
use crate::repo::manager::RiskPolicyRepository;
use crate::repo::order_plan::{CreateOrderPlanInput, OrderPlan, OrderPlanRepository, RiskStatus};
use crate::repo::scenario::ScenarioItemRepository;
use crate::service::notification::NotificationService;
use crate::service::risk::{evaluate, OrderContext};

pub struct OrderPlanService {
    order_plan_repo: Arc<dyn OrderPlanRepository>,
    scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    risk_policy_repo: Arc<dyn RiskPolicyRepository>,
    broker_order_repo: Option<Arc<dyn BrokerOrderRepository>>,
    broker: Option<Arc<dyn Broker>>,
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
            broker: None,
            notification: None,
        }
    }

    pub fn with_notification(mut self, svc: Arc<NotificationService>) -> Self {
        self.notification = Some(svc);
        self
    }

    pub fn with_broker(
        mut self,
        broker: Arc<dyn Broker>,
        broker_order_repo: Arc<dyn BrokerOrderRepository>,
    ) -> Self {
        self.broker = Some(broker);
        self.broker_order_repo = Some(broker_order_repo);
        self
    }

    /// Approved 상태인 OrderPlan을 실제 브로커에 주문 제출한다.
    /// `live-trading` feature가 없으면 항상 에러를 반환한다.
    pub async fn execute_approved(
        &self,
        plan: &OrderPlan,
        #[allow(unused_variables)] broker_connection_id: Uuid,
    ) -> AppResult<()> {
        #[cfg(not(feature = "live-trading"))]
        return Err(AppError::Validation(
            "live-trading feature is not enabled".to_string(),
        ));

        #[cfg(feature = "live-trading")]
        {
            let broker = self
                .broker
                .as_ref()
                .ok_or_else(|| AppError::Validation("broker not configured".to_string()))?;
            let broker_order_repo = self.broker_order_repo.as_ref().ok_or_else(|| {
                AppError::Validation("broker_order_repo not configured".to_string())
            })?;

            if plan.risk_status != RiskStatus::Approved {
                return Err(AppError::Validation(
                    "only approved plans can be executed".to_string(),
                ));
            }

            let symbol_code = plan
                .symbol_code
                .clone()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    AppError::Validation("symbol_code not resolved for order plan".to_string())
                })?;

            let side = if plan.side == "buy" {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };
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

            broker_order_repo
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

        let limit_price = item.target_price.unwrap_or(dec!(0));
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

        let idempotency_key = format!("{}:{}:{}", item.scenario_run_id, scenario_item_id, side_str);

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

    pub async fn list_for_manager(
        &self,
        manager_id: Uuid,
        limit: i64,
    ) -> AppResult<Vec<OrderPlan>> {
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

        let ctx =
            OrderContext::new_simple(manager_id, symbol_id, side.clone(), quantity, limit_price);
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
}
