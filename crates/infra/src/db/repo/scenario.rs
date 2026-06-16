use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::scenario::{
    CreateScenarioItemInput, CreateScenarioRunInput, EvaluableItem, EvidenceCardRepository,
    ScenarioItemRepository, ScenarioOutcomeRepository, ScenarioRunRepository,
};
use lumos_domain::model::scenario::{
    EvidenceCard, EvidenceSourceType, OutcomeResult, ScenarioAction, ScenarioItem, ScenarioOutcome,
    ScenarioRun, ScenarioStatus, ScenarioType, SentimentLabel,
};

// ─── Evidence Card ───────────────────────────────────────────────────────────

pub struct PgEvidenceCardRepository {
    pool: PgPool,
}

impl PgEvidenceCardRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct EvidenceCardRow {
    id: Uuid,
    symbol_id: Uuid,
    source_type: String,
    source_name: String,
    source_ref_table: Option<String>,
    source_ref_id: Option<Uuid>,
    title: String,
    summary: String,
    url: Option<String>,
    sentiment_label: Option<String>,
    importance_score: Decimal,
    reliability_score: Decimal,
    as_of: DateTime<Utc>,
    fetched_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<EvidenceCardRow> for EvidenceCard {
    fn from(r: EvidenceCardRow) -> Self {
        Self {
            id: r.id,
            symbol_id: r.symbol_id,
            source_type: parse_source_type(&r.source_type),
            source_name: r.source_name,
            source_ref_table: r.source_ref_table,
            source_ref_id: r.source_ref_id,
            title: r.title,
            summary: r.summary,
            url: r.url,
            sentiment_label: r.sentiment_label.as_deref().and_then(parse_sentiment),
            importance_score: r.importance_score,
            reliability_score: r.reliability_score,
            as_of: r.as_of,
            fetched_at: r.fetched_at,
            created_at: r.created_at,
        }
    }
}

fn parse_source_type(s: &str) -> EvidenceSourceType {
    match s {
        "price" => EvidenceSourceType::Price,
        "technical" => EvidenceSourceType::Technical,
        "news" => EvidenceSourceType::News,
        "disclosure" => EvidenceSourceType::Disclosure,
        "financial" => EvidenceSourceType::Financial,
        _ => EvidenceSourceType::Community,
    }
}

fn source_type_str(t: &EvidenceSourceType) -> &'static str {
    match t {
        EvidenceSourceType::Price => "price",
        EvidenceSourceType::Technical => "technical",
        EvidenceSourceType::News => "news",
        EvidenceSourceType::Disclosure => "disclosure",
        EvidenceSourceType::Financial => "financial",
        EvidenceSourceType::Community => "community",
    }
}

fn parse_sentiment(s: &str) -> Option<SentimentLabel> {
    match s {
        "positive" => Some(SentimentLabel::Positive),
        "neutral" => Some(SentimentLabel::Neutral),
        "negative" => Some(SentimentLabel::Negative),
        "mixed" => Some(SentimentLabel::Mixed),
        _ => None,
    }
}

fn sentiment_str(l: &SentimentLabel) -> &'static str {
    match l {
        SentimentLabel::Positive => "positive",
        SentimentLabel::Neutral => "neutral",
        SentimentLabel::Negative => "negative",
        SentimentLabel::Mixed => "mixed",
    }
}

