use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{
    AnalysisReportDto, HoldingDto, ManagerDto, RiskPolicyDto, ScenarioItemDto, ScenarioRunDto,
    TradeDto,
};

const AUTH_TOKEN_STORAGE_KEY: &str = "lumos.dev.jwt";

#[derive(Debug, Serialize)]
pub struct CreateManagerRequest {
    pub broker_connection_id: Option<Uuid>,
    pub name: String,
    pub mode: String,
    pub region: String,
    pub base_currency: String,
    pub initial_capital: f64,
}

#[derive(Debug, Deserialize)]
pub struct DevTokenResponse {
    pub token: String,
    pub user_id: Uuid,
    pub expires_in_hours: i64,
}

fn base_url() -> &'static str {
    option_env!("API_BASE_URL").unwrap_or("/api")
}

pub fn save_auth_token(token: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(AUTH_TOKEN_STORAGE_KEY, token);
    }
}

fn auth_token() -> Option<String> {
    local_storage()?
        .get_item(AUTH_TOKEN_STORAGE_KEY)
        .ok()
        .flatten()
        .filter(|token| !token.trim().is_empty())
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

async fn get_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let url = format!("{}{}", base_url(), path);
    let mut req = gloo_net::http::Request::get(&url);
    if let Some(token) = auth_token() {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }

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
    let mut req = gloo_net::http::Request::post(&url);
    if let Some(token) = auth_token() {
        req = req.header("Authorization", &format!("Bearer {token}"));
    }

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

// ─── Manager ─────────────────────────────────────────────────────────────────

pub async fn list_managers() -> Result<Vec<ManagerDto>> {
    get_json("/managers").await
}

pub async fn get_manager(id: Uuid) -> Result<ManagerDto> {
    get_json(&format!("/managers/{id}")).await
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

// ─── Holdings ─────────────────────────────────────────────────────────────────

pub async fn list_holdings(manager_id: Uuid) -> Result<Vec<HoldingDto>> {
    get_json(&format!("/managers/{manager_id}/holdings")).await
}

// ─── Trades ───────────────────────────────────────────────────────────────────

pub async fn list_trades(manager_id: Uuid) -> Result<Vec<TradeDto>> {
    get_json(&format!("/managers/{manager_id}/trades")).await
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

// ─── Auth (Dev) ───────────────────────────────────────────────────────────────

pub async fn get_dev_token(user_id: Option<Uuid>) -> Result<DevTokenResponse> {
    #[derive(Serialize)]
    struct Req {
        user_id: Option<Uuid>,
    }
    post_json("/auth/token", &Req { user_id }).await
}
