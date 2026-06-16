use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthUser;
use lumos_app::error::AppError;
use lumos_app::repo::manager::CreateManagerInput;
use lumos_domain::model::broker::{BrokerAccount, BrokerEnvironment};
use lumos_domain::model::manager::{Manager, ManagerMode};
use lumos_domain::model::symbol::{Currency, Region};
use lumos_infra::kis::{KisClient, KisEnvironment};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_managers).post(create_manager))
        .route("/validate-kis", post(validate_kis_connection))
        .route("/verify-kis-auth", post(verify_kis_auth))
        .route("/:id", get(get_manager).delete(delete_manager))
        .route("/:id/risk-policy", get(get_risk_policy))
        .route("/:id/auto-trade", post(set_auto_trade))
}

#[derive(Debug, Serialize)]
pub struct ManagerResponse {
    pub id: Uuid,
    pub broker_connection_id: Uuid,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub auto_trade_enabled: bool,
    pub status: String,
    pub initial_capital: Decimal,
    pub llm_key_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyKisAuthResponse {
    pub success: bool,
    pub message: String,
}

impl From<Manager> for ManagerResponse {
    fn from(m: Manager) -> Self {
        Self {
            id: m.id,
            broker_connection_id: m.broker_connection_id,
            name: m.name,
            mode: format!("{:?}", m.mode).to_lowercase(),
            region: m.region.to_string(),
            base_currency: m.base_currency.to_string(),
            auto_trade_enabled: m.auto_trade_enabled,
            status: format!("{:?}", m.status).to_lowercase(),
            initial_capital: m.initial_capital,
            llm_key_id: m.llm_key_id,
            model_provider: m.model_provider,
            model_name: m.model_name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateManagerRequest {
    #[serde(default)]
    pub broker_connection_id: Option<Uuid>,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    #[serde(default)]
    pub initial_capital: Option<Decimal>,
    /// 연결할 LLM 키 ID. None이면 서버 기본 LLM 사용.
    #[serde(default)]
    pub llm_key_id: Option<Uuid>,
    /// LLM 프로바이더 ('openai' | 'gemini'). 미지정 시 'openai'.
    #[serde(default)]
    pub model_provider: Option<String>,
    /// 모델명 (예: 'gpt-4o-mini'). 미지정 시 기본값 사용.
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub kis_app_key: Option<String>,
    #[serde(default)]
    pub kis_app_secret: Option<String>,
    #[serde(default)]
    pub kis_account_no: Option<String>,
    #[serde(default)]
    pub kis_account_product: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateKisConnectionRequest {
    pub app_key: String,
    pub app_secret: String,
    pub account_no: String,
    #[serde(default)]
    pub account_product: Option<String>,
    pub mode: String,
    pub region: String,
}

async fn list_managers(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> ApiResult<Json<Vec<ManagerResponse>>> {
    let managers = state
        .manager_service
        .list_for_user(auth_user.user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        managers.into_iter().map(ManagerResponse::from).collect(),
    ))
}

async fn get_manager(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ManagerResponse>> {
    let manager = state
        .manager_service
        .get(id)
        .await
        .map_err(ApiError::from)?;
    if manager.user_id != auth_user.user_id {
        return Err(ApiError::from(AppError::Forbidden(
            "manager does not belong to this user".to_string(),
        )));
    }
    Ok(Json(ManagerResponse::from(manager)))
}

async fn delete_manager(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state
        .manager_service
        .delete(auth_user.user_id, id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_manager(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<CreateManagerRequest>,
) -> ApiResult<Json<ManagerResponse>> {
    let mode = match req.mode.as_str() {
        "live" => ManagerMode::Live,
        _ => ManagerMode::Paper,
    };
    let region = match req.region.as_str() {
        "US" => Region::Us,
        _ => Region::Kr,
    };
    let currency = match req.base_currency.as_str() {
        "USD" => Currency::Usd,
        _ => Currency::Krw,
    };
    let broker_connection = resolve_broker_connection(
        &state,
        auth_user.user_id,
        req.broker_connection_id,
        req.kis_app_key.as_deref(),
        req.kis_app_secret.as_deref(),
        req.kis_account_no.as_deref(),
        req.kis_account_product.as_deref(),
        mode.clone(),
        region.clone(),
    )
    .await?;
    let initial_capital = match mode {
        ManagerMode::Paper => match req.initial_capital {
            Some(value) if value > Decimal::ZERO => value,
            _ => {
                return Err(ApiError::from(AppError::Validation(
                    "initial_capital is required for paper managers".to_string(),
                )))
            }
        },
        ManagerMode::Live => broker_connection
            .account
            .as_ref()
            .map(|account| account.total_equity)
            .unwrap_or(Decimal::ZERO),
    };

    // LLM 연결 설정 결정. llm_key_id가 주어지면 소유권을 검증한다.
    if let Some(key_id) = req.llm_key_id {
        state
            .llm_key_service
            .get(auth_user.user_id, key_id)
            .await
            .map_err(ApiError::from)?;
    }
    let model_provider = req
        .model_provider
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "openai".to_string());
    let model_name = req
        .model_name
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let input = CreateManagerInput {
        user_id: auth_user.user_id,
        broker_connection_id: broker_connection.id,
        name: req.name,
        mode,
        region,
        base_currency: currency,
        initial_capital,
        llm_key_id: req.llm_key_id,
        model_provider,
        model_name,
    };

    let manager = state
        .manager_service
        .create(input)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ManagerResponse::from(manager)))
}

async fn validate_kis_connection(
    State(_state): State<AppState>,
    _auth_user: AuthUser,
    Json(req): Json<ValidateKisConnectionRequest>,
) -> ApiResult<Json<BrokerAccount>> {
    let mode = match req.mode.as_str() {
        "live" => ManagerMode::Live,
        _ => ManagerMode::Paper,
    };
    let region = match req.region.as_str() {
        "US" => Region::Us,
        _ => Region::Kr,
    };
    let environment = match mode {
        ManagerMode::Live => BrokerEnvironment::Real,
        _ => BrokerEnvironment::Paper,
    };
    let product = req
        .account_product
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "01".to_string());
    let client = build_kis_client(
        &req.app_key,
        &req.app_secret,
        &req.account_no,
        &product,
        environment,
    );

    validate_kis_balance(&client, region)
        .await
        .map(Json)
        .map_err(ApiError::from)
}

async fn verify_kis_auth(
    State(_state): State<AppState>,
    _auth_user: AuthUser,
    Json(req): Json<ValidateKisConnectionRequest>,
) -> ApiResult<Json<VerifyKisAuthResponse>> {
    // 입력값 검증만 수행 (토큰 발급 X, API 호출 제한 회피)
    if req.app_key.trim().is_empty() {
        return Ok(Json(VerifyKisAuthResponse {
            success: false,
            message: "App Key가 비어있습니다".to_string(),
        }));
    }
    if req.app_secret.trim().is_empty() {
        return Ok(Json(VerifyKisAuthResponse {
            success: false,
            message: "App Secret이 비어있습니다".to_string(),
        }));
    }
    if req.account_no.trim().is_empty() {
        return Ok(Json(VerifyKisAuthResponse {
            success: false,
            message: "계좌번호가 비어있습니다".to_string(),
        }));
    }

    // 입력값이 유효하면 성공 반환 (실제 인증은 validate_kis_connection에서)
    Ok(Json(VerifyKisAuthResponse {
        success: true,
        message: "입력값 확인 완료".to_string(),
    }))
}

struct BrokerConnectionResolution {
    id: Uuid,
    account: Option<BrokerAccount>,
}

async fn resolve_broker_connection(
    state: &AppState,
    user_id: Uuid,
    requested_id: Option<Uuid>,
    kis_app_key: Option<&str>,
    kis_app_secret: Option<&str>,
    kis_account_no: Option<&str>,
    kis_account_product: Option<&str>,
    mode: ManagerMode,
    region: Region,
) -> ApiResult<BrokerConnectionResolution> {
    if let Some(id) = requested_id.filter(|id| *id != Uuid::nil()) {
        return Ok(BrokerConnectionResolution { id, account: None });
    }

    let has_kis_creds = kis_app_key
        .and_then(|v| (!v.trim().is_empty()).then_some(v))
        .is_some()
        && kis_app_secret
            .and_then(|v| (!v.trim().is_empty()).then_some(v))
            .is_some()
        && kis_account_no
            .and_then(|v| (!v.trim().is_empty()).then_some(v))
            .is_some();

    if has_kis_creds {
        let app_key = kis_app_key.unwrap().trim();
        let app_secret = kis_app_secret.unwrap().trim();
        let account_no = kis_account_no.unwrap().trim();
        let account_product = kis_account_product
            .and_then(|v| {
                let trimmed = v.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| "01".to_string());

        return create_kis_broker_connection(
            state,
            user_id,
            app_key,
            app_secret,
            account_no,
            &account_product,
            mode,
            region,
        )
        .await;
    }

    let existing = state
        .broker_connection_repo
        .find_by_user(user_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    if let Some(conn) = existing.into_iter().next() {
        return Ok(BrokerConnectionResolution {
            id: conn.id,
            account: None,
        });
    }

    // 모의(paper) 모드는 실제 KIS 자격증명 없이도 broker connection을 만들 수 있어야 한다.
    // 라이브 모드만 환경변수의 KIS 키를 요구한다.
    match mode {
        ManagerMode::Paper => create_paper_broker_connection(state, user_id).await,
        ManagerMode::Live => create_env_broker_connection(state, user_id).await,
    }
}

/// 모의 모드 전용 broker connection. 실제 KIS 인증 없이 placeholder 시크릿으로 생성한다.
async fn create_paper_broker_connection(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<BrokerConnectionResolution> {
    let app_key_secret_id =
        ensure_paper_secret(state, user_id, "paper_app_key", "paper-app-key").await?;
    let app_secret_secret_id =
        ensure_paper_secret(state, user_id, "paper_app_secret", "paper-app-secret").await?;
    let account_no = "paper-account";
    let account_no_encrypted = state
        .secret_service
        .encrypt_payload(account_no.as_bytes())
        .map_err(ApiError::from)?;
    let account_no_masked = format!("{}-01", state.secret_service.mask(account_no));

    let conn = state
        .broker_connection_repo
        .create(
            user_id,
            BrokerEnvironment::Paper,
            account_no_masked,
            account_no_encrypted,
            app_key_secret_id,
            app_secret_secret_id,
        )
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(BrokerConnectionResolution {
        id: conn.id,
        account: None,
    })
}

/// 모의 모드용 placeholder 시크릿을 멱등하게 확보한다 (이미 있으면 재사용).
async fn ensure_paper_secret(
    state: &AppState,
    user_id: Uuid,
    label: &str,
    raw_value: &str,
) -> ApiResult<Uuid> {
    let existing = state.secret_service.list_for_user(user_id).await?;
    if let Some(secret) = existing
        .into_iter()
        .find(|secret| secret.provider == "kis" && secret.label == label)
    {
        return Ok(secret.id);
    }

    let secret = state
        .secret_service
        .store(user_id, "kis".to_string(), label.to_string(), raw_value)
        .await?;
    Ok(secret.id)
}

async fn create_kis_broker_connection(
    state: &AppState,
    user_id: Uuid,
    app_key: &str,
    app_secret: &str,
    account_no: &str,
    account_product: &str,
    mode: ManagerMode,
    region: Region,
) -> ApiResult<BrokerConnectionResolution> {
    let environment = match mode {
        ManagerMode::Live => BrokerEnvironment::Real,
        _ => BrokerEnvironment::Paper,
    };
    // 모의(paper) 모드는 실제 KIS 인증/잔고 조회 없이 입력한 자본금을 사용한다.
    // 라이브 모드에서만 실제 계좌 잔고를 검증한다.
    let account = match mode {
        ManagerMode::Live => {
            let client = build_kis_client(
                app_key,
                app_secret,
                account_no,
                account_product,
                environment.clone(),
            );
            Some(validate_kis_balance(&client, region).await?)
        }
        ManagerMode::Paper => None,
    };

    let app_key_secret = state
        .secret_service
        .store(
            user_id,
            "kis".to_string(),
            format!("kis_app_key:{}", account_no),
            app_key,
        )
        .await?;
    let app_secret_secret = state
        .secret_service
        .store(
            user_id,
            "kis".to_string(),
            format!("kis_app_secret:{}", account_no),
            app_secret,
        )
        .await?;
    let account_no_encrypted = state
        .secret_service
        .encrypt_payload(account_no.as_bytes())
        .map_err(ApiError::from)?;
    let account_no_masked = format!(
        "{}-{}",
        state.secret_service.mask(account_no),
        account_product
    );

    let conn = state
        .broker_connection_repo
        .create(
            user_id,
            environment,
            account_no_masked,
            account_no_encrypted,
            app_key_secret.id,
            app_secret_secret.id,
        )
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(BrokerConnectionResolution {
        id: conn.id,
        account,
    })
}

async fn validate_kis_balance(client: &KisClient, region: Region) -> ApiResult<BrokerAccount> {
    // Ensure we have a bearer token before calling online endpoints.
    // If token issuance fails, surface as internal error.
    if client.requires_access_token_for_balance() {
        if let Err(e) = client.issue_access_token().await {
            return Err(ApiError::from(AppError::Internal(e)));
        }
    }
    let (account, _) = match region {
        Region::Kr => client.domestic_balance().await,
        Region::Us => client.overseas_balance("NAS").await,
    }
    .map_err(|e| ApiError::from(AppError::Internal(e)))?;
    Ok(account)
}

fn build_kis_client(
    app_key: &str,
    app_secret: &str,
    account_no: &str,
    account_product: &str,
    environment: BrokerEnvironment,
) -> KisClient {
    let env = match environment {
        BrokerEnvironment::Real => KisEnvironment::Real,
        BrokerEnvironment::Paper => KisEnvironment::Paper,
    };
    KisClient::new(
        env,
        app_key.to_string(),
        app_secret.to_string(),
        account_no.to_string(),
        account_product.to_string(),
    )
}

async fn create_env_broker_connection(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<BrokerConnectionResolution> {
    let app_key = required_env("KIS_APP_KEY")?;
    let app_secret = required_env("KIS_APP_SECRET")?;
    let account_no = required_env("KIS_ACCOUNT_NO")?;
    let account_product = std::env::var("KIS_ACCOUNT_PRODUCT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "01".to_string());
    let environment = match std::env::var("KIS_ENV")
        .unwrap_or_else(|_| "paper".to_string())
        .to_lowercase()
        .as_str()
    {
        "real" => BrokerEnvironment::Real,
        _ => BrokerEnvironment::Paper,
    };

    let app_key_secret_id = ensure_env_secret(state, user_id, "app_key", &app_key).await?;
    let app_secret_secret_id = ensure_env_secret(state, user_id, "app_secret", &app_secret).await?;
    let account_no_encrypted = state
        .secret_service
        .encrypt_payload(account_no.as_bytes())
        .map_err(ApiError::from)?;
    let account_no_masked = format!(
        "{}-{}",
        state.secret_service.mask(&account_no),
        account_product
    );

    let conn = state
        .broker_connection_repo
        .create(
            user_id,
            environment,
            account_no_masked,
            account_no_encrypted,
            app_key_secret_id,
            app_secret_secret_id,
        )
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?;

    Ok(BrokerConnectionResolution {
        id: conn.id,
        account: None,
    })
}

async fn ensure_env_secret(
    state: &AppState,
    user_id: Uuid,
    label: &str,
    raw_value: &str,
) -> ApiResult<Uuid> {
    let existing = state.secret_service.list_for_user(user_id).await?;
    if let Some(secret) = existing
        .into_iter()
        .find(|secret| secret.provider == "kis" && secret.label == label)
    {
        return Ok(secret.id);
    }

    let secret = state
        .secret_service
        .store(user_id, "kis".to_string(), label.to_string(), raw_value)
        .await?;
    Ok(secret.id)
}

fn required_env(key: &str) -> ApiResult<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::from(AppError::Validation(format!(
                "{key} is required to create the default KIS broker connection"
            )))
        })
}

async fn get_risk_policy(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<lumos_domain::model::risk::RiskPolicy>> {
    let manager = state
        .manager_service
        .get(id)
        .await
        .map_err(ApiError::from)?;
    if manager.user_id != auth_user.user_id {
        return Err(ApiError::from(AppError::Forbidden(
            "manager does not belong to this user".to_string(),
        )));
    }
    let policy = state
        .manager_service
        .get_risk_policy(id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(policy))
}

#[derive(Debug, Deserialize)]
pub struct SetAutoTradeRequest {
    pub enabled: bool,
}

async fn set_auto_trade(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<SetAutoTradeRequest>,
) -> ApiResult<Json<ManagerResponse>> {
    let existing = state
        .manager_service
        .get(id)
        .await
        .map_err(ApiError::from)?;
    if existing.user_id != auth_user.user_id {
        return Err(ApiError::from(AppError::Forbidden(
            "manager does not belong to this user".to_string(),
        )));
    }
    let manager = state
        .manager_service
        .set_auto_trade(id, req.enabled)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ManagerResponse::from(manager)))
}