#[async_trait]
impl EvidenceCardRepository for PgEvidenceCardRepository {
    async fn find_for_symbol(
        &self,
        symbol_id: Uuid,
        source_types: &[EvidenceSourceType],
        limit_per_type: u32,
    ) -> Result<Vec<EvidenceCard>> {
        let type_strs: Vec<&str> = source_types.iter().map(source_type_str).collect();
        let rows: Vec<EvidenceCardRow> = sqlx::query_as::<_, EvidenceCardRow>(
            r#"SELECT id, symbol_id, source_type, source_name, source_ref_table, source_ref_id,
                      title, summary, url, sentiment_label, importance_score, reliability_score,
                      as_of, fetched_at, created_at
               FROM evidence_cards
               WHERE symbol_id = $1 AND source_type = ANY($2)
               ORDER BY importance_score DESC, as_of DESC
               LIMIT $3"#,
        )
        .bind(symbol_id)
        .bind(&type_strs[..])
        .bind(limit_per_type as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create(&self, card: EvidenceCard) -> Result<EvidenceCard> {
        let row: EvidenceCardRow = sqlx::query_as::<_, EvidenceCardRow>(
            r#"INSERT INTO evidence_cards
               (id, symbol_id, source_type, source_name, source_ref_table, source_ref_id,
                title, summary, url, sentiment_label, importance_score, reliability_score,
                as_of, fetched_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
               RETURNING id, symbol_id, source_type, source_name, source_ref_table, source_ref_id,
                         title, summary, url, sentiment_label, importance_score, reliability_score,
                         as_of, fetched_at, created_at"#,
        )
        .bind(card.id)
        .bind(card.symbol_id)
        .bind(source_type_str(&card.source_type))
        .bind(&card.source_name)
        .bind(&card.source_ref_table)
        .bind(card.source_ref_id)
        .bind(&card.title)
        .bind(&card.summary)
        .bind(&card.url)
        .bind(card.sentiment_label.as_ref().map(sentiment_str))
        .bind(card.importance_score)
        .bind(card.reliability_score)
        .bind(card.as_of)
        .bind(card.fetched_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }
}

// ─── Scenario Run ─────────────────────────────────────────────────────────────

pub struct PgScenarioRunRepository {
    pool: PgPool,
}

impl PgScenarioRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ScenarioRunRow {
    id: Uuid,
    manager_id: Uuid,
    schedule_slot_id: Option<Uuid>,
    model_provider: String,
    model_name: String,
    prompt_version: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
}

impl From<ScenarioRunRow> for ScenarioRun {
    fn from(r: ScenarioRunRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            schedule_slot_id: r.schedule_slot_id,
            model_provider: r.model_provider,
            model_name: r.model_name,
            prompt_version: r.prompt_version,
            status: parse_run_status(&r.status),
            created_at: r.created_at,
        }
    }
}

fn parse_run_status(s: &str) -> ScenarioStatus {
    match s {
        "validated" => ScenarioStatus::Validated,
        "rejected" => ScenarioStatus::Rejected,
        "executed" => ScenarioStatus::Executed,
        "failed" => ScenarioStatus::Failed,
        _ => ScenarioStatus::Generated,
    }
}

fn run_status_str(s: &ScenarioStatus) -> &'static str {
    match s {
        ScenarioStatus::Generated => "generated",
        ScenarioStatus::Validated => "validated",
        ScenarioStatus::Rejected => "rejected",
        ScenarioStatus::Executed => "executed",
        ScenarioStatus::Failed => "failed",
    }
}

#[async_trait]
impl ScenarioRunRepository for PgScenarioRunRepository {
    async fn create(&self, input: CreateScenarioRunInput) -> Result<ScenarioRun> {
        let row: ScenarioRunRow = sqlx::query_as::<_, ScenarioRunRow>(
            r#"INSERT INTO scenario_runs
               (manager_id, schedule_slot_id, model_provider, model_name, prompt_version)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, manager_id, schedule_slot_id, model_provider, model_name,
                         prompt_version, status, created_at"#,
        )
        .bind(input.manager_id)
        .bind(input.schedule_slot_id)
        .bind(&input.model_provider)
        .bind(&input.model_name)
        .bind(&input.prompt_version)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn update_status(&self, id: Uuid, status: ScenarioStatus) -> Result<ScenarioRun> {
        let row: ScenarioRunRow = sqlx::query_as::<_, ScenarioRunRow>(
            r#"UPDATE scenario_runs SET status = $2
               WHERE id = $1
               RETURNING id, manager_id, schedule_slot_id, model_provider, model_name,
                         prompt_version, status, created_at"#,
        )
        .bind(id)
        .bind(run_status_str(&status))
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn find_latest_for_manager(
        &self,
        manager_id: Uuid,
        limit: u32,
    ) -> Result<Vec<ScenarioRun>> {
        let rows: Vec<ScenarioRunRow> = sqlx::query_as::<_, ScenarioRunRow>(
            r#"SELECT id, manager_id, schedule_slot_id, model_provider, model_name,
                      prompt_version, status, created_at
               FROM scenario_runs
               WHERE manager_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(manager_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScenarioRun>> {
        let row: Option<ScenarioRunRow> = sqlx::query_as::<_, ScenarioRunRow>(
            r#"SELECT id, manager_id, schedule_slot_id, model_provider, model_name,
                      prompt_version, status, created_at
               FROM scenario_runs WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }
}

// ─── Scenario Item ────────────────────────────────────────────────────────────

pub struct PgScenarioItemRepository {
    pool: PgPool,
}

impl PgScenarioItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ScenarioItemRow {
    id: Uuid,
    scenario_run_id: Uuid,
    analysis_report_id: Option<Uuid>,
    symbol_id: Uuid,
    scenario_type: String,
    action: String,
    probability_pct: Decimal,
    target_price: Option<Decimal>,
    stop_loss_price: Option<Decimal>,
    condition_text: String,
    strategy_text: String,
    risk_text: Option<String>,
    rank_order: i32,
}

impl From<ScenarioItemRow> for ScenarioItem {
    fn from(r: ScenarioItemRow) -> Self {
        Self {
            id: r.id,
            scenario_run_id: r.scenario_run_id,
            analysis_report_id: r.analysis_report_id,
            symbol_id: r.symbol_id,
            scenario_type: parse_scenario_type(&r.scenario_type),
            action: parse_scenario_action(&r.action),
            probability_pct: r.probability_pct,
            target_price: r.target_price,
            stop_loss_price: r.stop_loss_price,
            condition_text: r.condition_text,
            strategy_text: r.strategy_text,
            risk_text: r.risk_text,
            rank_order: r.rank_order,
        }
    }
}

fn parse_scenario_type(s: &str) -> ScenarioType {
    match s {
        "bullish" => ScenarioType::Bullish,
        "bearish" => ScenarioType::Bearish,
        _ => ScenarioType::Sideways,
    }
}

fn scenario_type_str(t: &ScenarioType) -> &'static str {
    match t {
        ScenarioType::Bullish => "bullish",
        ScenarioType::Sideways => "sideways",
        ScenarioType::Bearish => "bearish",
    }
}

fn parse_scenario_action(s: &str) -> ScenarioAction {
    match s {
        "buy" => ScenarioAction::Buy,
        "sell" => ScenarioAction::Sell,
        "watch" => ScenarioAction::Watch,
        _ => ScenarioAction::Hold,
    }
}

fn scenario_action_str(a: &ScenarioAction) -> &'static str {
    match a {
        ScenarioAction::Buy => "buy",
        ScenarioAction::Sell => "sell",
        ScenarioAction::Hold => "hold",
        ScenarioAction::Watch => "watch",
    }
}

#[async_trait]
impl ScenarioItemRepository for PgScenarioItemRepository {
    async fn create_batch(
        &self,
        items: Vec<CreateScenarioItemInput>,
    ) -> Result<Vec<ScenarioItem>> {
        let mut result = Vec::with_capacity(items.len());
        for input in items {
            let item = &input.item;
            let row: ScenarioItemRow = sqlx::query_as::<_, ScenarioItemRow>(
                r#"INSERT INTO scenario_items
                   (scenario_run_id, symbol_id, scenario_type, action, probability_pct,
                    target_price, stop_loss_price, condition_text, strategy_text, risk_text,
                    rank_order)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                   RETURNING id, scenario_run_id, analysis_report_id, symbol_id, scenario_type,
                             action, probability_pct, target_price, stop_loss_price,
                             condition_text, strategy_text, risk_text, rank_order"#,
            )
            .bind(input.scenario_run_id)
            .bind(input.symbol_id)
            .bind(scenario_type_str(&item.scenario_type))
            .bind(scenario_action_str(&item.action))
            .bind(item.probability_pct)
            .bind(item.target_price)
            .bind(item.stop_loss_price)
            .bind(&item.condition_text)
            .bind(&item.strategy_text)
            .bind(&item.risk_text)
            .bind(item.rank_order)
            .fetch_one(&self.pool)
            .await?;
            result.push(row.into());
        }
        Ok(result)
    }

    async fn find_by_run(&self, run_id: Uuid) -> Result<Vec<ScenarioItem>> {
        let rows: Vec<ScenarioItemRow> = sqlx::query_as::<_, ScenarioItemRow>(
            r#"SELECT id, scenario_run_id, analysis_report_id, symbol_id, scenario_type,
                      action, probability_pct, target_price, stop_loss_price,
                      condition_text, strategy_text, risk_text, rank_order
               FROM scenario_items
               WHERE scenario_run_id = $1
               ORDER BY rank_order"#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_run_and_id(&self, item_id: Uuid) -> Result<Option<ScenarioItem>> {
        let row: Option<ScenarioItemRow> = sqlx::query_as::<_, ScenarioItemRow>(
            r#"SELECT id, scenario_run_id, analysis_report_id, symbol_id, scenario_type,
                      action, probability_pct, target_price, stop_loss_price,
                      condition_text, strategy_text, risk_text, rank_order
               FROM scenario_items WHERE id = $1"#,
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_pending_for_manager(&self, manager_id: Uuid) -> Result<Vec<ScenarioItem>> {
        let rows: Vec<ScenarioItemRow> = sqlx::query_as::<_, ScenarioItemRow>(
            r#"SELECT si.id, si.scenario_run_id, si.analysis_report_id, si.symbol_id,
                      si.scenario_type, si.action, si.probability_pct, si.target_price,
                      si.stop_loss_price, si.condition_text, si.strategy_text, si.risk_text,
                      si.rank_order
               FROM scenario_items si
               INNER JOIN scenario_runs sr ON sr.id = si.scenario_run_id
               WHERE sr.manager_id = $1
                 AND sr.status IN ('validated', 'generated')
                 AND si.action IN ('buy', 'sell')
               ORDER BY sr.created_at DESC, si.rank_order
               LIMIT 20"#,
        )
        .bind(manager_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ─── Scenario Outcome (자기진화) ───────────────────────────────────────────────

pub struct PgScenarioOutcomeRepository {
    pool: PgPool,
}

impl PgScenarioOutcomeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn parse_outcome_result(s: &str) -> OutcomeResult {
    match s {
        "target_hit" => OutcomeResult::TargetHit,
        "stop_hit" => OutcomeResult::StopHit,
        _ => OutcomeResult::Expired,
    }
}

#[derive(FromRow)]
struct EvaluableItemRow {
    id: Uuid,
    scenario_run_id: Uuid,
    analysis_report_id: Option<Uuid>,
    symbol_id: Uuid,
    scenario_type: String,
    action: String,
    probability_pct: Decimal,
    target_price: Option<Decimal>,
    stop_loss_price: Option<Decimal>,
    condition_text: String,
    strategy_text: String,
    risk_text: Option<String>,
    rank_order: i32,
    run_created_at: DateTime<Utc>,
}

impl From<EvaluableItemRow> for EvaluableItem {
    fn from(r: EvaluableItemRow) -> Self {
        let run_created_at = r.run_created_at;
        EvaluableItem {
            item: ScenarioItem {
                id: r.id,
                scenario_run_id: r.scenario_run_id,
                analysis_report_id: r.analysis_report_id,
                symbol_id: r.symbol_id,
                scenario_type: parse_scenario_type(&r.scenario_type),
                action: parse_scenario_action(&r.action),
                probability_pct: r.probability_pct,
                target_price: r.target_price,
                stop_loss_price: r.stop_loss_price,
                condition_text: r.condition_text,
                strategy_text: r.strategy_text,
                risk_text: r.risk_text,
                rank_order: r.rank_order,
            },
            run_created_at,
        }
    }
}

#[derive(FromRow)]
struct ScenarioOutcomeRow {
    id: Uuid,
    scenario_item_id: Uuid,
    symbol_id: Uuid,
    result: String,
    evaluated_price: Decimal,
    base_price: Option<Decimal>,
    return_pct: Option<Decimal>,
    evaluated_at: DateTime<Utc>,
}

impl From<ScenarioOutcomeRow> for ScenarioOutcome {
    fn from(r: ScenarioOutcomeRow) -> Self {
        Self {
            id: r.id,
            scenario_item_id: r.scenario_item_id,
            symbol_id: r.symbol_id,
            result: parse_outcome_result(&r.result),
            evaluated_price: r.evaluated_price,
            base_price: r.base_price,
            return_pct: r.return_pct,
            evaluated_at: r.evaluated_at,
        }
    }
}

#[async_trait]
impl ScenarioOutcomeRepository for PgScenarioOutcomeRepository {
    async fn find_unevaluated(
        &self,
        created_before: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<EvaluableItem>> {
        // 아직 outcome이 없고, target/stop이 있으며, Buy/Sell 액션인 항목.
        let rows: Vec<EvaluableItemRow> = sqlx::query_as::<_, EvaluableItemRow>(
            r#"SELECT si.id, si.scenario_run_id, si.analysis_report_id, si.symbol_id,
                      si.scenario_type, si.action, si.probability_pct, si.target_price,
                      si.stop_loss_price, si.condition_text, si.strategy_text, si.risk_text,
                      si.rank_order, sr.created_at AS run_created_at
               FROM scenario_items si
               JOIN scenario_runs sr ON sr.id = si.scenario_run_id
               LEFT JOIN scenario_outcomes so ON so.scenario_item_id = si.id
               WHERE so.id IS NULL
                 AND sr.created_at < $1
                 AND si.action IN ('buy','sell')
                 AND si.target_price IS NOT NULL
                 AND si.stop_loss_price IS NOT NULL
               ORDER BY sr.created_at
               LIMIT $2"#,
        )
        .bind(created_before)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn create(&self, outcome: ScenarioOutcome) -> Result<ScenarioOutcome> {
        let row: ScenarioOutcomeRow = sqlx::query_as::<_, ScenarioOutcomeRow>(
            r#"INSERT INTO scenario_outcomes
               (id, scenario_item_id, symbol_id, result, evaluated_price, base_price,
                return_pct, evaluated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               ON CONFLICT (scenario_item_id) DO NOTHING
               RETURNING id, scenario_item_id, symbol_id, result, evaluated_price,
                         base_price, return_pct, evaluated_at"#,
        )
        .bind(outcome.id)
        .bind(outcome.scenario_item_id)
        .bind(outcome.symbol_id)
        .bind(outcome.result.as_str())
        .bind(outcome.evaluated_price)
        .bind(outcome.base_price)
        .bind(outcome.return_pct)
        .bind(outcome.evaluated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn find_recent_for_symbol(
        &self,
        symbol_id: Uuid,
        limit: u32,
    ) -> Result<Vec<ScenarioOutcome>> {
        let rows: Vec<ScenarioOutcomeRow> = sqlx::query_as::<_, ScenarioOutcomeRow>(
            r#"SELECT id, scenario_item_id, symbol_id, result, evaluated_price,
                      base_price, return_pct, evaluated_at
               FROM scenario_outcomes
               WHERE symbol_id = $1
               ORDER BY evaluated_at DESC
               LIMIT $2"#,
        )
        .bind(symbol_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
