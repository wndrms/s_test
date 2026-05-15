use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use lumos_app::repo::manager::ManagerRepository;
use lumos_app::repo::manager::RiskPolicyRepository;
use lumos_app::repo::order_plan::OrderPlanRepository;
use lumos_app::repo::scenario::ScenarioItemRepository;
use lumos_app::repo::schedule::{ManagerScheduleRepository, ScheduleRunRepository};
use lumos_app::repo::symbol::SymbolRepository;
use lumos_app::service::order_plan::OrderPlanService;
use lumos_app::service::scenario::ScenarioService;
use lumos_domain::model::scenario::EvidenceCard;
use lumos_domain::model::schedule::{Market, RunType, ScheduleRunStatus};
use lumos_infra::kis::client::KisClient;
use lumos_infra::providers::naver_finance::NaverFinanceClient;
use lumos_infra::scenario::evidence_builder;

const TICK_SECS: u64 = 30;
/// 5분 슬롯 내에서 실행 허용 오차 (초)
const SLOT_WINDOW_SECS: i64 = 90;
/// KIS 투자자 수급 조회 기본 일수
const INVESTOR_FLOW_DAYS: u32 = 5;

pub struct Scheduler {
    schedule_repo: Arc<dyn ManagerScheduleRepository>,
    run_repo: Arc<dyn ScheduleRunRepository>,
    scenario_svc: Arc<ScenarioService>,
    symbol_repo: Arc<dyn SymbolRepository>,
    manager_repo: Arc<dyn ManagerRepository>,
    scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    order_plan_svc: Option<Arc<OrderPlanService>>,
    kis_client: Option<Arc<KisClient>>,
    naver_client: Arc<NaverFinanceClient>,
}

impl Scheduler {
    pub fn new(
        schedule_repo: Arc<dyn ManagerScheduleRepository>,
        run_repo: Arc<dyn ScheduleRunRepository>,
        scenario_svc: Arc<ScenarioService>,
        symbol_repo: Arc<dyn SymbolRepository>,
        manager_repo: Arc<dyn ManagerRepository>,
        scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    ) -> Self {
        Self {
            schedule_repo,
            run_repo,
            scenario_svc,
            symbol_repo,
            manager_repo,
            scenario_item_repo,
            order_plan_svc: None,
            kis_client: None,
            naver_client: Arc::new(NaverFinanceClient::new()),
        }
    }

    pub fn with_kis_client(mut self, client: Arc<KisClient>) -> Self {
        self.kis_client = Some(client);
        self
    }

