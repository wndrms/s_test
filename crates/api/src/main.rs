use std::sync::Arc;

use anyhow::Context;
use dotenvy::dotenv;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod auth;
mod error;
mod routes;
mod state;

use lumos_app::repo::broker_connection::BrokerConnectionRepository;
use lumos_app::service::manager::ManagerService;
use lumos_app::service::notification::NotificationService;
use lumos_app::service::scenario::ScenarioService;
use lumos_app::service::secret::SecretService;
use lumos_infra::crypto::AesGcmEncryptor;
use lumos_infra::db::pg_pool;
use lumos_infra::db::repo::analysis_report::PgAnalysisReportRepository;
use lumos_infra::db::repo::broker_connection::PgBrokerConnectionRepository;
use lumos_infra::db::repo::broker_order::PgBrokerOrderRepository;
use lumos_infra::db::repo::holdings::PgHoldingsRepository;
use lumos_infra::db::repo::manager::{PgManagerRepository, PgRiskPolicyRepository};
use lumos_infra::db::repo::manager_universe::PgManagerUniverseRepository;
use lumos_infra::db::repo::order_plan::PgOrderPlanRepository;
use lumos_infra::db::repo::scenario::{
    PgEvidenceCardRepository, PgScenarioItemRepository, PgScenarioRunRepository,
};
use lumos_infra::db::repo::schedule::PgManagerScheduleRepository;
use lumos_infra::db::repo::schedule_mgmt::PgManagerScheduleWriteRepository;
use lumos_infra::db::repo::trade_cycle::PgTradeCycleRepository;
use lumos_infra::db::repo::symbol::PgSymbolRepository;
use lumos_infra::db::repo::user::PgSecretKeyRepository;
use lumos_app::service::llm_key::{LlmKeyService, LlmProviderFactory};
use lumos_domain::port::llm::LlmProvider as LlmProviderTrait;
use lumos_infra::providers::mock_notification::MockNotificationProvider;
use lumos_infra::scenario::gemini_llm::GeminiLlmProvider;
use lumos_infra::scenario::mock_llm::MockLlmProvider;
use lumos_infra::scenario::openai_llm::OpenAiLlmProvider;

struct DefaultProviderFactory;

impl LlmProviderFactory for DefaultProviderFactory {
    fn build_openai(
        &self,
        api_key: String,
        model: String,
        base_url: Option<String>,
    ) -> Arc<dyn LlmProviderTrait> {
        match base_url {
            Some(url) => Arc::new(OpenAiLlmProvider::with_base_url(api_key, model, url)),
            None => Arc::new(OpenAiLlmProvider::new(api_key, model)),
        }
    }

    fn build_gemini(
        &self,
        api_key: String,
        model: String,
        base_url: Option<String>,
    ) -> Arc<dyn LlmProviderTrait> {
        match base_url {
            Some(url) => Arc::new(GeminiLlmProvider::with_base_url(api_key, model, url)),
            None => Arc::new(GeminiLlmProvider::new(api_key, model)),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    init_tracing();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let encryption_key = std::env::var("ENCRYPTION_KEY").unwrap_or_else(|_| {
        tracing::warn!("ENCRYPTION_KEY not set — generating random key (data will not persist across restarts)");
        use rand::RngCore;
        use base64::Engine;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        base64::engine::general_purpose::STANDARD.encode(key)
    });
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    let pool = pg_pool::connect(&database_url).await?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    let encryptor: Arc<dyn lumos_app::service::secret::SecretEncryptor> = Arc::new(
        AesGcmEncryptor::from_base64(&encryption_key)
            .context("invalid ENCRYPTION_KEY — must be base64-encoded 32 bytes")?,
    );

    let manager_repo = Arc::new(PgManagerRepository::new(pool.clone()));
    let policy_repo: Arc<dyn lumos_app::repo::manager::RiskPolicyRepository> =
        Arc::new(PgRiskPolicyRepository::new(pool.clone()));
    let secret_repo: Arc<dyn lumos_app::repo::user::SecretKeyRepository> =
        Arc::new(PgSecretKeyRepository::new(pool.clone()));
    let symbol_repo: Arc<dyn lumos_app::repo::symbol::SymbolRepository> =
        Arc::new(PgSymbolRepository::new(pool.clone()));
    let evidence_repo = Arc::new(PgEvidenceCardRepository::new(pool.clone()));
    let scenario_run_repo = Arc::new(PgScenarioRunRepository::new(pool.clone()));
    let scenario_item_repo: Arc<dyn lumos_app::repo::scenario::ScenarioItemRepository> =
        Arc::new(PgScenarioItemRepository::new(pool.clone()));
    let llm: Arc<dyn lumos_domain::port::llm::LlmProvider> =
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let model = std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string());
            tracing::info!(model, "LLM: OpenAI 연결");
            Arc::new(OpenAiLlmProvider::new(api_key, model))
        } else {
            tracing::warn!("OPENAI_API_KEY 미설정 — MockLlmProvider 사용");
            Arc::new(MockLlmProvider::new())
        };

    let holdings_repo = Arc::new(PgHoldingsRepository::new(pool.clone()));
    let trade_cycle_repo: Arc<dyn lumos_app::repo::trade_cycle::TradeCycleRepository> =
        Arc::new(PgTradeCycleRepository::new(pool.clone()));
    let schedule_read_repo = Arc::new(PgManagerScheduleRepository::new(pool.clone()));
    let schedule_write_repo = Arc::new(PgManagerScheduleWriteRepository::new(pool.clone()));
    let analysis_report_repo: Arc<dyn lumos_app::repo::analysis_report::AnalysisReportRepository> =
        Arc::new(PgAnalysisReportRepository::new(pool.clone()));
    let order_plan_repo: Arc<dyn lumos_app::repo::order_plan::OrderPlanRepository> =
        Arc::new(PgOrderPlanRepository::new(pool.clone()));
    let broker_connection_repo = Arc::new(PgBrokerConnectionRepository::new(pool.clone()));
    let broker_order_repo: Arc<dyn lumos_app::repo::broker_order::BrokerOrderRepository> =
        Arc::new(PgBrokerOrderRepository::new(pool.clone()));
    let manager_universe_repo: Arc<dyn lumos_app::repo::manager_universe::ManagerUniverseRepository> =
        Arc::new(PgManagerUniverseRepository::new(pool.clone()));

    let manager_service = Arc::new(
        ManagerService::new(manager_repo, Arc::clone(&policy_repo))
            .with_broker_connection_repo(
                Arc::clone(&broker_connection_repo) as Arc<dyn BrokerConnectionRepository>,
            ),
    );
    let secret_service = Arc::new(SecretService::new(
        Arc::clone(&secret_repo),
        Arc::clone(&encryptor),
    ));
    let llm_key_service = Arc::new(LlmKeyService::new(
        Arc::clone(&secret_repo),
        Arc::clone(&encryptor),
        Arc::clone(&llm),
        Arc::new(DefaultProviderFactory),
    ));
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
        scenario_service,
        symbol_repo,
        holdings_repo,
        trade_cycle_repo,
        schedule_read_repo,
        schedule_write_repo,
        analysis_report_repo,
        order_plan_repo,
        scenario_item_repo,
        risk_policy_repo: policy_repo,
        notification_service,
        broker_connection_repo: broker_connection_repo as Arc<dyn BrokerConnectionRepository>,
        broker_order_repo,
        llm_key_service,
        manager_universe_repo,
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
