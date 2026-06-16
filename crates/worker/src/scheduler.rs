use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, NaiveTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use lumos_app::repo::manager::ManagerRepository;
use lumos_app::repo::scenario::{
    EvidenceCardRepository, ScenarioItemRepository, ScenarioOutcomeRepository,
};
use lumos_app::repo::schedule::{ManagerScheduleRepository, ScheduleRunRepository};
use lumos_app::repo::symbol::SymbolRepository;
use lumos_app::service::order_plan::OrderPlanService;
use lumos_app::service::scenario::ScenarioService;
use lumos_domain::model::scenario::{
    EvidenceCard, OutcomeResult, ScenarioAction, ScenarioOutcome,
};
use lumos_domain::model::symbol::Region;
use lumos_domain::model::schedule::{Market, ScheduleRunStatus};
use lumos_infra::kis::client::KisClient;
use lumos_infra::providers::naver_finance::NaverFinanceClient;
use lumos_infra::providers::naver_news::NaverNewsClient;
use lumos_infra::scenario::evidence_builder;
use lumos_domain::port::news::{NewsProvider, NewsQuery};

const TICK_SECS: u64 = 30;
/// 5분 슬롯 내에서 실행 허용 오차 (초)
const SLOT_WINDOW_SECS: i64 = 90;
/// KIS 투자자 수급 조회 기본 일수
const INVESTOR_FLOW_DAYS: u32 = 5;
/// 시나리오 생성 후 결과 평가까지의 대기 일수
const EVAL_DELAY_DAYS: i64 = 3;
/// 1회 평가 배치 최대 건수
const EVAL_BATCH_LIMIT: u32 = 200;
/// 프롬프트 피드백에 요약할 최근 outcome 건수
const OUTCOME_FEEDBACK_LIMIT: u32 = 10;

pub struct Scheduler {
    schedule_repo: Arc<dyn ManagerScheduleRepository>,
    run_repo: Arc<dyn ScheduleRunRepository>,
    scenario_svc: Arc<ScenarioService>,
    symbol_repo: Arc<dyn SymbolRepository>,
    manager_repo: Arc<dyn ManagerRepository>,
    scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    evidence_repo: Arc<dyn EvidenceCardRepository>,
    order_plan_svc: Option<Arc<OrderPlanService>>,
    kis_client: Option<Arc<KisClient>>,
    naver_client: Arc<NaverFinanceClient>,
    news_client: Option<Arc<NaverNewsClient>>,
    outcome_repo: Option<Arc<dyn ScenarioOutcomeRepository>>,
    last_evaluation_date: tokio::sync::Mutex<Option<chrono::NaiveDate>>,
}

impl Scheduler {
    pub fn new(
        schedule_repo: Arc<dyn ManagerScheduleRepository>,
        run_repo: Arc<dyn ScheduleRunRepository>,
        scenario_svc: Arc<ScenarioService>,
        symbol_repo: Arc<dyn SymbolRepository>,
        manager_repo: Arc<dyn ManagerRepository>,
        scenario_item_repo: Arc<dyn ScenarioItemRepository>,
        evidence_repo: Arc<dyn EvidenceCardRepository>,
    ) -> Self {
        Self {
            schedule_repo,
            run_repo,
            scenario_svc,
            symbol_repo,
            manager_repo,
            scenario_item_repo,
            evidence_repo,
            order_plan_svc: None,
            kis_client: None,
            naver_client: Arc::new(NaverFinanceClient::new()),
            news_client: None,
            outcome_repo: None,
            last_evaluation_date: tokio::sync::Mutex::new(None),
        }
    }

    pub fn with_outcome_repo(mut self, repo: Arc<dyn ScenarioOutcomeRepository>) -> Self {
        self.outcome_repo = Some(repo);
        self
    }

    pub fn with_kis_client(mut self, client: Arc<KisClient>) -> Self {
        self.kis_client = Some(client);
        self
    }

    pub fn with_order_plan_svc(mut self, svc: Arc<OrderPlanService>) -> Self {
        self.order_plan_svc = Some(svc);
        self
    }

