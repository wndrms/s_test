use std::sync::Arc;

use anyhow::Context;
use dotenvy::dotenv;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod scheduler;

use lumos_app::service::order_plan::OrderPlanService;
use lumos_app::service::scenario::ScenarioService;
use lumos_infra::crypto::AesGcmEncryptor;
use lumos_infra::db::pg_pool;
use lumos_infra::kis::{KisClient, KisEnvironment};

use lumos_infra::db::repo::analysis_report::PgAnalysisReportRepository;
use lumos_infra::db::repo::manager::{PgManagerRepository, PgRiskPolicyRepository};
use lumos_infra::db::repo::order_plan::PgOrderPlanRepository;
use lumos_infra::db::repo::scenario::{
    PgEvidenceCardRepository, PgScenarioItemRepository, PgScenarioRunRepository,
};
use lumos_infra::db::repo::schedule::{PgManagerScheduleRepository, PgScheduleRunRepository};
use lumos_infra::db::repo::symbol::PgSymbolRepository;
use lumos_infra::scenario::mock_llm::MockLlmProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_tracing();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let encryption_key = std::env::var("ENCRYPTION_KEY").context("ENCRYPTION_KEY not set")?;

    let pool = pg_pool::connect(&database_url).await?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    let _encryptor = Arc::new(
        AesGcmEncryptor::from_base64(&encryption_key).context("invalid ENCRYPTION_KEY")?,
    );

    // KIS 클라이언트 (환경변수가 있으면 생성, 없으면 None)
    let kis_client = build_kis_client_from_env();

    let symbol_repo: Arc<dyn lumos_app::repo::symbol::SymbolRepository> =
        Arc::new(PgSymbolRepository::new(pool.clone()));
    let evidence_repo = Arc::new(PgEvidenceCardRepository::new(pool.clone()));
    let scenario_run_repo = Arc::new(PgScenarioRunRepository::new(pool.clone()));
    let scenario_item_repo: Arc<dyn lumos_app::repo::scenario::ScenarioItemRepository> =
        Arc::new(PgScenarioItemRepository::new(pool.clone()));
    let llm = Arc::new(MockLlmProvider::new());

    let report_repo: Arc<dyn lumos_app::repo::analysis_report::AnalysisReportRepository> =
        Arc::new(PgAnalysisReportRepository::new(pool.clone()));

    let scenario_svc = Arc::new(
        ScenarioService::new(
            llm,
            evidence_repo,
            scenario_run_repo,
            Arc::clone(&scenario_item_repo),
            Arc::clone(&symbol_repo),
        )
        .with_report_repo(report_repo),
    );

    let manager_repo: Arc<dyn lumos_app::repo::manager::ManagerRepository> =
        Arc::new(PgManagerRepository::new(pool.clone()));
    let order_plan_repo: Arc<dyn lumos_app::repo::order_plan::OrderPlanRepository> =
        Arc::new(PgOrderPlanRepository::new(pool.clone()));
    let risk_policy_repo: Arc<dyn lumos_app::repo::manager::RiskPolicyRepository> =
        Arc::new(PgRiskPolicyRepository::new(pool.clone()));

    let order_plan_svc = Arc::new(OrderPlanService::new(
        Arc::clone(&order_plan_repo),
        Arc::clone(&scenario_item_repo),
        Arc::clone(&risk_policy_repo),
    ));

    let schedule_repo = Arc::new(PgManagerScheduleRepository::new(pool.clone()));
    let run_repo = Arc::new(PgScheduleRunRepository::new(pool.clone()));

    let mut sched_builder = scheduler::Scheduler::new(
        schedule_repo,
        run_repo,
        scenario_svc,
        symbol_repo,
        manager_repo,
        scenario_item_repo,
    )
    .with_order_plan_svc(order_plan_svc);

    if let Some(client) = kis_client {
        sched_builder = sched_builder.with_kis_client(Arc::new(client));
    }

    let sched = Arc::new(sched_builder);

    tracing::info!("Lumos worker started");
    sched.run().await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// KIS_APP_KEY / KIS_APP_SECRET / KIS_ACCOUNT_NO 가 모두 설정된 경우에만 KisClient를 반환합니다.
fn build_kis_client_from_env() -> Option<KisClient> {
    let app_key = std::env::var("KIS_APP_KEY").unwrap_or_default();
    let app_secret = std::env::var("KIS_APP_SECRET").unwrap_or_default();
    let account_no = std::env::var("KIS_ACCOUNT_NO").unwrap_or_default();
    let account_product = std::env::var("KIS_ACCOUNT_PRODUCT").unwrap_or_else(|_| "01".to_string());

    if app_key.is_empty() || app_secret.is_empty() || account_no.is_empty() {
        tracing::warn!("KIS_APP_KEY / KIS_APP_SECRET / KIS_ACCOUNT_NO not set — KIS features disabled");
        return None;
    }

    let env = match std::env::var("KIS_ENV").as_deref() {
        Ok("real") => {
            tracing::info!("KIS worker mode: REAL");
            KisEnvironment::Real
        }
        _ => {
            tracing::info!("KIS worker mode: PAPER (모의투자)");
            KisEnvironment::Paper
        }
    };

    Some(KisClient::new(env, app_key, app_secret, account_no, account_product))
}
