use std::sync::Arc;

use anyhow::Context;
use dotenvy::dotenv;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod auth;
mod error;
mod routes;
mod state;

use lumos_app::service::manager::ManagerService;
use lumos_app::service::notification::NotificationService;
use lumos_app::service::scenario::ScenarioService;
use lumos_app::service::secret::SecretService;
use lumos_infra::crypto::AesGcmEncryptor;
use lumos_infra::db::pg_pool;
use lumos_infra::db::repo::analysis_report::PgAnalysisReportRepository;
use lumos_infra::db::repo::broker_connection::PgBrokerConnectionRepository;
use lumos_infra::db::repo::holdings::PgHoldingsRepository;
use lumos_infra::db::repo::manager::{PgManagerRepository, PgRiskPolicyRepository};
use lumos_infra::db::repo::order_plan::PgOrderPlanRepository;
use lumos_infra::db::repo::scenario::{
    PgEvidenceCardRepository, PgScenarioItemRepository, PgScenarioRunRepository,
};
use lumos_infra::db::repo::schedule::PgManagerScheduleRepository;
use lumos_infra::db::repo::schedule_mgmt::PgManagerScheduleWriteRepository;
use lumos_infra::db::repo::symbol::PgSymbolRepository;
use lumos_infra::db::repo::trades::PgTradesRepository;
use lumos_infra::db::repo::user::PgSecretKeyRepository;
use lumos_infra::providers::mock_notification::MockNotificationProvider;
use lumos_infra::scenario::mock_llm::MockLlmProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_tracing();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let encryption_key = std::env::var("ENCRYPTION_KEY").context("ENCRYPTION_KEY not set")?;
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    let pool = pg_pool::connect(&database_url).await?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    let encryptor = Arc::new(
        AesGcmEncryptor::from_base64(&encryption_key)
            .context("invalid ENCRYPTION_KEY — must be base64-encoded 32 bytes")?,
    );

    let manager_repo = Arc::new(PgManagerRepository::new(pool.clone()));
    let policy_repo: Arc<dyn lumos_app::repo::manager::RiskPolicyRepository> =
        Arc::new(PgRiskPolicyRepository::new(pool.clone()));
    let secret_repo = Arc::new(PgSecretKeyRepository::new(pool.clone()));
    let broker_connection_repo: Arc<
        dyn lumos_app::repo::broker_connection::BrokerConnectionRepository,
    > = Arc::new(PgBrokerConnectionRepository::new(pool.clone()));
    let symbol_repo: Arc<dyn lumos_app::repo::symbol::SymbolRepository> =
        Arc::new(PgSymbolRepository::new(pool.clone()));
    let evidence_repo = Arc::new(PgEvidenceCardRepository::new(pool.clone()));
    let scenario_run_repo = Arc::new(PgScenarioRunRepository::new(pool.clone()));
    let scenario_item_repo: Arc<dyn lumos_app::repo::scenario::ScenarioItemRepository> =
        Arc::new(PgScenarioItemRepository::new(pool.clone()));
    let llm = Arc::new(MockLlmProvider::new());

    let holdings_repo = Arc::new(PgHoldingsRepository::new(pool.clone()));
    let trades_repo = Arc::new(PgTradesRepository::new(pool.clone()));
    let schedule_read_repo = Arc::new(PgManagerScheduleRepository::new(pool.clone()));
    let schedule_write_repo = Arc::new(PgManagerScheduleWriteRepository::new(pool.clone()));
    let analysis_report_repo: Arc<dyn lumos_app::repo::analysis_report::AnalysisReportRepository> =
        Arc::new(PgAnalysisReportRepository::new(pool.clone()));
    let order_plan_repo: Arc<dyn lumos_app::repo::order_plan::OrderPlanRepository> =
        Arc::new(PgOrderPlanRepository::new(pool.clone()));

    let manager_service = Arc::new(ManagerService::new(manager_repo, Arc::clone(&policy_repo)));
    let secret_service = Arc::new(SecretService::new(secret_repo, encryptor));
    let scenario_service = Arc::new(
        ScenarioService::new(
            llm,
            evidence_repo,
            scenario_run_repo,
            Arc::clone(&scenario_item_repo),
            Arc::clone(&symbol_repo),
        )
        .with_report_repo(Arc::clone(&analysis_report_repo)),
    );

    let notification_service = Arc::new(NotificationService::new(Arc::new(
        MockNotificationProvider::new(),
    )));

    let app_state = state::AppState {
        db: pool,
        manager_service,
        secret_service,
        broker_connection_repo,
        scenario_service,
        symbol_repo,
        holdings_repo,
        trades_repo,
        schedule_read_repo,
        schedule_write_repo,
        analysis_report_repo,
        order_plan_repo,
        scenario_item_repo,
        risk_policy_repo: policy_repo,
        notification_service,
    };

    let app = routes::router(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .with_context(|| format!("failed to bind {listen_addr}"))?;

    tracing::info!("Lumos server listening on {listen_addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
