use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::scenario::{EvidenceCard, EvidenceSourceType, RecommendedAction, ScenarioItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioPromptInput {
    pub manager_id: Uuid,
    pub symbol_code: String,
    pub base_price: String,
    pub evidence_cards: Vec<EvidenceCard>,
    pub prompt_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOutput {
    pub symbol: String,
    pub base_price: String,
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
    pub analysis_summary: String,
    pub analysis_detail: Option<String>,
    pub scenarios: Vec<ScenarioItem>,
    pub recommended_action: RecommendedAction,
}

// ── 멀티스텝 파이프라인 타입 ──────────────────────────────────────────────────

/// Step 1: Fundamental 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalAnalysis {
    /// 재무/공시 기반 기업 상태 요약 (1-3문장)
    pub health_summary: String,
    /// 주요 재무 지표 관찰 (부채비율, PER, 성장률 등)
    pub key_observations: Vec<String>,
    /// 중장기 리스크 요인
    pub risks: Vec<String>,
    /// 분석에 사용된 evidence card id 목록
    pub evidence_ids: Vec<Uuid>,
}

/// Step 2: News/Event 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEventAnalysis {
    /// 이벤트/뉴스 기반 촉매 요약
    pub catalyst_summary: String,
    /// 감성 방향: "positive" | "negative" | "neutral" | "mixed"
    pub sentiment: String,
    /// 고위험 키워드 감지 여부 (거래정지, 횡령 등)
    pub high_risk_detected: bool,
    /// 감지된 고위험 키워드 목록
    pub high_risk_keywords: Vec<String>,
    /// 분석에 사용된 evidence card id 목록
    pub evidence_ids: Vec<Uuid>,
}

/// Step 3: Strategy 생성 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDraft {
    /// 강세/횡보/약세 각 시나리오 초안
    pub scenarios: Vec<ScenarioItem>,
    /// 추천 액션 초안
    pub recommended_action: RecommendedAction,
    /// 전략 근거 요약
    pub strategy_rationale: String,
}

/// Step 4: Critic 검토 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticReview {
    /// 전략 초안 수용 여부
    pub accepted: bool,
    /// 수정된 시나리오 (accepted=false인 경우 제공)
    pub revised_scenarios: Option<Vec<ScenarioItem>>,
    /// 수정된 추천 액션
    pub revised_action: Option<RecommendedAction>,
    /// Critic 검토 의견
    pub critique: String,
    /// 편향/과적합/논리 비약 지적 목록
    pub issues: Vec<String>,
}

/// 신뢰도 게이팅 적용 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceGateResult {
    /// 게이팅 미발동(통과) 여부
    pub passed: bool,
    /// 게이팅이 발동된 이유 목록
    pub triggered_rules: Vec<String>,
    /// 게이팅 후 조정된 recommended_action
    pub adjusted_action: Option<RecommendedAction>,
}

/// 멀티스텝 분석을 지원하는 LLM 프로바이더
/// 기본 구현은 단계별 메서드가 없을 수 있으므로 trait에 default 구현 제공
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate_scenario(&self, input: ScenarioPromptInput) -> Result<ScenarioOutput>;

    /// Fundamental 분석 (재무/공시 evidence 대상)
    async fn analyze_fundamentals(
        &self,
        symbol_code: &str,
        base_price: &str,
        cards: &[EvidenceCard],
    ) -> Result<FundamentalAnalysis> {
        // 기본: 단순 evidence 요약으로 대체 (mock/간단한 구현용)
        let evidence_ids = cards.iter().map(|c| c.id).collect();
        let observations: Vec<String> = cards
            .iter()
            .filter(|c| {
                matches!(
                    c.source_type,
                    EvidenceSourceType::Financial | EvidenceSourceType::Disclosure
                )
            })
            .take(3)
            .map(|c| c.title.clone())
            .collect();
        Ok(FundamentalAnalysis {
            health_summary: format!(
                "{} 기업 상태: {}개 재무/공시 데이터 기반 분석",
                symbol_code,
                observations.len()
            ),
            key_observations: observations,
            risks: vec![],
            evidence_ids,
        })
    }

    /// News/Event 분석 (뉴스/커뮤니티 evidence 대상)
    async fn analyze_news_events(
        &self,
        symbol_code: &str,
        cards: &[EvidenceCard],
    ) -> Result<NewsEventAnalysis> {
        let evidence_ids = cards.iter().map(|c| c.id).collect();
        let news_cards: Vec<_> = cards
            .iter()
            .filter(|c| {
                matches!(
                    c.source_type,
                    EvidenceSourceType::News | EvidenceSourceType::Community
                )
            })
            .collect();

        let high_risk_keywords = detect_high_risk_keywords(&news_cards);
        let high_risk_detected = !high_risk_keywords.is_empty();

        let sentiment = if high_risk_detected {
            "negative"
        } else if news_cards.is_empty() {
            "neutral"
        } else {
            "mixed"
        };

        Ok(NewsEventAnalysis {
            catalyst_summary: format!(
                "{} 뉴스/이벤트: {}개 항목 분석",
                symbol_code,
                news_cards.len()
            ),
            sentiment: sentiment.to_string(),
            high_risk_detected,
            high_risk_keywords,
            evidence_ids,
        })
    }

    /// Strategy 초안 생성 (fundamental + news 결과를 컨텍스트로 활용)
    async fn draft_strategy(
        &self,
        input: &ScenarioPromptInput,
        fundamental: &FundamentalAnalysis,
        news: &NewsEventAnalysis,
    ) -> Result<StrategyDraft> {
        // 기본: generate_scenario를 호출해 결과를 StrategyDraft로 변환
        let output = self.generate_scenario(input.clone()).await?;
        Ok(StrategyDraft {
            scenarios: output.scenarios,
            recommended_action: output.recommended_action,
            strategy_rationale: format!(
                "펀더멘털: {} | 뉴스 감성: {}",
                fundamental.health_summary, news.sentiment
            ),
        })
    }

    /// Critic 검토 (strategy 초안에 대한 자기비판)
    async fn critic_review(
        &self,
        _input: &ScenarioPromptInput,
        draft: &StrategyDraft,
        _fundamental: &FundamentalAnalysis,
        news: &NewsEventAnalysis,
    ) -> Result<CriticReview> {
        // 기본: 고위험 키워드 감지 시 적극적 매수 의견을 hold로 하향
        let issues = if news.high_risk_detected {
            news.high_risk_keywords
                .iter()
                .map(|k| format!("고위험 키워드 감지: {k}"))
                .collect()
        } else {
            vec![]
        };

        let accepted = issues.is_empty();
        Ok(CriticReview {
            accepted,
            revised_scenarios: None,
            revised_action: None,
            critique: if accepted {
                "전략 초안 검토 통과".to_string()
            } else {
                format!("고위험 신호 {}건 감지 — 전략 재검토 필요", issues.len())
            },
            issues,
        })
    }
}

fn detect_high_risk_keywords(cards: &[&EvidenceCard]) -> Vec<String> {
    const HIGH_RISK: &[&str] = &[
        "거래정지",
        "상장폐지",
        "횡령",
        "배임",
        "감사의견 거절",
        "유상증자",
        "전환사채",
        "파산",
        "부도",
        "리콜",
        "소송",
        "압수수색",
    ];
    let mut found = vec![];
    for card in cards {
        for kw in HIGH_RISK {
            if (card.title.contains(kw) || card.summary.contains(kw))
                && !found.contains(&kw.to_string())
            {
                found.push(kw.to_string());
            }
        }
    }
    found
}
