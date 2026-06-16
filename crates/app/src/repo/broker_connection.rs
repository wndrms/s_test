use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::broker::{BrokerConnection, BrokerEnvironment};

/// broker 인스턴스 생성에 필요한 자격증명 참조. 복호화 전 상태이며
/// API 응답 모델(BrokerConnection)과 분리해 외부 노출을 막는다.
#[derive(Debug, Clone)]
pub struct BrokerCredentials {
    pub broker_connection_id: Uuid,
    pub environment: BrokerEnvironment,
    pub account_no_encrypted: Vec<u8>,
    pub app_key_secret_id: Uuid,
    pub app_secret_secret_id: Uuid,
}

#[async_trait]
pub trait BrokerConnectionRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<BrokerConnection>>;
    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<BrokerConnection>>;
    /// broker 생성용 자격증명 조회 (시크릿 ID + 암호화 계좌번호).
    async fn find_credentials(&self, id: Uuid) -> Result<Option<BrokerCredentials>>;
    async fn create(
        &self,
        user_id: Uuid,
        environment: BrokerEnvironment,
        account_no_masked: String,
        account_no_encrypted: Vec<u8>,
        app_key_secret_id: Uuid,
        app_secret_secret_id: Uuid,
    ) -> Result<BrokerConnection>;
    async fn set_verified(&self, id: Uuid) -> Result<BrokerConnection>;
}