    pub fn with_order_plan_svc(mut self, svc: Arc<OrderPlanService>) -> Self {
        self.order_plan_svc = Some(svc);
        self
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(TICK_SECS));
        loop {
            ticker.tick().await;
            let now = Utc::now();
            if let Err(e) = self.tick(now).await {
                tracing::error!("scheduler tick error: {e:?}");
            }
        }
    }

    async fn tick(&self, now: DateTime<Utc>) -> Result<()> {
        let schedules = self.schedule_repo.find_enabled().await?;
        tracing::debug!("tick: {} active schedules", schedules.len());

        for sched in schedules {
            let tz: Tz = sched.timezone.parse().unwrap_or(chrono_tz::UTC);
            let local_now = now.with_timezone(&tz);
            let local_time = local_now.time();

            let (market_open, market_close) = sched.market.trading_hours();
            if local_time < market_open || local_time >= market_close {
                continue;
            }

            let aligned = align_to_5min(local_now.naive_local().time());
            if time_diff_secs(local_time, aligned).abs() > SLOT_WINDOW_SECS {
                continue;
            }

            let slots = self.schedule_repo.find_slots(sched.id).await?;
            for slot in slots
                .iter()
                .filter(|s| s.enabled && s.time_of_day == aligned)
            {
                let scheduled_for = slot_utc_time(&sched.market, now, aligned);

                if slot.run_scenario {
                    self.maybe_run(sched.manager_id, slot.id, RunType::Scenario, scheduled_for)
                        .await;
                }

                if slot.run_trade {
                    self.maybe_run(sched.manager_id, slot.id, RunType::Trade, scheduled_for)
                        .await;
                }
            }
        }
        Ok(())
    }

    async fn maybe_run(
        &self,
        manager_id: Uuid,
        slot_id: Uuid,
        run_type: RunType,
        scheduled_for: DateTime<Utc>,
    ) {
        let type_str = match run_type {
            RunType::Scenario => "scenario",
            RunType::Trade => "trade",
        };
        let key = idempotency_key(manager_id, slot_id, scheduled_for, type_str);

        let run = match self
            .run_repo
            .create_if_not_exists(manager_id, slot_id, type_str, scheduled_for, &key)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::debug!("skipping duplicate run: {key}");
                return;
            }
            Err(e) => {
                tracing::error!("failed to create schedule_run {key}: {e:?}");
                return;
            }
        };

        if let Err(e) = self
            .run_repo
            .update_status(run.id, ScheduleRunStatus::Running, None)
            .await
        {
            tracing::error!("failed to mark running {}: {e:?}", run.id);
        }

        match run_type {
            RunType::Scenario => self.run_scenario_job(manager_id, run.id, slot_id).await,
            RunType::Trade => self.run_trade_job(manager_id, run.id).await,
        }
    }

    async fn run_scenario_job(&self, manager_id: Uuid, run_id: Uuid, slot_id: Uuid) {
        tracing::info!("running scenario job for manager {manager_id}, run {run_id}");

        // active symbols를 모두 조회해서 각 심볼별로 시나리오 생성
        let symbols = match self.symbol_repo.find_active().await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("failed to load active symbols: {e:?}");
                let _ = self
                    .run_repo
                    .update_status(run_id, ScheduleRunStatus::Failed, Some(e.to_string()))
                    .await;
                return;
            }
        };

        if symbols.is_empty() {
            tracing::warn!("no active symbols, skipping scenario job");
            let _ = self
                .run_repo
                .update_status(run_id, ScheduleRunStatus::Skipped, None)
                .await;
            return;
        }

        let mut any_error: Option<String> = None;
        for symbol in &symbols {
            let extra_evidence = self.collect_extra_evidence(&symbol.code, symbol.id).await;

            let result = self
                .scenario_svc
                .run_for_symbol(
                    manager_id,
                    symbol.id,
                    Some(slot_id),
                    "mock".to_string(),
                    "mock-v1".to_string(),
                    "v1".to_string(),
                    "0".to_string(),
                    extra_evidence,
                )
                .await;

            match result {
                Ok(scenario_run_id) => {
                    tracing::info!(
                        "scenario run {scenario_run_id} created for symbol {}",
                        symbol.code
                    );
                }
                Err(e) => {
                    tracing::error!("scenario failed for symbol {}: {e:?}", symbol.code);
                    any_error = Some(e.to_string());
                }
            }
        }

        let final_status = if any_error.is_some() {
            ScheduleRunStatus::Failed
        } else {
            ScheduleRunStatus::Success
        };
        let _ = self
            .run_repo
            .update_status(run_id, final_status, any_error)
            .await;
    }

    async fn run_trade_job(&self, manager_id: Uuid, run_id: Uuid) {
        tracing::info!("running trade job for manager {manager_id}, run {run_id}");

        let manager = match self.manager_repo.find_by_id(manager_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                tracing::warn!("manager {manager_id} not found, skipping trade job");
                let _ = self
                    .run_repo
                    .update_status(run_id, ScheduleRunStatus::Skipped, None)
                    .await;
                return;
            }
            Err(e) => {
                tracing::error!("failed to load manager {manager_id}: {e}");
                let _ = self
                    .run_repo
                    .update_status(run_id, ScheduleRunStatus::Failed, Some(e.to_string()))
                    .await;
                return;
            }
        };

        let Some(order_plan_svc) = &self.order_plan_svc else {
            tracing::warn!(
                "order_plan_svc not configured, skipping trade job for manager {manager_id}"
            );
            let _ = self
                .run_repo
                .update_status(run_id, ScheduleRunStatus::Skipped, None)
                .await;
            return;
        };

        let items = match self
            .scenario_item_repo
            .find_pending_for_manager(manager_id)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("failed to load scenario items for {manager_id}: {e}");
                let _ = self
                    .run_repo
                    .update_status(run_id, ScheduleRunStatus::Failed, Some(e.to_string()))
                    .await;
                return;
            }
        };

        if items.is_empty() {
            tracing::info!(
                "no pending scenario items for manager {manager_id}, skipping trade job"
            );
            let _ = self
                .run_repo
                .update_status(run_id, ScheduleRunStatus::Skipped, None)
                .await;
            return;
        }

        let mut any_error: Option<String> = None;
        for item in &items {
            match order_plan_svc
                .create_from_scenario_item(manager_id, item.id)
                .await
            {
                Ok(plan) => {
                    tracing::info!(
                        "order plan {} created (risk: {})",
                        plan.id,
                        plan.risk_status
                    );
                    if manager.auto_trade_enabled {
                        if let Err(e) = order_plan_svc
                            .execute_approved(&plan, manager.broker_connection_id)
                            .await
                        {
                            tracing::warn!("execute_approved failed for plan {}: {e}", plan.id);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("create_from_scenario_item failed for item {}: {e}", item.id);
                    any_error = Some(e.to_string());
                }
            }
        }

        let final_status = if any_error.is_some() {
            ScheduleRunStatus::Failed
        } else {
            ScheduleRunStatus::Success
        };
        let _ = self
            .run_repo
            .update_status(run_id, final_status, any_error)
            .await;
    }

    /// KIS 투자자 수급 + 네이버 컨센서스를 best-effort로 수집해 EvidenceCard 반환
    async fn collect_extra_evidence(
        &self,
        symbol_code: &str,
        symbol_id: Uuid,
    ) -> Vec<EvidenceCard> {
        let mut cards = vec![];

        // KIS 투자자 수급 (공식, 신뢰도 높음)
        if let Some(kis) = &self.kis_client {
            match kis
                .domestic_investor_flow(symbol_code, INVESTOR_FLOW_DAYS)
                .await
            {
                Ok(flow) if !flow.is_empty() => {
                    cards.push(evidence_builder::from_investor_flow(symbol_id, &flow));
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("KIS investor flow skipped for {symbol_code}: {e}");
                }
            }
        }

        // 네이버 컨센서스 (비공식, best-effort)
        let naver_data = self.naver_client.fetch_integration(symbol_code).await;
        if let Some(card) = evidence_builder::from_naver_consensus(symbol_id, &naver_data) {
            cards.push(card);
        }

        cards
    }
}

