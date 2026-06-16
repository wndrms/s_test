use std::sync::Arc;

use lumos_app::repo::analysis_report::AnalysisReportRepository;
use lumos_app::repo::broker_connection::BrokerConnectionRepository;
use lumos_app::repo::broker_order::BrokerOrderRepository;
use lumos_app::repo::holdings::HoldingsRepository;
use lumos_app::repo::manager::RiskPolicyRepository;
use lumos_app::repo::manager_universe::ManagerUniverseRepository;
use lumos_app::repo::order_plan::OrderPlanRepository;
use lumos_app::repo::scenario::ScenarioItemRepository;
use lumos_app::repo::schedule::{ManagerScheduleRepository, ManagerScheduleWriteRepository};
use lumos_app::repo::symbol::SymbolRepository;
use lumos_app::repo::trade_cycle::TradeCycleRepository;
use lumos_app::service::llm_key::LlmKeyService;
use lumos_app::service::manager::ManagerService;
use lumos_app::service::notification::NotificationService;
use lumos_app::service::scenario::ScenarioService;
use lumos_app::service::secret::SecretService;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    /// 풀 핸들. 일부 라우트가 직접 쿼리할 때 사용 (현재는 repo 경유).
    #[allow(dead_code)]
    pub db: PgPool,
    pub manager_service: Arc<ManagerService>,
    pub secret_service: Arc<SecretService>,
    pub scenario_service: Arc<ScenarioService>,
    pub symbol_repo: Arc<dyn SymbolRepository>,
    pub holdings_repo: Arc<dyn HoldingsRepository>,
    pub trade_cycle_repo: Arc<dyn TradeCycleRepository>,
    pub schedule_read_repo: Arc<dyn ManagerScheduleRepository>,
    pub schedule_write_repo: Arc<dyn ManagerScheduleWriteRepository>,
    pub analysis_report_repo: Arc<dyn AnalysisReportRepository>,
    pub order_plan_repo: Arc<dyn OrderPlanRepository>,
    pub scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    pub risk_policy_repo: Arc<dyn RiskPolicyRepository>,
    pub notification_service: Arc<NotificationService>,
    pub broker_connection_repo: Arc<dyn BrokerConnectionRepository>,
    pub broker_order_repo: Arc<dyn BrokerOrderRepository>,
    pub llm_key_service: Arc<LlmKeyService>,
    pub manager_universe_repo: Arc<dyn ManagerUniverseRepository>,
}
