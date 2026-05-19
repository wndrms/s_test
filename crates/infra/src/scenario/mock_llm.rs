use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal_macros::dec;
use uuid::Uuid;

use lumos_domain::model::scenario::{
    RecommendedAction, ScenarioAction, ScenarioItem, ScenarioType,
};
use lumos_domain::port::llm::{LlmProvider, ScenarioOutput, ScenarioPromptInput};

/// offline-fixtures 모드에서 사용하는 결정적(deterministic) 목 LLM
pub struct MockLlmProvider;

impl MockLlmProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn generate_scenario(&self, input: ScenarioPromptInput) -> Result<ScenarioOutput> {
        let run_id = Uuid::new_v4();
        let symbol_id = Uuid::nil();

        let scenarios = vec![
            ScenarioItem {
                id: Uuid::new_v4(),
                scenario_run_id: run_id,
                analysis_report_id: None,
                symbol_id,
                scenario_type: ScenarioType::Bullish,
                action: ScenarioAction::Buy,
                probability_pct: dec!(45),
                target_price: Some(
                    input
                        .base_price
                        .parse()
                        .map(|p: rust_decimal::Decimal| p * dec!(1.10))
                        .unwrap_or(dec!(0)),
                ),
                stop_loss_price: Some(
                    input
                        .base_price
                        .parse()
                        .map(|p: rust_decimal::Decimal| p * dec!(0.95))
                        .unwrap_or(dec!(0)),
                ),
                condition_text: format!(
                    "[MOCK] {} 강세 조건: 거래량 증가 및 주요 저항선 돌파 시",
                    input.symbol_code
                ),
                strategy_text: "목표가 +10% 도달 시 분할 매도".to_string(),
                risk_text: Some("시장 전반 조정 시 손절매 -5%".to_string()),
                rank_order: 1,
            },
            ScenarioItem {
                id: Uuid::new_v4(),
                scenario_run_id: run_id,
                analysis_report_id: None,
                symbol_id,
                scenario_type: ScenarioType::Sideways,
                action: ScenarioAction::Hold,
                probability_pct: dec!(35),
                target_price: None,
                stop_loss_price: None,
                condition_text: format!(
                    "[MOCK] {} 횡보 조건: 박스권 내 등락 지속",
                    input.symbol_code
                ),
                strategy_text: "관망 유지, 돌파 신호 대기".to_string(),
                risk_text: None,
                rank_order: 2,
            },
            ScenarioItem {
                id: Uuid::new_v4(),
                scenario_run_id: run_id,
                analysis_report_id: None,
                symbol_id,
                scenario_type: ScenarioType::Bearish,
                action: ScenarioAction::Watch,
                probability_pct: dec!(20),
                target_price: Some(
                    input
                        .base_price
                        .parse()
                        .map(|p: rust_decimal::Decimal| p * dec!(0.90))
                        .unwrap_or(dec!(0)),
                ),
                stop_loss_price: None,
                condition_text: format!(
                    "[MOCK] {} 약세 조건: 외국인 순매도 지속 및 섹터 약세",
                    input.symbol_code
                ),
                strategy_text: "매수 자제, 기존 포지션 점진적 축소 검토".to_string(),
                risk_text: Some("추가 하락 시 손실 확대 가능".to_string()),
                rank_order: 3,
            },
        ];

        let evidence_count = input.evidence_cards.len();
        let summary = format!(
            "[MOCK 분석] {} 종목에 대해 {}개의 근거 카드를 기반으로 생성된 시나리오입니다. \
             현재가 {} 기준으로 강세(45%) / 횡보(35%) / 약세(20%) 시나리오를 제시합니다.",
            input.symbol_code, evidence_count, input.base_price
        );

        Ok(ScenarioOutput {
            symbol: input.symbol_code,
            base_price: input.base_price,
            analyzed_at: Utc::now(),
            analysis_summary: summary,
            analysis_detail: None,
            scenarios,
            recommended_action: RecommendedAction {
                action: ScenarioAction::Buy,
                reason: "[MOCK] 강세 시나리오 확률 우위 및 위험 대비 수익 비율 양호".to_string(),
                confidence_pct: dec!(45),
                order_intent: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumos_domain::model::scenario::EvidenceCard;

    fn dummy_input() -> ScenarioPromptInput {
        ScenarioPromptInput {
            manager_id: Uuid::new_v4(),
            symbol_code: "005930".to_string(),
            base_price: "75000".to_string(),
            evidence_cards: vec![],
            prompt_version: "v1".to_string(),
        }
    }

    #[tokio::test]
    async fn mock_returns_three_scenarios() {
        let llm = MockLlmProvider::new();
        let output = llm.generate_scenario(dummy_input()).await.unwrap();
        assert_eq!(output.scenarios.len(), 3);
        assert_eq!(output.symbol, "005930");
    }

    #[tokio::test]
    async fn probability_sums_to_100() {
        let llm = MockLlmProvider::new();
        let output = llm.generate_scenario(dummy_input()).await.unwrap();
        let total: rust_decimal::Decimal = output.scenarios.iter().map(|s| s.probability_pct).sum();
        assert_eq!(total, dec!(100));
    }

    #[tokio::test]
    async fn recommended_action_confidence_matches_bullish_probability() {
        let llm = MockLlmProvider::new();
        let output = llm.generate_scenario(dummy_input()).await.unwrap();
        assert_eq!(output.recommended_action.confidence_pct, dec!(45));
    }

    #[tokio::test]
    async fn target_price_scaled_from_base() {
        let llm = MockLlmProvider::new();
        let output = llm.generate_scenario(dummy_input()).await.unwrap();
        let bullish = output.scenarios.iter().find(|s| s.scenario_type == ScenarioType::Bullish).unwrap();
        // 75000 * 1.10 = 82500
        assert_eq!(bullish.target_price, Some(dec!(82500)));
    }
}
