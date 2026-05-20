use std::sync::Arc;
use uuid::Uuid;

use lumos_domain::model::user::SecretKey;
use lumos_domain::port::llm::LlmProvider;

use crate::error::{AppError, AppResult};
use crate::repo::user::SecretKeyRepository;
use crate::service::secret::SecretEncryptor;

const PROVIDER_OPENAI: &str = "openai";
const PROVIDER_GEMINI: &str = "gemini";

/// `LlmProvider` 인스턴스를 생성하는 팩토리 트레이트.
/// infra 크레이트의 구체 타입을 app 레이어에서 직접 참조하지 않도록 추상화한다.
pub trait LlmProviderFactory: Send + Sync {
    fn build_openai(
        &self,
        api_key: String,
        model: String,
        base_url: Option<String>,
    ) -> Arc<dyn LlmProvider>;

    fn build_gemini(
        &self,
        api_key: String,
        model: String,
        base_url: Option<String>,
    ) -> Arc<dyn LlmProvider>;
}

pub struct LlmKeyService {
    repo: Arc<dyn SecretKeyRepository>,
    encryptor: Arc<dyn SecretEncryptor>,
    default_llm: Arc<dyn LlmProvider>,
    factory: Arc<dyn LlmProviderFactory>,
}

impl LlmKeyService {
    pub fn new(
        repo: Arc<dyn SecretKeyRepository>,
        encryptor: Arc<dyn SecretEncryptor>,
        default_llm: Arc<dyn LlmProvider>,
        factory: Arc<dyn LlmProviderFactory>,
    ) -> Self {
        Self {
            repo,
            encryptor,
            default_llm,
            factory,
        }
    }

    /// LLM API 키를 암호화하여 저장한다.
    pub async fn store(
        &self,
        user_id: Uuid,
        provider: String,
        label: String,
        raw_api_key: &str,
    ) -> AppResult<SecretKey> {
        validate_provider(&provider)?;
        if label.trim().is_empty() {
            return Err(AppError::Validation("label cannot be empty".to_string()));
        }
        if raw_api_key.trim().is_empty() {
            return Err(AppError::Validation("api_key cannot be empty".to_string()));
        }

        // 중복 확인
        let existing = self.repo.find_by_user(user_id).await.map_err(AppError::Internal)?;
        if existing.iter().any(|k| k.provider == provider && k.label == label) {
            return Err(AppError::Conflict(format!(
                "LLM key with provider '{}' and label '{}' already exists",
                provider, label
            )));
        }

        let encrypted = self
            .encryptor
            .encrypt(raw_api_key.as_bytes())
            .map_err(AppError::Internal)?;
        let masked_hint = Some(self.encryptor.mask(raw_api_key));

        self.repo
            .create(user_id, provider, label, encrypted, masked_hint)
            .await
            .map_err(AppError::Internal)
    }

    /// 사용자 LLM 키 목록을 반환한다 (encrypted_payload 미포함).
    pub async fn list(&self, user_id: Uuid) -> AppResult<Vec<SecretKey>> {
        let all = self
            .repo
            .find_by_user(user_id)
            .await
            .map_err(AppError::Internal)?;

        Ok(all
            .into_iter()
            .filter(|k| is_llm_provider(&k.provider))
            .collect())
    }

    /// 단일 키 메타데이터를 반환한다 (소유자 검증 포함).
    pub async fn get(&self, user_id: Uuid, key_id: Uuid) -> AppResult<SecretKey> {
        let key = self
            .repo
            .find_by_id(key_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("llm key {key_id}")))?;

        if key.user_id != user_id {
            return Err(AppError::Forbidden(
                "llm key does not belong to this user".to_string(),
            ));
        }
        Ok(key)
    }

    /// 키를 삭제한다 (소유자 검증 포함).
    pub async fn delete(&self, user_id: Uuid, key_id: Uuid) -> AppResult<()> {
        self.get(user_id, key_id).await?;
        self.repo.delete(key_id).await.map_err(AppError::Internal)
    }

    /// `key_id`에 해당하는 LlmProvider를 반환한다.
    /// - `key_id == None` → 서버 기본 LLM(환경변수 기반)
    /// - `key_id == Some(id)` → 복호화 후 동적 생성
    pub async fn resolve_provider(
        &self,
        user_id: Uuid,
        key_id: Option<Uuid>,
        model_name: &str,
        base_url_override: Option<&str>,
    ) -> AppResult<Arc<dyn LlmProvider>> {
        let id = match key_id {
            None => return Ok(Arc::clone(&self.default_llm)),
            Some(id) => id,
        };

        let raw = self
            .repo
            .find_raw_by_id(id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("llm key {id}")))?;

        if raw.key.user_id != user_id {
            return Err(AppError::Forbidden(
                "llm key does not belong to this user".to_string(),
            ));
        }

        validate_provider(&raw.key.provider)?;

        let api_key = String::from_utf8(
            self.encryptor
                .decrypt(&raw.encrypted_payload)
                .map_err(AppError::Internal)?,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid utf-8 in api key: {e}")))?;

        let provider = match raw.key.provider.as_str() {
            PROVIDER_OPENAI => self.factory.build_openai(
                api_key,
                model_name.to_string(),
                base_url_override.map(str::to_owned),
            ),
            PROVIDER_GEMINI => self.factory.build_gemini(
                api_key,
                model_name.to_string(),
                base_url_override.map(str::to_owned),
            ),
            _ => return Err(AppError::Validation(format!(
                "unsupported provider: {}",
                raw.key.provider
            ))),
        };

        Ok(provider)
    }
}

