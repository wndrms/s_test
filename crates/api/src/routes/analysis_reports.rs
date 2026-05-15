use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use lumos_app::error::AppError;
use lumos_app::repo::analysis_report::{AnalysisReport, AnalysisReportRepository, ChartAnnotation};
use lumos_domain::model::scenario::EvidenceCard;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/:report_id", get(get_report))
}

#[derive(Debug, Serialize)]
pub struct EvidenceCardResponse {
    pub id: Uuid,
    pub source_type: String,
    pub source_name: String,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub sentiment_label: Option<String>,
    pub importance_score: Decimal,
    pub reliability_score: Decimal,
    pub as_of: DateTime<Utc>,
}

impl From<EvidenceCard> for EvidenceCardResponse {
    fn from(e: EvidenceCard) -> Self {
        Self {
            id: e.id,
            source_type: format!("{:?}", e.source_type).to_lowercase(),
            source_name: e.source_name,
            title: e.title,
            summary: e.summary,
            url: e.url,
            sentiment_label: e.sentiment_label.map(|s| format!("{:?}", s).to_lowercase()),
            importance_score: e.importance_score,
            reliability_score: e.reliability_score,
            as_of: e.as_of,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChartAnnotationResponse {
    pub id: Uuid,
    pub annotation_type: String,
    pub price: Decimal,
    pub label: String,
    pub color_hint: Option<String>,
}

impl From<ChartAnnotation> for ChartAnnotationResponse {
    fn from(a: ChartAnnotation) -> Self {
        Self {
            id: a.id,
            annotation_type: a.annotation_type,
            price: a.price,
            label: a.label,
            color_hint: a.color_hint,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AnalysisReportResponse {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub scenario_run_id: Uuid,
    pub base_price: Decimal,
    pub analyzed_at: DateTime<Utc>,
    pub report_text: String,
    pub report_summary: Option<String>,
    pub data_freshness_level: Option<String>,
    pub evidence: Vec<EvidenceCardResponse>,
    pub annotations: Vec<ChartAnnotationResponse>,
}

impl AnalysisReportResponse {
    fn from_parts(
        r: AnalysisReport,
        evidence: Vec<EvidenceCard>,
        annotations: Vec<ChartAnnotation>,
    ) -> Self {
        Self {
            id: r.id,
            manager_id: r.manager_id,
            symbol_id: r.symbol_id,
            scenario_run_id: r.scenario_run_id,
            base_price: r.base_price,
            analyzed_at: r.analyzed_at,
            report_text: r.report_text,
            report_summary: r.report_summary,
            data_freshness_level: r
                .data_freshness_level
                .map(|l| format!("{:?}", l).to_lowercase()),
            evidence: evidence.into_iter().map(Into::into).collect(),
            annotations: annotations.into_iter().map(Into::into).collect(),
        }
    }
}

async fn get_report(
    State(state): State<AppState>,
    Path((manager_id, report_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<AnalysisReportResponse>> {
    let report = state
        .analysis_report_repo
        .find_by_id(report_id)
        .await
        .map_err(|e| ApiError::from(AppError::Internal(e)))?
        .ok_or_else(|| ApiError::from(AppError::NotFound("analysis_report".to_string())))?;

    if report.manager_id != manager_id {
        return Err(ApiError::from(AppError::NotFound(
            "analysis_report".to_string(),
        )));
    }

    let (evidence, annotations) = tokio::join!(
        state.analysis_report_repo.find_evidence(report_id),
        state.analysis_report_repo.find_annotations(report_id),
    );

    Ok(Json(AnalysisReportResponse::from_parts(
        report,
        evidence.map_err(|e| ApiError::from(AppError::Internal(e)))?,
        annotations.map_err(|e| ApiError::from(AppError::Internal(e)))?,
    )))
}
