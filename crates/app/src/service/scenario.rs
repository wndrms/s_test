use std::sync::Arc;
use uuid::Uuid;

use rust_decimal::Decimal;

use lumos_domain::model::scenario::{
    EvidenceCard, EvidenceSourceType, RecommendedAction, ScenarioAction, ScenarioItem,
    ScenarioStatus,
};
use lumos_domain::port::llm::{
    CriticReview, LlmProvider, NewsEventAnalysis, ScenarioOutput, ScenarioPromptInput,
};

use crate::error::{AppError, AppResult};
use crate::repo::analysis_report::{
    AnalysisReportRepository, CreateAnalysisReportInput, CreateChartAnnotationInput,
};
use crate::repo::scenario::{
    CreateScenarioItemInput, CreateScenarioRunInput, EvidenceCardRepository,
    ScenarioItemRepository, ScenarioRunRepository,
};
use crate::repo::symbol::SymbolRepository;

const BUDGET_PRICE: u32 = 5;
const BUDGET_DISCLOSURE: u32 = 3;
const BUDGET_FINANCIAL: u32 = 3;
const BUDGET_NEWS: u32 = 5;
const BUDGET_COMMUNITY: u32 = 3;

pub struct ScenarioService {
    llm: Arc<dyn LlmProvider>,
    evidence_repo: Arc<dyn EvidenceCardRepository>,
    scenario_run_repo: Arc<dyn ScenarioRunRepository>,
    scenario_item_repo: Arc<dyn ScenarioItemRepository>,
    symbol_repo: Arc<dyn SymbolRepository>,
    report_repo: Option<Arc<dyn AnalysisReportRepository>>,
    /// true이면 멀티스텝 파이프라인(Fundamental→News→Strategy→Critic) 사용
    use_multistep: bool,
}

