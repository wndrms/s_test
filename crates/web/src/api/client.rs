use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{
    AnalysisReportDto, HoldingDto, ManagerDto, ManagerSymbolDto, RiskPolicyDto, ScenarioItemDto,
    ScenarioRunDto, ScheduleDto, TradeDto, TradeCycleDto, UpsertScheduleRequest,
};
use lumos_domain::model::broker::BrokerAccount;

pub use super::types::{LlmKeyDto, ManagerSymbolDto as ManagerSymbolDtoExport, SymbolDto};

#[derive(Debug, Serialize)]
pub struct CreateManagerRequest {
    pub broker_connection_id: Option<Uuid>,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_capital: Option<f64>,
    #[serde(default)]
    pub kis_app_key: Option<String>,
    #[serde(default)]
    pub kis_app_secret: Option<String>,
    #[serde(default)]
    pub kis_account_no: Option<String>,
    #[serde(default)]
    pub kis_account_product: Option<String>,
}

fn base_url() -> &'static str {
    option_env!("API_BASE_URL").unwrap_or("/api")
}

async fn get_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    get_json_internal(path).await
}

async fn get_json_internal<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let url = format!("{}{}", base_url(), path);
    let req = gloo_net::http::Request::get(&url);

    let resp = req
        .send()
        .await
        .with_context(|| format!("GET {url} failed"))?;

    if !resp.ok() {
        anyhow::bail!("GET {url} returned {}", resp.status());
    }

    let data = resp
        .json::<T>()
        .await
        .with_context(|| format!("GET {url} json parse failed"))?;

    Ok(data)
}

async fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T> {
    let url = format!("{}{}", base_url(), path);
    let req = gloo_net::http::Request::post(&url);

    let req = req
        .json(body)
        .with_context(|| format!("POST {url} serialize failed"))?;

    let resp = req
        .send()
        .await
        .with_context(|| format!("POST {url} failed"))?;

    if !resp.ok() {
        anyhow::bail!("POST {url} returned {}", resp.status());
    }

    resp.json::<T>()
        .await
        .with_context(|| format!("POST {url} json parse failed"))
}

/// 본문 없는 응답을 기대하는 PATCH 요청 (성공 여부만 확인).
async fn patch_json_no_content<B: serde::Serialize>(path: &str, body: &B) -> Result<()> {
    let url = format!("{}{}", base_url(), path);
    let req = gloo_net::http::Request::patch(&url)
        .json(body)
        .with_context(|| format!("PATCH {url} serialize failed"))?;

    let resp = req
        .send()
        .await
        .with_context(|| format!("PATCH {url} failed"))?;

    if !resp.ok() {
        anyhow::bail!("PATCH {url} returned {}", resp.status());
    }
    Ok(())
}

// ─── Manager ─────────────────────────────────────────────────────────────────

pub async fn list_managers() -> Result<Vec<ManagerDto>> {
    get_json("/managers").await
}

pub async fn get_manager(id: Uuid) -> Result<ManagerDto> {
    get_json(&format!("/managers/{id}")).await
}

pub async fn delete_manager(id: Uuid) -> Result<()> {
    delete_json(&format!("/managers/{id}")).await
}

pub async fn get_risk_policy(id: Uuid) -> Result<RiskPolicyDto> {
    get_json(&format!("/managers/{id}/risk-policy")).await
}

// ─── Scenarios ────────────────────────────────────────────────────────────────

pub async fn list_scenario_runs(manager_id: Uuid) -> Result<Vec<ScenarioRunDto>> {
    get_json(&format!("/managers/{manager_id}/scenarios/runs")).await
}

pub async fn list_scenario_items(manager_id: Uuid, run_id: Uuid) -> Result<Vec<ScenarioItemDto>> {
    get_json(&format!(
        "/managers/{manager_id}/scenarios/runs/{run_id}/items"
    ))
    .await
}

/// 최근 run의 items를 한 번에 반환 (UI 편의용)
pub async fn list_scenarios(manager_id: Uuid) -> Result<Vec<ScenarioItemDto>> {
    let runs = list_scenario_runs(manager_id).await?;
    let latest = match runs.into_iter().next() {
        Some(r) => r,
        None => return Ok(vec![]),
    };
    list_scenario_items(manager_id, latest.id).await
}

// ─── Schedule ─────────────────────────────────────────────────────────────────

/// 매니저 스케줄 조회. 미설정이면 None.
pub async fn get_schedule(manager_id: Uuid) -> Result<Option<ScheduleDto>> {
    get_json(&format!("/managers/{manager_id}/schedule")).await
}

