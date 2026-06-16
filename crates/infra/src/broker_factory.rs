use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tokio::sync::Mutex;
use uuid::Uuid;

use lumos_app::error::{AppError, AppResult};
use lumos_app::repo::broker_connection::BrokerConnectionRepository;
use lumos_app::service::broker_factory::BrokerFactory;
use lumos_app::service::secret::SecretService;
use lumos_domain::model::broker::BrokerEnvironment;
use lumos_domain::model::symbol::Currency;
use lumos_domain::port::broker::Broker;

use crate::kis::client::{KisClient, KisEnvironment};
use crate::kis::paper_broker::PaperBroker;

/// 모의 broker의 기본 초기 자본 (KRW). broker_connection에는 자본 정보가 없으므로
/// 환경변수 PAPER_INITIAL_CASH로 조정 가능하다.
const DEFAULT_PAPER_CASH: Decimal = dec!(100_000_000);
const DEFAULT_ACCOUNT_PRODUCT: &str = "01";

/// broker_connection으로부터 실행 가능한 Broker를 생성한다.
/// - Real  → KisClient (stateless, 매번 생성)
/// - Paper → PaperBroker (상태 유지를 위해 connection별로 캐시)
pub struct DefaultBrokerFactory {
    broker_conn_repo: Arc<dyn BrokerConnectionRepository>,
    secret_service: Arc<SecretService>,
    paper_cache: Mutex<HashMap<Uuid, Arc<dyn Broker>>>,
}

impl DefaultBrokerFactory {
    pub fn new(
        broker_conn_repo: Arc<dyn BrokerConnectionRepository>,
        secret_service: Arc<SecretService>,
    ) -> Self {
        Self {
            broker_conn_repo,
            secret_service,
            paper_cache: Mutex::new(HashMap::new()),
        }
    }

    fn paper_initial_cash() -> Decimal {
        std::env::var("PAPER_INITIAL_CASH")
            .ok()
            .and_then(|v| v.parse::<Decimal>().ok())
            .unwrap_or(DEFAULT_PAPER_CASH)
    }
}

#[async_trait]
impl BrokerFactory for DefaultBrokerFactory {
    async fn create(&self, broker_connection_id: Uuid) -> AppResult<Arc<dyn Broker>> {
        let creds = self
            .broker_conn_repo
            .find_credentials(broker_connection_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| {
                AppError::NotFound(format!("broker_connection {broker_connection_id}"))
            })?;

        match creds.environment {
            BrokerEnvironment::Paper => {
                // 모의 broker는 포지션/현금 상태를 유지해야 하므로 connection별로 캐시한다.
                let mut cache = self.paper_cache.lock().await;
                if let Some(existing) = cache.get(&broker_connection_id) {
                    return Ok(Arc::clone(existing));
                }
                // 체결은 limit_price 기준으로 처리되므로 quote_source는 평가용 폴백(0)으로 둔다.
                // TODO: 보유 종목 mark-to-market을 위해 실시간 시세 소스 연결.
                let broker: Arc<dyn Broker> = Arc::new(PaperBroker::with_static_quotes(
                    broker_connection_id,
                    Self::paper_initial_cash(),
                    Currency::Krw,
                    HashMap::new(),
                ));
                cache.insert(broker_connection_id, Arc::clone(&broker));
                Ok(broker)
            }
            BrokerEnvironment::Real => {
                let app_key = self.decrypt_secret(creds.app_key_secret_id).await?;
                let app_secret = self.decrypt_secret(creds.app_secret_secret_id).await?;
                let account_no = String::from_utf8(
                    self.secret_service
                        .decrypt_payload(&creds.account_no_encrypted)?,
                )
                .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid account_no: {e}")))?;

                let client = KisClient::new(
                    KisEnvironment::Real,
                    app_key,
                    app_secret,
                    account_no,
                    DEFAULT_ACCOUNT_PRODUCT.to_string(),
                );
                Ok(Arc::new(client))
            }
        }
    }
}

impl DefaultBrokerFactory {
    async fn decrypt_secret(&self, secret_id: Uuid) -> AppResult<String> {
        let raw = self.secret_service.decrypt_raw(secret_id).await?;
        String::from_utf8(raw)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid secret utf-8: {e}")))
    }
}