    pub fn with_news_client(mut self, client: NaverNewsClient) -> Self {
        self.news_client = Some(Arc::new(client));
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
            // 하루 1회 시나리오 결과 평가 (자기진화).
            self.maybe_run_daily_evaluation(now).await;
        }
    }

    /// 날짜가 바뀌면 시나리오 결과 평가를 1회 실행한다.
    async fn maybe_run_daily_evaluation(&self, now: DateTime<Utc>) {
        let today = now.date_naive();
        {
            let mut last = self.last_evaluation_date.lock().await;
            if *last == Some(today) {
                return;
            }
            *last = Some(today);
        }
        if let Err(e) = self.evaluate_outcomes(now).await {
            tracing::error!("scenario outcome evaluation failed: {e:?}");
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
            for slot in slots.iter().filter(|s| s.enabled && s.time_of_day == aligned) {
                let scheduled_for = slot_utc_time(&sched.market, now, aligned);
                // 활성 슬롯은 시나리오 생성 → 매매를 하나의 사이클로 실행한다.
                self.maybe_run(sched.manager_id, slot.id, scheduled_for).await;
            }
        }
        Ok(())
    }

    async fn maybe_run(&self, manager_id: Uuid, slot_id: Uuid, scheduled_for: DateTime<Utc>) {
        let key = idempotency_key(manager_id, slot_id, scheduled_for);

        let run = match self
            .run_repo
            .create_if_not_exists(manager_id, slot_id, scheduled_for, &key)
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

        // 하나의 사이클: 시나리오 생성 → 매매 순차 실행
        let mut errors: Vec<String> = vec![];
        if let Err(e) = self.run_scenario_job(manager_id, slot_id).await {
            errors.push(format!("scenario: {e}"));
        }
        if let Err(e) = self.run_trade_job(manager_id).await {
            errors.push(format!("trade: {e}"));
        }

        let (status, msg) = if errors.is_empty() {
            (ScheduleRunStatus::Success, None)
        } else {
            (ScheduleRunStatus::Failed, Some(errors.join("; ")))
        };
        let _ = self.run_repo.update_status(run.id, status, msg).await;
    }

    /// 시나리오 생성 단계. 사이클 상태는 호출자(maybe_run)가 관리한다.
    async fn run_scenario_job(&self, manager_id: Uuid, slot_id: Uuid) -> Result<()> {
        tracing::info!("running scenario job for manager {manager_id}");

        // 매니저에 연결된 LLM 설정을 조회한다 (없으면 기본값으로 폴백).
        let (model_provider, model_name) = match self.manager_repo.find_by_id(manager_id).await? {
            Some(m) => (m.model_provider, m.model_name),
            None => {
                tracing::warn!("manager {manager_id} not found, using default model");
                ("openai".to_string(), "gpt-4o-mini".to_string())
            }
        };

        // active symbols를 모두 조회해서 각 심볼별로 시나리오 생성
        let symbols = self.symbol_repo.find_active().await?;

        if symbols.is_empty() {
            tracing::warn!("no active symbols, skipping scenario generation");
            return Ok(());
        }

        let mut any_error: Option<String> = None;
        for symbol in &symbols {
            // 수집한 evidence를 DB에 적재한다. 시나리오 생성은 DB에서 읽으므로
            // (run_for_symbol 내부 find_for_symbol) 여기서는 빈 벡터를 넘겨 중복을 피한다.
            let collected = self.collect_extra_evidence(&symbol.code, symbol.id).await;
            tracing::debug!(
                symbol = %symbol.code,
                count = collected.len(),
                "evidence persisted"
            );

            // 자기진화: 과거 평가 결과를 요약해 프롬프트 컨텍스트로 주입한다.
            // (매번 재계산되는 요약이므로 DB에 적재하지 않고 in-memory로 전달)
            let feedback = self.build_outcome_feedback(symbol.id).await;

            let result = self
                .scenario_svc
                .run_for_symbol(
                    manager_id,
                    symbol.id,
                    Some(slot_id),
                    model_provider.clone(),
                    model_name.clone(),
                    "v1".to_string(),
                    "0".to_string(),
                    feedback,
                    None,
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

        match any_error {
            Some(msg) => Err(anyhow::anyhow!(msg)),
            None => Ok(()),
        }
    }

    /// 매매 단계. 사이클 상태는 호출자(maybe_run)가 관리한다.
    async fn run_trade_job(&self, manager_id: Uuid) -> Result<()> {
        tracing::info!("running trade job for manager {manager_id}");

        let manager = match self.manager_repo.find_by_id(manager_id).await? {
            Some(m) => m,
            None => {
                tracing::warn!("manager {manager_id} not found, skipping trade job");
                return Ok(());
            }
        };

        let Some(order_plan_svc) = &self.order_plan_svc else {
            tracing::warn!("order_plan_svc not configured, skipping trade job for manager {manager_id}");
            return Ok(());
        };

        let items = self.scenario_item_repo.find_pending_for_manager(manager_id).await?;

        if items.is_empty() {
            tracing::info!("no pending scenario items for manager {manager_id}, skipping trade job");
            return Ok(());
        }

        let mut any_error: Option<String> = None;
        for item in &items {
            match order_plan_svc.create_from_scenario_item(manager_id, item.id).await {
                Ok(plan) => {
                    tracing::info!("order plan {} created (risk: {})", plan.id, plan.risk_status);
                    // 실주문은 매니저의 auto_trade_enabled + 시스템 전역 ENABLE_LIVE_TRADING 둘 다 true일 때만.
                    // 안전장치: 환경변수가 명시적으로 켜지지 않으면 주문을 내지 않는다.
                    if manager.auto_trade_enabled && live_trading_enabled() {
                        // 주문 실행 전에 symbol_repo로 symbol_code를 채운다.
                        // (symbol_code는 DB에 저장되지 않고 실행 시점에 해석된다)
                        let mut plan_with_ctx = plan;
                        match self.symbol_repo.find_by_id(plan_with_ctx.symbol_id).await {
                            Ok(Some(symbol)) => {
                                plan_with_ctx.symbol_code = Some(symbol.code);
                                if let Err(e) = order_plan_svc
                                    .execute_approved(&plan_with_ctx, manager.broker_connection_id)
                                    .await
                                {
                                    tracing::warn!(
                                        "execute_approved failed for plan {}: {e}",
                                        plan_with_ctx.id
                                    );
                                }
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    "symbol {} not found, cannot execute plan {}",
                                    plan_with_ctx.symbol_id,
                                    plan_with_ctx.id
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "failed to resolve symbol for plan {}: {e}",
                                    plan_with_ctx.id
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("create_from_scenario_item failed for item {}: {e}", item.id);
                    any_error = Some(e.to_string());
                }
            }
        }

        match any_error {
            Some(msg) => Err(anyhow::anyhow!(msg)),
            None => Ok(()),
        }
    }

    /// KIS 투자자 수급 + 네이버 컨센서스 + 뉴스를 best-effort로 수집해 EvidenceCard 반환
    async fn collect_extra_evidence(
        &self,
        symbol_code: &str,
        symbol_id: Uuid,
    ) -> Vec<EvidenceCard> {
        let mut cards = vec![];

        // KIS 투자자 수급 (공식, 신뢰도 높음)
        if let Some(kis) = &self.kis_client {
            match kis.domestic_investor_flow(symbol_code, INVESTOR_FLOW_DAYS).await {
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

        // 뉴스 수집 (NaverNewsClient가 주입된 경우)
        if let Some(news) = &self.news_client {
            let query = NewsQuery {
                keyword: symbol_code.to_string(),
                from: None,
                limit: 5,
            };
            match news.search_news(query).await {
                Ok(items) => {
                    for item in &items {
                        cards.push(evidence_builder::from_news(symbol_id, item));
                    }
                    tracing::debug!(
                        symbol = symbol_code,
                        count = items.len(),
                        "news evidence collected"
                    );
                }
                Err(e) => {
                    tracing::warn!("news collection skipped for {symbol_code}: {e}");
                }
            }
        }

        // 수집한 evidence를 DB에 적재한다. 저장된 카드(서버 생성 메타 포함)를 반환해
        // 시나리오 생성에 그대로 사용하고, 이후 조회/이력에도 남도록 한다.
        let mut persisted = Vec::with_capacity(cards.len());
        for card in cards {
            match self.evidence_repo.create(card.clone()).await {
                Ok(saved) => persisted.push(saved),
                Err(e) => {
                    // 저장 실패해도 시나리오 생성은 진행 (best-effort): 메모리 카드 사용
                    tracing::warn!("failed to persist evidence for {symbol_code}: {e}");
                    persisted.push(card);
                }
            }
        }
        persisted
    }

    /// 과거 시나리오 평가 결과를 요약한 EvidenceCard를 만든다 (자기진화 피드백).
    /// outcome_repo가 없거나 이력이 없으면 빈 벡터.
    async fn build_outcome_feedback(&self, symbol_id: Uuid) -> Vec<EvidenceCard> {
        let Some(outcome_repo) = &self.outcome_repo else {
            return vec![];
        };
        match outcome_repo.find_recent_for_symbol(symbol_id, OUTCOME_FEEDBACK_LIMIT).await {
            Ok(outcomes) => evidence_builder::from_scenario_outcomes(symbol_id, &outcomes)
                .into_iter()
                .collect(),
            Err(e) => {
                tracing::warn!("failed to load outcome feedback for {symbol_id}: {e}");
                vec![]
            }
        }
    }

    /// 시나리오 결과 평가 (자기진화). target/stop이 설정된 만료 시나리오를
    /// 현재가와 비교해 적중 여부를 scenario_outcomes에 기록한다.
    ///
    /// 한계: KIS historical 시세가 없어 "평가 시점 현재가" 기준으로 판정한다.
    /// 기간 중 일시 도달 후 되돌린 경우는 포착하지 못한다 (보수적 평가).
    async fn evaluate_outcomes(&self, now: DateTime<Utc>) -> Result<()> {
        let Some(outcome_repo) = &self.outcome_repo else {
            return Ok(());
        };
        let Some(kis) = &self.kis_client else {
            tracing::debug!("kis_client 미설정 — outcome 평가 스킵");
            return Ok(());
        };

        // 생성 후 EVAL_DELAY_DAYS 경과한 항목만 평가 대상.
        let cutoff = now - chrono::Duration::days(EVAL_DELAY_DAYS);
        let items = outcome_repo.find_unevaluated(cutoff, EVAL_BATCH_LIMIT).await?;
        if items.is_empty() {
            return Ok(());
        }
        tracing::info!("evaluating {} scenario outcomes", items.len());

        for ev in &items {
            let item = &ev.item;
            let symbol = match self.symbol_repo.find_by_id(item.symbol_id).await {
                Ok(Some(s)) => s,
                _ => continue,
            };

            let quote = match symbol.region {
                Region::Kr => kis.domestic_quote(&symbol.code).await,
                Region::Us => kis.overseas_quote(&symbol.code, "NAS").await,
            };
            let price = match quote {
                Ok(q) => q.last_price,
                Err(e) => {
                    tracing::warn!("quote failed for {}: {e}", symbol.code);
                    continue;
                }
            };

            let (target, stop) = match (item.target_price, item.stop_loss_price) {
                (Some(t), Some(s)) => (t, s),
                _ => continue,
            };
            // buy/sell 모두: target 방향 도달 우선, 그다음 stop.
            let result = match item.action {
                ScenarioAction::Buy => {
                    if price >= target {
                        OutcomeResult::TargetHit
                    } else if price <= stop {
                        OutcomeResult::StopHit
                    } else {
                        OutcomeResult::Expired
                    }
                }
                ScenarioAction::Sell => {
                    if price <= target {
                        OutcomeResult::TargetHit
                    } else if price >= stop {
                        OutcomeResult::StopHit
                    } else {
                        OutcomeResult::Expired
                    }
                }
                _ => OutcomeResult::Expired,
            };

            let outcome = ScenarioOutcome {
                id: Uuid::new_v4(),
                scenario_item_id: item.id,
                symbol_id: item.symbol_id,
                result,
                evaluated_price: price,
                base_price: None,
                return_pct: None,
                evaluated_at: now,
            };
            if let Err(e) = outcome_repo.create(outcome).await {
                tracing::warn!("failed to record outcome for item {}: {e}", item.id);
            }
        }
        Ok(())
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
    let naive_dt = local
        .naive_local()
        .date()
        .and_time(slot_time);
    tz.from_local_datetime(&naive_dt)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(now)
}

/// 실거래 주문 실행을 전역으로 켜는 안전 스위치.
/// ENABLE_LIVE_TRADING=true (또는 1)일 때만 실제 주문을 낸다.
fn live_trading_enabled() -> bool {
    matches!(
        std::env::var("ENABLE_LIVE_TRADING")
            .unwrap_or_default()
            .trim()
            .to_lowercase()
            .as_str(),
        "true" | "1" | "yes"
    )
}

fn idempotency_key(manager_id: Uuid, slot_id: Uuid, scheduled_for: DateTime<Utc>) -> String {
    format!(
        "{manager_id}:{slot_id}:{}",
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
        let k1 = idempotency_key(manager_id, slot_id, dt);
        let k2 = idempotency_key(manager_id, slot_id, dt);
        assert_eq!(k1, k2);
    }

    #[test]
    fn idempotency_key_differs_by_time() {
        let manager_id = Uuid::nil();
        let slot_id = Uuid::nil();
        let dt1 = Utc.with_ymd_and_hms(2024, 1, 2, 9, 30, 0).unwrap();
        let dt2 = Utc.with_ymd_and_hms(2024, 1, 2, 9, 35, 0).unwrap();
        let k1 = idempotency_key(manager_id, slot_id, dt1);
        let k2 = idempotency_key(manager_id, slot_id, dt2);
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
