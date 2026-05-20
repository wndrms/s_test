use std::sync::Arc;
use uuid::Uuid;

use lumos_domain::model::user::SecretKey;

use crate::error::{AppError, AppResult};
use crate::repo::user::SecretKeyRepository;

pub struct SecretService {
    repo: Arc<dyn SecretKeyRepository>,
    encryptor: Arc<dyn SecretEncryptor>,
}

pub trait SecretEncryptor: Send + Sync {
    fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>>;
    fn mask(&self, raw: &str) -> String;
}

impl SecretService {
    pub fn new(repo: Arc<dyn SecretKeyRepository>, encryptor: Arc<dyn SecretEncryptor>) -> Self {
        Self { repo, encryptor }
    }

    pub async fn store(
        &self,
        user_id: Uuid,
        provider: String,
        label: String,
        raw_key: &str,
    ) -> AppResult<SecretKey> {
        let encrypted = self
            .encryptor
            .encrypt(raw_key.as_bytes())
            .map_err(AppError::Internal)?;
        let masked_hint = Some(self.encryptor.mask(raw_key));

        self.repo
            .create(user_id, provider, label, encrypted, masked_hint)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn decrypt_raw(&self, id: Uuid) -> AppResult<Vec<u8>> {
        let raw = self
            .repo
            .find_raw_by_id(id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("secret key {id}")))?;

        self.encryptor
            .decrypt(&raw.encrypted_payload)
            .map_err(AppError::Internal)
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> AppResult<Vec<SecretKey>> {
        self.repo
            .find_by_user(user_id)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn delete(&self, id: Uuid) -> AppResult<()> {
        self.repo.delete(id).await.map_err(AppError::Internal)
    }
}