/// 현재 시각을 5분 슬롯으로 내림
fn align_to_5min(t: NaiveTime) -> NaiveTime {
    let minute = (t.minute() / 5) * 5;
    NaiveTime::from_hms_opt(t.hour(), minute, 0).unwrap()
}

fn time_diff_secs(a: NaiveTime, b: NaiveTime) -> i64 {
    let a_secs = a.num_seconds_from_midnight() as i64;
    let b_secs = b.num_seconds_from_midnight() as i64;
    a_secs - b_secs
}

/// 5분 슬롯 시각을 UTC로 변환
fn slot_utc_time(market: &Market, now: DateTime<Utc>, slot_time: NaiveTime) -> DateTime<Utc> {
    let tz: Tz = market.timezone().parse().unwrap_or(chrono_tz::UTC);
    let local = now.with_timezone(&tz);
    let naive_dt = local.naive_local().date().and_time(slot_time);
    tz.from_local_datetime(&naive_dt)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(now)
}

fn idempotency_key(
    manager_id: Uuid,
    slot_id: Uuid,
    scheduled_for: DateTime<Utc>,
    run_type: &str,
) -> String {
    format!(
        "{manager_id}:{slot_id}:{run_type}:{}",
        scheduled_for.format("%Y%m%dT%H%M")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    #[test]
    fn align_to_5min_rounds_down() {
        let t = NaiveTime::from_hms_opt(9, 17, 45).unwrap();
        assert_eq!(align_to_5min(t), NaiveTime::from_hms_opt(9, 15, 0).unwrap());
    }

    #[test]
    fn align_to_5min_exact() {
        let t = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        assert_eq!(align_to_5min(t), NaiveTime::from_hms_opt(9, 30, 0).unwrap());
    }

    #[test]
    fn idempotency_key_is_stable() {
        let manager_id = Uuid::nil();
        let slot_id = Uuid::nil();
        let dt = Utc.with_ymd_and_hms(2024, 1, 2, 9, 30, 0).unwrap();
        let k1 = idempotency_key(manager_id, slot_id, dt, "scenario");
        let k2 = idempotency_key(manager_id, slot_id, dt, "scenario");
        assert_eq!(k1, k2);
    }

    #[test]
    fn idempotency_key_differs_by_type() {
        let manager_id = Uuid::nil();
        let slot_id = Uuid::nil();
        let dt = Utc.with_ymd_and_hms(2024, 1, 2, 9, 30, 0).unwrap();
        let k1 = idempotency_key(manager_id, slot_id, dt, "scenario");
        let k2 = idempotency_key(manager_id, slot_id, dt, "trade");
        assert_ne!(k1, k2);
    }

    #[test]
    fn krx_trading_hours() {
        let (open, close) = Market::Krx.trading_hours();
        assert_eq!(open, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(close, NaiveTime::from_hms_opt(15, 30, 0).unwrap());
    }

    #[test]
    fn us_trading_hours() {
        let (open, close) = Market::Us.trading_hours();
        assert_eq!(open, NaiveTime::from_hms_opt(9, 30, 0).unwrap());
        assert_eq!(close, NaiveTime::from_hms_opt(16, 0, 0).unwrap());
    }
}
