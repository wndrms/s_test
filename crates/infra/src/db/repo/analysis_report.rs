use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use lumos_app::repo::analysis_report::{
    AnalysisReport, AnalysisReportRepository, ChartAnnotation, CreateAnalysisReportInput,
    CreateChartAnnotationInput,
};
use lumos_domain::model::scenario::{
    DataFreshnessLevel, EvidenceCard, EvidenceSourceType, SentimentLabel,
};

pub struct PgAnalysisReportRepository {
    pool: PgPool,
}

impl PgAnalysisReportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ReportRow {
    id: Uuid,
    manager_id: Uuid,
    symbol_id: Uuid,
    scenario_run_id: Uuid,
    base_price: Decimal,
    analyzed_at: DateTime<Utc>,
    report_text: String,
    report_summary: Option<String>,
    data_freshness_level: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<ReportRow> for AnalysisReport {
    fn from(r: ReportRow) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            symbol_id: r.symbol_id,
            scenario_run_id: r.scenario_run_id,
            base_price: r.base_price,
            analyzed_at: r.analyzed_at,
            report_text: r.report_text,
            report_summary: r.report_summary,
            data_freshness_level: r.data_freshness_level.and_then(|s| match s.as_str() {
                "fresh" => Some(DataFreshnessLevel::Fresh),
                "stale" => Some(DataFreshnessLevel::Stale),
                "blocking" => Some(DataFreshnessLevel::Blocking),
                _ => None,
            }),
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
struct EvidenceRow {
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

impl From<EvidenceRow> for EvidenceCard {
    fn from(r: EvidenceRow) -> Self {
        Self {
            id: r.id,
            symbol_id: r.symbol_id,
            source_type: match r.source_type.as_str() {
                "price" => EvidenceSourceType::Price,
                "technical" => EvidenceSourceType::Technical,
                "news" => EvidenceSourceType::News,
                "disclosure" => EvidenceSourceType::Disclosure,
                "financial" => EvidenceSourceType::Financial,
                _ => EvidenceSourceType::Community,
            },
            source_name: r.source_name,
            source_ref_table: r.source_ref_table,
            source_ref_id: r.source_ref_id,
            title: r.title,
            summary: r.summary,
            url: r.url,
            sentiment_label: r.sentiment_label.and_then(|s| match s.as_str() {
                "positive" => Some(SentimentLabel::Positive),
                "neutral" => Some(SentimentLabel::Neutral),
                "negative" => Some(SentimentLabel::Negative),
                _ => Some(SentimentLabel::Mixed),
            }),
            importance_score: r.importance_score,
            reliability_score: r.reliability_score,
            as_of: r.as_of,
            fetched_at: r.fetched_at,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
struct AnnotationRow {
    id: Uuid,
    analysis_report_id: Uuid,
    symbol_id: Uuid,
    annotation_type: String,
    price: Decimal,
    label: String,
    color_hint: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<AnnotationRow> for ChartAnnotation {
    fn from(r: AnnotationRow) -> Self {
        Self {
            id: r.id,
            analysis_report_id: r.analysis_report_id,
            symbol_id: r.symbol_id,
            annotation_type: r.annotation_type,
            price: r.price,
            label: r.label,
            color_hint: r.color_hint,
            created_at: r.created_at,
        }
    }
}

#[async_trait]
impl AnalysisReportRepository for PgAnalysisReportRepository {
    async fn create(&self, input: CreateAnalysisReportInput) -> Result<AnalysisReport> {
        let row: ReportRow = sqlx::query_as::<_, ReportRow>(
            r#"INSERT INTO analysis_reports
               (manager_id, symbol_id, scenario_run_id, base_price, analyzed_at,
                report_text, report_summary)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               RETURNING id, manager_id, symbol_id, scenario_run_id, base_price,
                         analyzed_at, report_text, report_summary, data_freshness_level, created_at"#,
        )
        .bind(input.manager_id)
        .bind(input.symbol_id)
        .bind(input.scenario_run_id)
        .bind(input.base_price)
        .bind(input.analyzed_at)
        .bind(&input.report_text)
        .bind(&input.report_summary)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn link_evidence(&self, report_id: Uuid, evidence_ids: &[Uuid]) -> Result<()> {
        for &eid in evidence_ids {
            sqlx::query(
                "INSERT INTO analysis_report_evidence (report_id, evidence_card_id)
                 VALUES ($1,$2) ON CONFLICT DO NOTHING",
            )
            .bind(report_id)
            .bind(eid)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn create_annotation(
        &self,
        input: CreateChartAnnotationInput,
    ) -> Result<ChartAnnotation> {
        let row: AnnotationRow = sqlx::query_as::<_, AnnotationRow>(
            r#"INSERT INTO chart_annotations
               (analysis_report_id, symbol_id, annotation_type, price, label, color_hint)
               VALUES ($1,$2,$3,$4,$5,$6)
               RETURNING id, analysis_report_id, symbol_id, annotation_type,
                         price, label, color_hint, created_at"#,
        )
        .bind(input.analysis_report_id)
        .bind(input.symbol_id)
        .bind(&input.annotation_type)
        .bind(input.price)
        .bind(&input.label)
        .bind(&input.color_hint)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into())
    }

    async fn update_scenario_item_report(&self, item_id: Uuid, report_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE scenario_items SET analysis_report_id = $1 WHERE id = $2")
            .bind(report_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<AnalysisReport>> {
        let row: Option<ReportRow> = sqlx::query_as::<_, ReportRow>(
            r#"SELECT id, manager_id, symbol_id, scenario_run_id, base_price,
                      analyzed_at, report_text, report_summary, data_freshness_level, created_at
               FROM analysis_reports WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn find_evidence(&self, report_id: Uuid) -> Result<Vec<EvidenceCard>> {
        let rows: Vec<EvidenceRow> = sqlx::query_as::<_, EvidenceRow>(
            r#"SELECT ec.id, ec.symbol_id, ec.source_type, ec.source_name,
                      ec.source_ref_table, ec.source_ref_id, ec.title, ec.summary,
                      ec.url, ec.sentiment_label, ec.importance_score, ec.reliability_score,
                      ec.as_of, ec.fetched_at, ec.created_at
               FROM evidence_cards ec
               JOIN analysis_report_evidence are ON are.evidence_card_id = ec.id
               WHERE are.report_id = $1
               ORDER BY ec.importance_score DESC"#,
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_annotations(&self, report_id: Uuid) -> Result<Vec<ChartAnnotation>> {
        let rows: Vec<AnnotationRow> = sqlx::query_as::<_, AnnotationRow>(
            r#"SELECT id, analysis_report_id, symbol_id, annotation_type,
                      price, label, color_hint, created_at
               FROM chart_annotations WHERE analysis_report_id = $1
               ORDER BY price DESC"#,
        )
        .bind(report_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
