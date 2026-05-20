use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use lumos_domain::model::scenario::{DataFreshnessLevel, EvidenceCard};

#[derive(Debug, Clone)]
pub struct CreateAnalysisReportInput {
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub scenario_run_id: Uuid,
    pub base_price: Decimal,
    pub analyzed_at: DateTime<Utc>,
    pub report_text: String,
    pub report_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateChartAnnotationInput {
    pub analysis_report_id: Uuid,
    pub symbol_id: Uuid,
    pub annotation_type: String,
    pub price: Decimal,
    pub label: String,
    pub color_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub scenario_run_id: Uuid,
    pub base_price: Decimal,
    pub analyzed_at: DateTime<Utc>,
    pub report_text: String,
    pub report_summary: Option<String>,
    pub data_freshness_level: Option<DataFreshnessLevel>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ChartAnnotation {
    pub id: Uuid,
    pub analysis_report_id: Uuid,
    pub symbol_id: Uuid,
    pub annotation_type: String,
    pub price: Decimal,
    pub label: String,
    pub color_hint: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait AnalysisReportRepository: Send + Sync {
    async fn create(&self, input: CreateAnalysisReportInput) -> Result<AnalysisReport>;
    async fn link_evidence(&self, report_id: Uuid, evidence_ids: &[Uuid]) -> Result<()>;
    async fn create_annotation(&self, input: CreateChartAnnotationInput) -> Result<ChartAnnotation>;
    async fn update_scenario_item_report(&self, item_id: Uuid, report_id: Uuid) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<AnalysisReport>>;
    async fn find_evidence(&self, report_id: Uuid) -> Result<Vec<EvidenceCard>>;
    async fn find_annotations(&self, report_id: Uuid) -> Result<Vec<ChartAnnotation>>;
}