fn validate_provider(provider: &str) -> AppResult<()> {
    match provider {
        PROVIDER_OPENAI | PROVIDER_GEMINI => Ok(()),
        _ => Err(AppError::Validation(format!(
            "unsupported LLM provider '{}'; supported: openai, gemini",
            provider
        ))),
    }
}

fn is_llm_provider(provider: &str) -> bool {
    matches!(provider, PROVIDER_OPENAI | PROVIDER_GEMINI)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use chrono::Utc;
    use lumos_domain::model::user::{SecretKey, SecretKeyRaw};
    use lumos_domain::port::llm::{
        CriticReview, FundamentalAnalysis, LlmProvider, NewsEventAnalysis, ScenarioOutput,
        ScenarioPromptInput, StrategyDraft,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    // ── Mock 구현체들 ─────────────────────────────────────────────────────────

    struct MockSecretKeyRepository {
        keys: Mutex<HashMap<Uuid, (SecretKey, Vec<u8>)>>,
    }

    impl MockSecretKeyRepository {
        fn new() -> Self {
            Self {
                keys: Mutex::new(HashMap::new()),
            }
        }

        fn insert(&self, key: SecretKey, payload: Vec<u8>) {
            self.keys.lock().unwrap().insert(key.id, (key, payload));
        }
    }

    #[async_trait]
    impl crate::repo::user::SecretKeyRepository for MockSecretKeyRepository {
        async fn find_by_id(&self, id: Uuid) -> Result<Option<SecretKey>> {
            Ok(self.keys.lock().unwrap().get(&id).map(|(k, _)| k.clone()))
        }

        async fn find_by_user(&self, user_id: Uuid) -> Result<Vec<SecretKey>> {
            Ok(self
                .keys
                .lock()
                .unwrap()
                .values()
                .filter(|(k, _)| k.user_id == user_id)
                .map(|(k, _)| k.clone())
                .collect())
        }

        async fn find_by_provider(&self, user_id: Uuid, provider: &str) -> Result<Vec<SecretKey>> {
            Ok(self
                .keys
                .lock()
                .unwrap()
                .values()
                .filter(|(k, _)| k.user_id == user_id && k.provider == provider)
                .map(|(k, _)| k.clone())
                .collect())
        }

        async fn create(
            &self,
            user_id: Uuid,
            provider: String,
            label: String,
            encrypted_payload: Vec<u8>,
            masked_hint: Option<String>,
        ) -> Result<SecretKey> {
            let key = SecretKey {
                id: Uuid::new_v4(),
                user_id,
                provider,
                label,
                masked_hint,
                verified_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.insert(key.clone(), encrypted_payload);
            Ok(key)
        }

        async fn delete(&self, id: Uuid) -> Result<()> {
            self.keys.lock().unwrap().remove(&id);
            Ok(())
        }

        async fn find_raw_by_id(&self, id: Uuid) -> Result<Option<SecretKeyRaw>> {
            Ok(self
                .keys
                .lock()
                .unwrap()
                .get(&id)
                .map(|(k, p)| SecretKeyRaw {
                    key: k.clone(),
                    encrypted_payload: p.clone(),
                }))
        }
    }

    struct MockEncryptor;

    impl crate::service::secret::SecretEncryptor for MockEncryptor {
        fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        fn decrypt(&self, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
            Ok(ciphertext.to_vec())
        }

        fn mask(&self, raw: &str) -> String {
            format!("{}...", &raw[..4.min(raw.len())])
        }
    }

    struct MockLlmProvider;

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn generate_scenario(&self, _: ScenarioPromptInput) -> Result<ScenarioOutput> {
            unimplemented!()
        }

        async fn analyze_fundamentals(
            &self,
            _: &str,
            _: &str,
            _: &[lumos_domain::model::scenario::EvidenceCard],
        ) -> Result<FundamentalAnalysis> {
            unimplemented!()
        }

        async fn analyze_news_events(
            &self,
            _: &str,
            _: &[lumos_domain::model::scenario::EvidenceCard],
        ) -> Result<NewsEventAnalysis> {
            unimplemented!()
        }

        async fn draft_strategy(
            &self,
            _: &ScenarioPromptInput,
            _: &FundamentalAnalysis,
            _: &NewsEventAnalysis,
        ) -> Result<StrategyDraft> {
            unimplemented!()
        }

        async fn critic_review(
            &self,
            _: &ScenarioPromptInput,
            _: &StrategyDraft,
            _: &FundamentalAnalysis,
            _: &NewsEventAnalysis,
        ) -> Result<CriticReview> {
            unimplemented!()
        }
    }

    struct MockFactory {
        built: Mutex<Vec<(String, String)>>,
    }

    impl MockFactory {
        fn new() -> Self {
            Self {
                built: Mutex::new(vec![]),
            }
        }
    }

    impl LlmProviderFactory for MockFactory {
        fn build_openai(
            &self,
            api_key: String,
            model: String,
            _base_url: Option<String>,
        ) -> Arc<dyn LlmProvider> {
            self.built.lock().unwrap().push((api_key, model));
            Arc::new(MockLlmProvider)
        }

        fn build_gemini(
            &self,
            api_key: String,
            model: String,
            _base_url: Option<String>,
        ) -> Arc<dyn LlmProvider> {
            self.built.lock().unwrap().push((api_key, model));
            Arc::new(MockLlmProvider)
        }
    }

    fn make_service(
        repo: Arc<MockSecretKeyRepository>,
        factory: Arc<MockFactory>,
    ) -> LlmKeyService {
        LlmKeyService::new(
            repo,
            Arc::new(MockEncryptor),
            Arc::new(MockLlmProvider),
            factory,
        )
    }

    // ── 테스트 ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_none_returns_default_llm() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let user_id = Uuid::new_v4();
        let provider = svc
            .resolve_provider(user_id, None, "gpt-4o", None)
            .await
            .unwrap();

        assert!(factory.built.lock().unwrap().is_empty());
        let _ = provider;
    }

    #[tokio::test]
    async fn resolve_some_builds_openai_provider() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let user_id = Uuid::new_v4();

        let key = SecretKey {
            id: Uuid::new_v4(),
            user_id,
            provider: "openai".to_string(),
            label: "my-key".to_string(),
            masked_hint: None,
            verified_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let payload = b"sk-test-api-key".to_vec();
        repo.insert(key.clone(), payload);

        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        svc.resolve_provider(user_id, Some(key.id), "gpt-4o", None)
            .await
            .unwrap();

        let built = factory.built.lock().unwrap();
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].0, "sk-test-api-key");
        assert_eq!(built[0].1, "gpt-4o");
    }

    #[tokio::test]
    async fn resolve_wrong_user_returns_forbidden() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let owner_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();

        let key = SecretKey {
            id: Uuid::new_v4(),
            user_id: owner_id,
            provider: "openai".to_string(),
            label: "key".to_string(),
            masked_hint: None,
            verified_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert(key.clone(), b"sk-secret".to_vec());

        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let result = svc
            .resolve_provider(other_id, Some(key.id), "gpt-4o", None)
            .await;

        assert!(matches!(result, Err(AppError::Forbidden(_))));
    }

    #[tokio::test]
    async fn resolve_nonexistent_returns_not_found() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let result = svc
            .resolve_provider(Uuid::new_v4(), Some(Uuid::new_v4()), "gpt-4o", None)
            .await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn store_invalid_provider_returns_validation_error() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let err = svc
            .store(Uuid::new_v4(), "anthropic".to_string(), "key".to_string(), "sk-x")
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn store_empty_label_returns_validation_error() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let err = svc
            .store(Uuid::new_v4(), "openai".to_string(), "  ".to_string(), "sk-x")
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn delete_wrong_user_returns_forbidden() {
        let repo = Arc::new(MockSecretKeyRepository::new());
        let owner_id = Uuid::new_v4();

        let key = SecretKey {
            id: Uuid::new_v4(),
            user_id: owner_id,
            provider: "openai".to_string(),
            label: "key".to_string(),
            masked_hint: None,
            verified_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.insert(key.clone(), b"sk".to_vec());

        let factory = Arc::new(MockFactory::new());
        let svc = make_service(Arc::clone(&repo), Arc::clone(&factory));

        let err = svc
            .delete(Uuid::new_v4(), key.id)
            .await
            .unwrap_err();

        assert!(matches!(err, AppError::Forbidden(_)));
    }
}
