use std::sync::Arc;

use anyhow::Context;
use dotenvy::dotenv;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod scheduler;

use lumos_app::service::order_plan::OrderPlanService;
use lumos_app::service::scenario::ScenarioService;
use lumos_infra::crypto::AesGcmEncryptor;
use lumos_infra::db::pg_pool;

use lumos_infra::db::repo::analysis_report::PgAnalysisReportRepository;
use lumos_infra::db::repo::manager::{PgManagerRepository, PgRiskPolicyRepository};
use lumos_infra::db::repo::order_plan::PgOrderPlanRepository;
use lumos_infra::db::repo::scenario::{
    PgEvidenceCardRepository, PgScenarioItemRepository, PgScenarioRunRepository,
};
use lumos_infra::db::repo::schedule::{PgManagerScheduleRepository, PgScheduleRunRepository};
use lumos_infra::db::repo::symbol::PgSymbolRepository;
use lumos_infra::kis::client::{KisClient, KisEnvironment};
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

    let _encryptor =
        Arc::new(AesGcmEncryptor::from_base64(&encryption_key).context("invalid ENCRYPTION_KEY")?);

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

    let mut scheduler = scheduler::Scheduler::new(
        schedule_repo,
        run_repo,
        scenario_svc,
        symbol_repo,
        manager_repo,
        scenario_item_repo,
    )
    .with_order_plan_svc(order_plan_svc);

    if let Some(kis_client) = build_kis_client_from_env().await? {
        scheduler = scheduler.with_kis_client(Arc::new(kis_client));
    }

    let sched = Arc::new(scheduler);

    tracing::info!("Lumos worker started");
    sched.run().await?;

    Ok(())
}

async fn build_kis_client_from_env() -> anyhow::Result<Option<KisClient>> {
    let Some(app_key) = optional_env("KIS_APP_KEY") else {
        tracing::warn!("KIS_APP_KEY not set; KIS evidence collection is disabled");
        return Ok(None);
    };

    let app_secret = required_env("KIS_APP_SECRET")?;
    let account_no = required_env("KIS_ACCOUNT_NO")?;
    let account_product = optional_env("KIS_ACCOUNT_PRODUCT").unwrap_or_else(|| "01".to_string());
    let env = match optional_env("KIS_ENV")
        .unwrap_or_else(|| "paper".to_string())
        .to_lowercase()
        .as_str()
    {
        "real" => KisEnvironment::Real,
        _ => KisEnvironment::Paper,
    };

    let client = KisClient::new(env, app_key, app_secret, account_no, account_product);

    #[cfg(feature = "online-kis")]
    {
        client.issue_access_token().await?;
        tracing::info!("KIS client initialized with an access token");
    }

    #[cfg(not(feature = "online-kis"))]
    tracing::info!("KIS client initialized in fixture mode");

    Ok(Some(client))
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_env(key: &str) -> anyhow::Result<String> {
    optional_env(key).with_context(|| format!("{key} not set"))
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
