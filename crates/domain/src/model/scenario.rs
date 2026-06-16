use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::broker::OrderSide;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioType {
    Bullish,
    Sideways,
    Bearish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioAction {
    Buy,
    Sell,
    Hold,
    Watch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioStatus {
    Generated,
    Validated,
    Rejected,
    Executed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataFreshnessLevel {
    Fresh,
    Stale,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SentimentLabel {
    Positive,
    Neutral,
    Negative,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceSourceType {
    Price,
    Technical,
    News,
    Disclosure,
    Financial,
    Community,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCard {
    pub id: Uuid,
    pub symbol_id: Uuid,
    pub source_type: EvidenceSourceType,
    pub source_name: String,
    pub source_ref_table: Option<String>,
    pub source_ref_id: Option<Uuid>,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub sentiment_label: Option<SentimentLabel>,
    pub importance_score: Decimal,
    pub reliability_score: Decimal,
    pub as_of: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRun {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub schedule_slot_id: Option<Uuid>,
    pub model_provider: String,
    pub model_name: String,
    pub prompt_version: Option<String>,
    pub status: ScenarioStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioItem {
    pub id: Uuid,
    pub scenario_run_id: Uuid,
    pub analysis_report_id: Option<Uuid>,
    pub symbol_id: Uuid,
    pub scenario_type: ScenarioType,
    pub action: ScenarioAction,
    pub probability_pct: Decimal,
    pub target_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,
    pub condition_text: String,
    pub strategy_text: String,
    pub risk_text: Option<String>,
    pub rank_order: i32,
}

/// 시나리오 사후 평가 결과 (자기진화용).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeResult {
    TargetHit,
    StopHit,
    Expired,
}

impl OutcomeResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutcomeResult::TargetHit => "target_hit",
            OutcomeResult::StopHit => "stop_hit",
            OutcomeResult::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOutcome {
    pub id: Uuid,
    pub scenario_item_id: Uuid,
    pub symbol_id: Uuid,
    pub result: OutcomeResult,
    pub evaluated_price: Decimal,
    pub base_price: Option<Decimal>,
    pub return_pct: Option<Decimal>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderIntent {
    pub side: OrderSide,
    pub limit_price: Decimal,
    pub max_position_pct_hint: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedAction {
    pub action: ScenarioAction,
    pub reason: String,
    pub confidence_pct: Decimal,
    pub order_intent: Option<OrderIntent>,
}

// ── 시나리오 출력 검증 ────────────────────────────────────────────────────────
//
// contracts/scenario_output.schema.json 계약을 도메인 모델 기준으로 강제한다.
// LLM provider(mock/실제 모두)가 반환한 ScenarioOutput을 저장하기 전에 통과해야 한다.

/// 시나리오 출력이 계약을 위반했을 때의 사유. 결정적이며 로깅에 사용된다.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScenarioValidationError {
    #[error("시나리오는 정확히 3개여야 합니다 (현재 {0}개)")]
    WrongScenarioCount(usize),
    #[error("bullish/sideways/bearish 세 가지 시나리오 타입이 모두 있어야 합니다")]
    MissingScenarioType,
    #[error("scenario[{index}].probability_pct({value})가 0~100 범위를 벗어났습니다")]
    ProbabilityOutOfRange { index: usize, value: Decimal },
    #[error("시나리오 확률 합계가 100에서 벗어났습니다 (합계 {sum})")]
    ProbabilitySumInvalid { sum: Decimal },
    #[error("scenario[{index}].{field}가 비어 있습니다")]
    EmptyScenarioField { index: usize, field: &'static str },
    #[error("scenario[{index}].{field} 길이 초과 (최대 {max}, 현재 {len})")]
    ScenarioFieldTooLong {
        index: usize,
        field: &'static str,
        max: usize,
        len: usize,
    },
    #[error("recommended_action.reason이 비어 있습니다")]
    EmptyRecommendedReason,
    #[error("recommended_action.confidence_pct({0})가 0~100 범위를 벗어났습니다")]
    ConfidenceOutOfRange(Decimal),
}

const PROB_SUM_TOLERANCE: &str = "0.5";

/// 도메인 ScenarioOutput을 계약 규칙에 따라 검증한다.
/// 통과하면 `Ok(())`, 위반하면 첫 위반 사유를 반환한다.
pub fn validate_scenario_output(
    output: &crate::port::llm::ScenarioOutput,
) -> Result<(), ScenarioValidationError> {
    use std::str::FromStr;

    // 1. 시나리오 개수 정확히 3개
    let n = output.scenarios.len();
    if n != 3 {
        return Err(ScenarioValidationError::WrongScenarioCount(n));
    }

    // 2. bullish/sideways/bearish 모두 존재
    let has = |t: ScenarioType| output.scenarios.iter().any(|s| s.scenario_type == t);
    if !(has(ScenarioType::Bullish) && has(ScenarioType::Sideways) && has(ScenarioType::Bearish)) {
        return Err(ScenarioValidationError::MissingScenarioType);
    }

    // 3. 각 시나리오 필드 검증
    let mut prob_sum = Decimal::ZERO;
    for (i, s) in output.scenarios.iter().enumerate() {
        if s.probability_pct < Decimal::ZERO || s.probability_pct > Decimal::from(100) {
            return Err(ScenarioValidationError::ProbabilityOutOfRange {
                index: i,
                value: s.probability_pct,
            });
        }
        prob_sum += s.probability_pct;

        check_field(i, "condition_text", &s.condition_text, 1600)?;
        check_field(i, "strategy_text", &s.strategy_text, 1200)?;
        if let Some(risk) = &s.risk_text {
            let len = risk.as_str().chars().count();
            if len > 1200 {
                return Err(ScenarioValidationError::ScenarioFieldTooLong {
                    index: i,
                    field: "risk_text",
                    max: 1200,
                    len,
                });
            }
        }
    }

    // 4. 확률 합계 ~100 (라운딩 허용)
    let tolerance = Decimal::from_str(PROB_SUM_TOLERANCE).unwrap_or(Decimal::ZERO);
    if (prob_sum - Decimal::from(100)).abs() > tolerance {
        return Err(ScenarioValidationError::ProbabilitySumInvalid { sum: prob_sum });
    }

    // 5. recommended_action
    if output.recommended_action.reason.trim().is_empty() {
        return Err(ScenarioValidationError::EmptyRecommendedReason);
    }
    let conf = output.recommended_action.confidence_pct;
    if conf < Decimal::ZERO || conf > Decimal::from(100) {
        return Err(ScenarioValidationError::ConfidenceOutOfRange(conf));
    }

    Ok(())
}

fn check_field(
    index: usize,
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), ScenarioValidationError> {
    if value.trim().is_empty() {
        return Err(ScenarioValidationError::EmptyScenarioField { index, field });
    }
    let len = value.chars().count();
    if len > max {
        return Err(ScenarioValidationError::ScenarioFieldTooLong {
            index,
            field,
            max,
            len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::port::llm::ScenarioOutput;
    use rust_decimal_macros::dec;

    fn item(t: ScenarioType, action: ScenarioAction, prob: Decimal, rank: i32) -> ScenarioItem {
        ScenarioItem {
            id: Uuid::new_v4(),
            scenario_run_id: Uuid::nil(),
            analysis_report_id: None,
            symbol_id: Uuid::nil(),
            scenario_type: t,
            action,
            probability_pct: prob,
            target_price: None,
            stop_loss_price: None,
            condition_text: "조건".to_string(),
            strategy_text: "전략".to_string(),
            risk_text: None,
            rank_order: rank,
        }
    }

    fn valid_output() -> ScenarioOutput {
        ScenarioOutput {
            symbol: "005930".to_string(),
            base_price: "75000".to_string(),
            analyzed_at: Utc::now(),
            analysis_summary: "요약".to_string(),
            analysis_detail: None,
            scenarios: vec![
                item(ScenarioType::Bullish, ScenarioAction::Buy, dec!(45), 1),
                item(ScenarioType::Sideways, ScenarioAction::Hold, dec!(35), 2),
                item(ScenarioType::Bearish, ScenarioAction::Watch, dec!(20), 3),
            ],
            recommended_action: RecommendedAction {
                action: ScenarioAction::Buy,
                reason: "강세 우위".to_string(),
                confidence_pct: dec!(45),
                order_intent: None,
            },
        }
    }

    #[test]
    fn valid_passes() {
        assert!(validate_scenario_output(&valid_output()).is_ok());
    }

    #[test]
    fn wrong_count_rejected() {
        let mut o = valid_output();
        o.scenarios.pop();
        assert_eq!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::WrongScenarioCount(2))
        );
    }

    #[test]
    fn missing_type_rejected() {
        let mut o = valid_output();
        o.scenarios[2].scenario_type = ScenarioType::Bullish;
        assert_eq!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::MissingScenarioType)
        );
    }

    #[test]
    fn prob_sum_not_100_rejected() {
        let mut o = valid_output();
        o.scenarios[0].probability_pct = dec!(60);
        assert!(matches!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::ProbabilitySumInvalid { .. })
        ));
    }

    #[test]
    fn empty_condition_rejected() {
        let mut o = valid_output();
        o.scenarios[0].condition_text = "   ".to_string();
        assert!(matches!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::EmptyScenarioField { field: "condition_text", .. })
        ));
    }

    #[test]
    fn empty_recommended_reason_rejected() {
        let mut o = valid_output();
        o.recommended_action.reason = "".to_string();
        assert_eq!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::EmptyRecommendedReason)
        );
    }

    #[test]
    fn confidence_out_of_range_rejected() {
        let mut o = valid_output();
        o.recommended_action.confidence_pct = dec!(150);
        assert!(matches!(
            validate_scenario_output(&o),
            Err(ScenarioValidationError::ConfidenceOutOfRange(_))
        ));
    }
}