/// 매니저 스케줄 저장 (전체 슬롯 덮어쓰기).
pub async fn save_schedule(manager_id: Uuid, req: &UpsertScheduleRequest) -> Result<()> {
    patch_json_no_content(&format!("/managers/{manager_id}/schedule"), req).await
}

// ─── Holdings ─────────────────────────────────────────────────────────────────

pub async fn list_holdings(manager_id: Uuid) -> Result<Vec<HoldingDto>> {
    get_json(&format!("/managers/{manager_id}/holdings")).await
}

// ─── Trades ───────────────────────────────────────────────────────────────────

pub async fn list_trades(manager_id: Uuid) -> Result<Vec<TradeDto>> {
    get_json(&format!("/managers/{manager_id}/trades")).await
}

pub async fn list_trade_cycles(manager_id: Uuid) -> Result<Vec<TradeCycleDto>> {
    get_json(&format!("/managers/{manager_id}/trade-cycles")).await
}

// ─── Analysis Reports ─────────────────────────────────────────────────────────

pub async fn get_analysis_report(manager_id: Uuid, report_id: Uuid) -> Result<AnalysisReportDto> {
    get_json(&format!(
        "/managers/{manager_id}/analysis-reports/{report_id}"
    ))
    .await
}

// ─── Manager Creation ─────────────────────────────────────────────────────────

pub async fn create_manager(req: CreateManagerRequest) -> Result<ManagerDto> {
    post_json("/managers", &req).await
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidateKisConnectionRequest {
    pub app_key: String,
    pub app_secret: String,
    pub account_no: String,
    pub account_product: Option<String>,
    pub mode: String,
    pub region: String,
}

pub async fn validate_kis_connection(req: ValidateKisConnectionRequest) -> Result<BrokerAccount> {
    post_json("/managers/validate-kis", &req).await
}

#[derive(Debug, Deserialize, Clone)]
pub struct VerifyKisAuthResponse {
    pub success: bool,
    pub message: String,
}

pub async fn verify_kis_auth(req: ValidateKisConnectionRequest) -> Result<VerifyKisAuthResponse> {
    post_json("/managers/verify-kis-auth", &req).await
}

// ─── Auth (Dev) ───────────────────────────────────────────────────────────────
// 자동 인증으로 변경되어 더 이상 토큰 발급이 필요 없음

// ─── LLM Keys ─────────────────────────────────────────────────────────────────

pub async fn fetch_llm_keys() -> Result<Vec<LlmKeyDto>> {
    get_json("/llm-keys").await
}

#[derive(Debug, Serialize)]
struct CreateLlmKeyRequest {
    provider: String,
    label: String,
    api_key: String,
}

pub async fn create_llm_key(provider: &str, label: &str, api_key: &str) -> Result<LlmKeyDto> {
    post_json(
        "/llm-keys",
        &CreateLlmKeyRequest {
            provider: provider.to_string(),
            label: label.to_string(),
            api_key: api_key.to_string(),
        },
    )
    .await
}

pub async fn delete_llm_key(key_id: Uuid) -> Result<()> {
    delete_json(&format!("/llm-keys/{key_id}")).await
}

// ─── Symbols ──────────────────────────────────────────────────────────────────

pub async fn search_symbols(query: &str, region: Option<&str>) -> Result<Vec<SymbolDto>> {
    let encoded_query = query.replace(' ', "%20").replace('&', "%26");
    let mut url = format!("/symbols/search?q={}", encoded_query);
    if let Some(r) = region {
        url.push_str(&format!("&region={}", r));
    }
    get_json(&url).await
}

pub async fn list_symbols() -> Result<Vec<SymbolDto>> {
    get_json("/symbols").await
}

async fn delete_json(path: &str) -> Result<()> {
    let url = format!("{}{}", base_url(), path);
    let req = gloo_net::http::Request::delete(&url);

    let resp = req
        .send()
        .await
        .with_context(|| format!("DELETE {} failed", path))?;

    if !resp.ok() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "(failed to read body)".to_string());
        anyhow::bail!("DELETE {} returned {}: {}", path, status, body);
    }

    Ok(())
}

// ─── Manager Universe ─────────────────────────────────────────────────────────

pub async fn fetch_manager_symbols(manager_id: Uuid) -> Result<Vec<ManagerSymbolDto>> {
    get_json(&format!("/managers/{}/universe", manager_id)).await
}

#[derive(Debug, Serialize)]
struct SetSymbolsRequest {
    symbol_ids: Vec<Uuid>,
}

pub async fn set_manager_symbols(manager_id: Uuid, symbol_ids: Vec<Uuid>) -> Result<()> {
    post_json::<_, serde_json::Value>(
        &format!("/managers/{}/universe", manager_id),
        &SetSymbolsRequest { symbol_ids },
    )
    .await?;
    Ok(())
}