impl ScenarioService {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        evidence_repo: Arc<dyn EvidenceCardRepository>,
        scenario_run_repo: Arc<dyn ScenarioRunRepository>,
        scenario_item_repo: Arc<dyn ScenarioItemRepository>,
        symbol_repo: Arc<dyn SymbolRepository>,
    ) -> Self {
        Self {
            llm,
            evidence_repo,
            scenario_run_repo,
            scenario_item_repo,
            symbol_repo,
            report_repo: None,
            use_multistep: false,
        }
    }

    pub fn with_report_repo(mut self, repo: Arc<dyn AnalysisReportRepository>) -> Self {
        self.report_repo = Some(repo);
        self
    }

    /// 멀티스텝 파이프라인을 활성화한다.
    /// LlmProvider가 analyze_fundamentals/analyze_news_events/draft_strategy/critic_review를
    /// 실제 LLM 호출로 구현한 경우에 사용한다.
    pub fn with_multistep(mut self) -> Self {
        self.use_multistep = true;
        self
    }

    pub async fn run_for_symbol(
        &self,
        manager_id: Uuid,
        symbol_id: Uuid,
        schedule_slot_id: Option<Uuid>,
        model_provider: String,
        model_name: String,
        prompt_version: String,
        base_price: String,
        extra_evidence: Vec<EvidenceCard>,
        llm_override: Option<Arc<dyn LlmProvider>>,
    ) -> AppResult<Uuid> {
        let symbol = self
            .symbol_repo
            .find_by_id(symbol_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound(format!("symbol {symbol_id}")))?;

        // 1. Evidence Card 수집
        let mut evidence = vec![];
        for (source_type, limit) in [
            (EvidenceSourceType::Price, BUDGET_PRICE),
            (EvidenceSourceType::Technical, BUDGET_PRICE),
            (EvidenceSourceType::Disclosure, BUDGET_DISCLOSURE),
            (EvidenceSourceType::Financial, BUDGET_FINANCIAL),
            (EvidenceSourceType::News, BUDGET_NEWS),
            (EvidenceSourceType::Community, BUDGET_COMMUNITY),
        ] {
            let mut cards = self
                .evidence_repo
                .find_for_symbol(symbol_id, &[source_type], limit)
                .await
                .map_err(AppError::Internal)?;
            evidence.append(&mut cards);
        }
        evidence.extend(extra_evidence);
        let evidence_ids: Vec<Uuid> = evidence.iter().map(|e| e.id).collect();

        // 2. ScenarioRun 생성
        let run = self
            .scenario_run_repo
            .create(CreateScenarioRunInput {
                manager_id,
                schedule_slot_id,
                model_provider: model_provider.clone(),
                model_name: model_name.clone(),
                prompt_version: prompt_version.clone(),
            })
            .await
            .map_err(AppError::Internal)?;

        // 3. LLM 파이프라인 실행
        let base_price_dec: Decimal = base_price.parse().unwrap_or(Decimal::ZERO);
        let prompt_input = ScenarioPromptInput {
            manager_id,
            symbol_code: symbol.code.clone(),
            base_price: base_price.clone(),
            evidence_cards: evidence.clone(),
            prompt_version,
        };

        let active_llm: &dyn LlmProvider = llm_override
            .as_ref()
            .map(|a| a.as_ref())
            .unwrap_or(self.llm.as_ref());

        let output = if self.use_multistep {
            match self.run_multistep_pipeline_with(&prompt_input, &evidence, active_llm).await {
                Ok(o) => o,
                Err(e) => {
                    let _ = self
                        .scenario_run_repo
                        .update_status(run.id, ScenarioStatus::Failed)
                        .await;
                    return Err(e);
                }
            }
        } else {
            match active_llm.generate_scenario(prompt_input).await {
                Ok(o) => o,
                Err(e) => {
                    let _ = self
                        .scenario_run_repo
                        .update_status(run.id, ScenarioStatus::Failed)
                        .await;
                    return Err(AppError::Internal(e));
                }
            }
        };

        // 4. AnalysisReport 저장 (report_repo 주입된 경우)
        let report_id = if let Some(repo) = &self.report_repo {
            let report = repo
                .create(CreateAnalysisReportInput {
                    manager_id,
                    symbol_id,
                    scenario_run_id: run.id,
                    base_price: base_price_dec,
                    analyzed_at: output.analyzed_at,
                    report_text: output.analysis_detail
                        .clone()
                        .unwrap_or_else(|| output.analysis_summary.clone()),
                    report_summary: Some(output.analysis_summary.clone()),
                })
                .await
                .map_err(AppError::Internal)?;

            if !evidence_ids.is_empty() {
                let _ = repo.link_evidence(report.id, &evidence_ids).await;
            }

            for scenario in &output.scenarios {
                if let Some(tp) = scenario.target_price {
                    let label = format!(
                        "{} 목표가",
                        scenario_type_short(&format!("{:?}", scenario.scenario_type))
                    );
                    let _ = repo
                        .create_annotation(CreateChartAnnotationInput {
                            analysis_report_id: report.id,
                            symbol_id,
                            annotation_type: "target".to_string(),
                            price: tp,
                            label,
                            color_hint: Some(
                                scenario_type_color(&format!("{:?}", scenario.scenario_type))
                                    .to_string(),
                            ),
                        })
                        .await;
                }
                if let Some(sl) = scenario.stop_loss_price {
                    if matches!(scenario.action, ScenarioAction::Buy) {
                        let _ = repo
                            .create_annotation(CreateChartAnnotationInput {
                                analysis_report_id: report.id,
                                symbol_id,
                                annotation_type: "stop_loss".to_string(),
                                price: sl,
                                label: "손절가".to_string(),
                                color_hint: Some("#ef4444".to_string()),
                            })
                            .await;
                    }
                }
            }

            Some(report.id)
        } else {
            None
        };

        // 5. ScenarioItem 저장
        let items: Vec<CreateScenarioItemInput> = output
            .scenarios
            .into_iter()
            .map(|item| CreateScenarioItemInput {
                scenario_run_id: run.id,
                symbol_id,
                item,
            })
            .collect();

        let saved_items = self
            .scenario_item_repo
            .create_batch(items)
            .await
            .map_err(AppError::Internal)?;

        // 6. scenario_item에 report_id 연결
        if let (Some(rid), Some(repo)) = (report_id, &self.report_repo) {
            for item in &saved_items {
                let _ = repo.update_scenario_item_report(item.id, rid).await;
            }
        }

        // 7. 상태 업데이트
        self.scenario_run_repo
            .update_status(run.id, ScenarioStatus::Validated)
            .await
            .map_err(AppError::Internal)?;

        Ok(run.id)
    }

    async fn run_multistep_pipeline_with(
        &self,
        input: &ScenarioPromptInput,
        evidence: &[lumos_domain::model::scenario::EvidenceCard],
        llm: &dyn LlmProvider,
    ) -> AppResult<ScenarioOutput> {
        // Step 1: Fundamental 분석
        let fundamental = llm
            .analyze_fundamentals(&input.symbol_code, &input.base_price, evidence)
            .await
            .map_err(AppError::Internal)?;

        tracing::debug!(
            symbol = %input.symbol_code,
            health = %fundamental.health_summary,
            "multistep: fundamental done"
        );

        // Step 2: News/Event 분석
        let news = llm
            .analyze_news_events(&input.symbol_code, evidence)
            .await
            .map_err(AppError::Internal)?;

        tracing::debug!(
            symbol = %input.symbol_code,
            sentiment = %news.sentiment,
            high_risk = news.high_risk_detected,
            "multistep: news done"
        );

        // Step 3: Strategy 초안 생성
        let draft = llm
            .draft_strategy(input, &fundamental, &news)
            .await
            .map_err(AppError::Internal)?;

        tracing::debug!(
            symbol = %input.symbol_code,
            scenarios = draft.scenarios.len(),
            "multistep: strategy draft done"
        );

        // Step 4: Critic 검토
        let critic = llm
            .critic_review(input, &draft, &fundamental, &news)
            .await
            .map_err(AppError::Internal)?;

        tracing::debug!(
            symbol = %input.symbol_code,
            accepted = critic.accepted,
            issues = critic.issues.len(),
            "multistep: critic done"
        );

        // Critic 결과 반영
        let (final_scenarios, critic_action) = if critic.accepted {
            (draft.scenarios, draft.recommended_action)
        } else {
            (
                critic.revised_scenarios.clone().unwrap_or(draft.scenarios),
                critic.revised_action.clone().unwrap_or(draft.recommended_action),
            )
        };

        // 신뢰도 게이팅 적용
        let (final_action, gate_rules) =
            apply_confidence_gate(&final_scenarios, critic_action, &critic, &news);

        let gate_triggered = !gate_rules.is_empty();

        tracing::debug!(
            symbol = %input.symbol_code,
            gate_triggered,
            rules = ?gate_rules,
            "multistep: confidence gate done"
        );

        // 멀티스텝 분석 결과를 report_text에 포함
        let mut analysis_detail = format!(
            "## 펀더멘털 분석\n{}\n\n## 뉴스/이벤트 분석\n감성: {} | 고위험: {}\n{}\n\n## 전략 근거\n{}\n\n## Critic 검토\n{}",
            fundamental.health_summary,
            news.sentiment,
            if news.high_risk_detected { "감지됨" } else { "없음" },
            news.catalyst_summary,
            draft.strategy_rationale,
            critic.critique,
        );

        if gate_triggered {
            analysis_detail.push_str(&format!(
                "\n\n## 신뢰도 게이팅\n[신뢰도 게이팅 적용]\n발동 규칙:\n{}",
                gate_rules
                    .iter()
                    .map(|r| format!("- {r}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        let analysis_summary = format!(
            "{} | 감성:{} | Critic:{} | 게이팅:{}",
            fundamental.health_summary,
            news.sentiment,
            if critic.accepted { "통과" } else { "수정됨" },
            if gate_triggered { "발동" } else { "없음" }
        );

        Ok(ScenarioOutput {
            symbol: input.symbol_code.clone(),
            base_price: input.base_price.clone(),
            analyzed_at: chrono::Utc::now(),
            analysis_summary,
            analysis_detail: Some(analysis_detail),
            scenarios: final_scenarios,
            recommended_action: final_action,
        })
    }

    pub async fn list_items_for_run(
        &self,
        run_id: Uuid,
    ) -> AppResult<Vec<lumos_domain::model::scenario::ScenarioItem>> {
        self.scenario_item_repo
            .find_by_run(run_id)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn latest_runs(
        &self,
        manager_id: Uuid,
        limit: u32,
    ) -> AppResult<Vec<lumos_domain::model::scenario::ScenarioRun>> {
        self.scenario_run_repo
            .find_latest_for_manager(manager_id, limit)
            .await
            .map_err(AppError::Internal)
    }
}

fn scenario_type_short(t: &str) -> &'static str {
    match t.to_lowercase().as_str() {
        "bullish" => "강세",
        "bearish" => "약세",
        _ => "횡보",
    }
}

fn scenario_type_color(t: &str) -> &'static str {
    match t.to_lowercase().as_str() {
        "bullish" => "#22c55e",
        "bearish" => "#ef4444",
        _ => "#f59e0b",
    }
}

/// 신뢰도 게이팅을 적용한다.
///
/// # 규칙
/// 1. Critic이 issues를 발견했고 수정된 시나리오가 없으면 → `Watch`로 강제 하향
/// 2. final_scenarios의 모든 probability_pct 합계가 70 미만이면 → 전체 Watch 처리
/// 3. news.high_risk_detected == true이고 recommended_action이 Buy이면 → Hold로 하향,
///    risk_text에 고위험 키워드 추가
///
/// # 반환
/// `(조정된 RecommendedAction, 발동된 규칙 목록)`
fn apply_confidence_gate(
    scenarios: &[ScenarioItem],
    action: RecommendedAction,
    critic: &CriticReview,
    news: &NewsEventAnalysis,
) -> (RecommendedAction, Vec<String>) {
    let mut triggered_rules: Vec<String> = Vec::new();
    let mut adjusted_action = action;

    // 규칙 1: Critic이 issues를 발견했고 수정된 시나리오가 없으면 Watch로 하향
    if !critic.issues.is_empty() && critic.revised_scenarios.is_none() {
        triggered_rules.push(format!(
            "Critic issues {}건 감지됐으나 수정된 시나리오 없음 — recommended_action을 Watch로 하향",
            critic.issues.len()
        ));
        adjusted_action = RecommendedAction {
            action: ScenarioAction::Watch,
            reason: format!(
                "{}; Critic 지적 사항 미수정으로 신뢰도 게이팅 발동",
                adjusted_action.reason
            ),
            confidence_pct: adjusted_action.confidence_pct,
            order_intent: None,
        };
    }

    // 규칙 2: 모든 probability_pct 합계가 70 미만이면 Watch로 처리
    let total_prob: Decimal = scenarios
        .iter()
        .map(|s| s.probability_pct)
        .fold(Decimal::ZERO, |acc, p| acc + p);

    if total_prob < Decimal::from(70) {
        triggered_rules.push(format!(
            "시나리오 확률 합계 {total_prob}% < 70% — 신뢰도 부족으로 Watch 처리"
        ));
        adjusted_action = RecommendedAction {
            action: ScenarioAction::Watch,
            reason: format!(
                "{}; 시나리오 확률 합계({total_prob}%)가 기준(70%) 미달",
                adjusted_action.reason
            ),
            confidence_pct: adjusted_action.confidence_pct,
            order_intent: None,
        };
    }

    // 규칙 3: 고위험 감지 + Buy → Hold로 하향
    if news.high_risk_detected && adjusted_action.action == ScenarioAction::Buy {
        let keywords = news.high_risk_keywords.join(", ");
        triggered_rules.push(format!(
            "고위험 키워드 감지({keywords}) + Buy 추천 — Hold로 하향"
        ));
        adjusted_action = RecommendedAction {
            action: ScenarioAction::Hold,
            reason: format!(
                "{}; 고위험 키워드({keywords}) 감지로 Buy → Hold 하향",
                adjusted_action.reason
            ),
            confidence_pct: adjusted_action.confidence_pct,
            order_intent: None,
        };
    }

    (adjusted_action, triggered_rules)
}

// ── 단위 테스트 ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lumos_domain::model::scenario::ScenarioType;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn make_scenario(prob: u32) -> ScenarioItem {
        ScenarioItem {
            id: Uuid::new_v4(),
            scenario_run_id: Uuid::new_v4(),
            analysis_report_id: None,
            symbol_id: Uuid::new_v4(),
            scenario_type: ScenarioType::Sideways,
            action: ScenarioAction::Hold,
            probability_pct: Decimal::from(prob),
            target_price: None,
            stop_loss_price: None,
            condition_text: "test condition".to_string(),
            strategy_text: "test strategy".to_string(),
            risk_text: None,
            rank_order: 1,
        }
    }

    fn make_action(action: ScenarioAction) -> RecommendedAction {
        RecommendedAction {
            action,
            reason: "기본 이유".to_string(),
            confidence_pct: Decimal::from(80),
            order_intent: None,
        }
    }

    fn make_critic(has_issues: bool, has_revised: bool) -> CriticReview {
        CriticReview {
            accepted: !has_issues,
            revised_scenarios: if has_revised {
                Some(vec![make_scenario(50)])
            } else {
                None
            },
            revised_action: None,
            critique: "test critique".to_string(),
            issues: if has_issues {
                vec!["논리 비약".to_string(), "과적합 우려".to_string()]
            } else {
                vec![]
            },
        }
    }

    fn make_news(high_risk: bool, keywords: Vec<&str>) -> NewsEventAnalysis {
        NewsEventAnalysis {
            catalyst_summary: "테스트 뉴스 요약".to_string(),
            sentiment: if high_risk {
                "negative".to_string()
            } else {
                "neutral".to_string()
            },
            high_risk_detected: high_risk,
            high_risk_keywords: keywords.into_iter().map(String::from).collect(),
            evidence_ids: vec![],
        }
    }

    /// 규칙 1: Critic issues 있고 수정 시나리오 없으면 Watch로 하향
    #[test]
    fn rule1_critic_issues_without_revised_forces_watch() {
        let scenarios = vec![make_scenario(40), make_scenario(40)]; // 합계 80 ≥ 70
        let action = make_action(ScenarioAction::Buy);
        let critic = make_critic(true, false); // issues 있음, 수정 없음
        let news = make_news(false, vec![]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        assert_eq!(adjusted.action, ScenarioAction::Watch);
        assert!(!rules.is_empty());
        assert!(rules[0].contains("Critic issues"));
    }

    /// 규칙 1 예외: Critic issues 있어도 수정 시나리오가 있으면 하향 안 함
    #[test]
    fn rule1_critic_issues_with_revised_does_not_force_watch() {
        let scenarios = vec![make_scenario(40), make_scenario(40)]; // 합계 80 ≥ 70
        let action = make_action(ScenarioAction::Buy);
        let critic = make_critic(true, true); // issues 있음, 수정 있음
        let news = make_news(false, vec![]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        // 규칙 1은 미발동; 규칙 3(Buy + no high_risk)도 미발동
        let rule1_triggered = rules.iter().any(|r| r.contains("Critic issues"));
        assert!(!rule1_triggered);
        // action은 Buy 유지 (고위험 없으므로 규칙 3 미발동)
        assert_eq!(adjusted.action, ScenarioAction::Buy);
    }

    /// 규칙 2: 확률 합계 70 미만이면 Watch로 처리
    #[test]
    fn rule2_low_probability_sum_forces_watch() {
        let scenarios = vec![make_scenario(30), make_scenario(30)]; // 합계 60 < 70
        let action = make_action(ScenarioAction::Buy);
        let critic = make_critic(false, false);
        let news = make_news(false, vec![]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        assert_eq!(adjusted.action, ScenarioAction::Watch);
        assert!(rules.iter().any(|r| r.contains("시나리오 확률 합계")));
    }

    /// 규칙 2: 확률 합계가 정확히 70이면 발동하지 않음
    #[test]
    fn rule2_exactly_70_does_not_trigger() {
        let scenarios = vec![make_scenario(35), make_scenario(35)]; // 합계 70
        let action = make_action(ScenarioAction::Hold);
        let critic = make_critic(false, false);
        let news = make_news(false, vec![]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        let rule2_triggered = rules.iter().any(|r| r.contains("시나리오 확률 합계"));
        assert!(!rule2_triggered);
        assert_eq!(adjusted.action, ScenarioAction::Hold);
    }

    /// 규칙 3: 고위험 + Buy → Hold로 하향
    #[test]
    fn rule3_high_risk_with_buy_forces_hold() {
        let scenarios = vec![make_scenario(40), make_scenario(40)]; // 합계 80 ≥ 70
        let action = make_action(ScenarioAction::Buy);
        let critic = make_critic(false, false);
        let news = make_news(true, vec!["횡령", "상장폐지"]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        assert_eq!(adjusted.action, ScenarioAction::Hold);
        assert!(rules.iter().any(|r| r.contains("고위험 키워드")));
        assert!(adjusted.reason.contains("횡령"));
    }

    /// 규칙 3: 고위험이어도 Buy가 아니면 미발동
    #[test]
    fn rule3_high_risk_with_hold_does_not_trigger() {
        let scenarios = vec![make_scenario(40), make_scenario(40)];
        let action = make_action(ScenarioAction::Hold);
        let critic = make_critic(false, false);
        let news = make_news(true, vec!["횡령"]);

        let (adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        let rule3_triggered = rules.iter().any(|r| r.contains("고위험 키워드"));
        assert!(!rule3_triggered);
        assert_eq!(adjusted.action, ScenarioAction::Hold);
    }

    /// 게이팅 미발동 시 rules 비어있음
    #[test]
    fn no_gate_triggered_empty_rules() {
        let scenarios = vec![make_scenario(40), make_scenario(40)];
        let action = make_action(ScenarioAction::Hold);
        let critic = make_critic(false, false);
        let news = make_news(false, vec![]);

        let (_adjusted, rules) = apply_confidence_gate(&scenarios, action, &critic, &news);

        assert!(rules.is_empty());
    }
}
