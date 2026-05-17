use std::sync::Arc;

use lumos_app::repo::analysis_report::AnalysisReportRepository;
use lumos_app::repo::broker_connection::BrokerConnectionRepository;
use lumos_app::repo::broker_order::BrokerOrderRepository;
use lumos_app::repo::holdings::HoldingsRepository;
use lumos_app::repo::manager::RiskPolicyRepository;
use lumos_app::repo::order_plan::OrderPlanRepository;
use lumos_app::repo::scenario::ScenarioItemRepository;
use lumos_app::repo::schedule::{ManagerScheduleRepository, ManagerScheduleWriteRepository};
use lumos_app::repo::symbol::SymbolRepository;
use lumos_app::repo::trades::TradesRepository;
use lumos_app::service::manager::ManagerService;
use lumos_app::service::notification::NotificationService;
use lumos_app::service::scenario::ScenarioService;
use lumos_app::service::secret::SecretService;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub manager_service: Arc<ManagerService>,
    pub secret_service: Arc<SecretService>,
    pub scenario_service: Arc<ScenarioService>,
    pub symbol_repo: Arc<dyn SymbolRepository>,
    pub holdings_repo: Arc<dyn HoldingsRepository>,
    pub trades_repo: Arc<dyn TradesRepository>,
    pub schedule_read_repo: Arc<dyn ManagerScheduleRepository>,
    pub schedule_write_repo: Arc<dyn ManagerScheduleWriteRepository>,
    pub analysis_report_repo: Arc<dyn AnalysisReportRepository>,
    pub order_plan_repo: Arc<dyn OrderPlanRepository>,
    pub scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    pub risk_policy_repo: Arc<dyn RiskPolicyRepository>,
    pub notification_service: Arc<NotificationService>,
    pub broker_connection_repo: Arc<dyn BrokerConnectionRepository>,
    pub broker_order_repo: Arc<dyn BrokerOrderRepository>,
}
