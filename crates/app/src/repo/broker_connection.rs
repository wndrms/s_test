use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::broker::{BrokerConnection, BrokerEnvironment};

#[async_trait]
pub trait BrokerConnectionRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<BrokerConnection>>;
    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<BrokerConnection>>;
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
