use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use lumos_domain::model::user::{SecretKey, SecretKeyRaw, User};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>>;
    async fn create(&self, email: Option<String>, display_name: Option<String>, password_hash: Option<String>) -> Result<User>;
}

#[async_trait]
pub trait SecretKeyRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<SecretKey>>;
    async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<SecretKey>>;
    async fn find_by_provider(&self, user_id: Uuid, provider: &str) -> Result<Vec<SecretKey>>;
    async fn create(
        &self,
        user_id: Uuid,
        provider: String,
        label: String,
        encrypted_payload: Vec<u8>,
        masked_hint: Option<String>,
    ) -> Result<SecretKey>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn find_raw_by_id(&self, id: Uuid) -> Result<Option<SecretKeyRaw>>;
}
