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
use lumos_infra::providers::naver_news::NaverNewsClient;
use lumos_infra::scenario::mock_llm::MockLlmProvider;
use lumos_infra::scenario::openai_llm::OpenAiLlmProvider;

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

    let pool = pg_pool::connect(&database_url).await?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    let _encryptor =
        Arc::new(AesGcmEncryptor::from_base64(&encryption_key).context("invalid ENCRYPTION_KEY")?);

    // KIS 클라이언트 (환경변수가 있으면 생성, 없으면 None)
    let kis_client = build_kis_client_from_env().await?;

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

    // 네이버 뉴스 클라이언트 (NAVER_CLIENT_ID + NAVER_CLIENT_SECRET 둘 다 있어야 활성화)
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("NAVER_CLIENT_ID"),
        std::env::var("NAVER_CLIENT_SECRET"),
    ) {
        tracing::info!("Naver News API 활성화");
        sched_builder = sched_builder.with_news_client(NaverNewsClient::new(client_id, client_secret));
    } else {
        tracing::warn!("NAVER_CLIENT_ID / NAVER_CLIENT_SECRET 미설정 — 뉴스 수집 비활성화");
    }

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
async fn build_kis_client_from_env() -> anyhow::Result<Option<KisClient>> {
    let app_key = std::env::var("KIS_APP_KEY").unwrap_or_default();
    let app_secret = std::env::var("KIS_APP_SECRET").unwrap_or_default();
    let account_no = std::env::var("KIS_ACCOUNT_NO").unwrap_or_default();
    let account_product = std::env::var("KIS_ACCOUNT_PRODUCT").unwrap_or_else(|_| "01".to_string());

    if app_key.is_empty() || app_secret.is_empty() || account_no.is_empty() {
        tracing::warn!(
            "KIS_APP_KEY / KIS_APP_SECRET / KIS_ACCOUNT_NO not set — KIS features disabled"
        );
        return Ok(None);
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

    let client = KisClient::new(env, app_key, app_secret, account_no, account_product);

    #[cfg(feature = "online-kis")]
    {
        client.issue_access_token().await?;
        tracing::info!("KIS access token initialized");
    }

    Ok(Some(client))
}
