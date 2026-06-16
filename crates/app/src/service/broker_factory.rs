use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::AppResult;
use lumos_domain::port::broker::Broker;

/// broker_connection_id로부터 실행 가능한 Broker 인스턴스를 생성한다.
/// 환경(Real/Paper)에 따라 실거래 KIS 클라이언트 또는 모의 broker를 반환한다.
/// 구현은 infra 레이어에 둔다 (KisClient/PaperBroker가 infra 타입이므로).
#[async_trait]
pub trait BrokerFactory: Send + Sync {
    async fn create(&self, broker_connection_id: Uuid) -> AppResult<Arc<dyn Broker>>;
}
