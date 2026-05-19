use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use lumos_domain::model::scenario::{
    EvidenceCard, EvidenceSourceType, RecommendedAction, ScenarioAction, ScenarioItem,
    ScenarioType,
};
use lumos_domain::port::llm::{
    CriticReview, FundamentalAnalysis, LlmProvider, NewsEventAnalysis, ScenarioOutput,
    ScenarioPromptInput, StrategyDraft,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI Chat Completions API를 사용하는 멀티스텝 LLM 프로바이더.
/// `base_url`을 바꾸면 OpenAI 호환 API(로컬 LLM 등)도 지원한다.
pub struct OpenAiLlmProvider {
    api_key: String,
    model: String,
    base_url: String,
    http: Client,
}

impl OpenAiLlmProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self::with_base_url(api_key, model, DEFAULT_BASE_URL.to_string())
    }

    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
            http: Client::new(),
        }
    }
}

// ── API 요청/응답 DTO ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<UsageInfo>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct UsageInfo {
    prompt_tokens: u32,
    completion_tokens: u32,
}

impl OpenAiLlmProvider {
    async fn chat(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: self.model.clone(),
                messages: vec![
                    ChatMessage { role: "system".to_string(), content: system.to_string() },
                    ChatMessage { role: "user".to_string(), content: user.to_string() },
                ],
                temperature: 0.3,
                response_format: ResponseFormat { format_type: "json_object".to_string() },
            })
            .send()
            .await
            .context("OpenAI request failed")?
            .json::<ChatResponse>()
            .await
            .context("OpenAI response parse failed")?;

        if let Some(usage) = resp.usage {
            tracing::debug!(
                model = %self.model,
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                "openai usage"
            );
        }

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow::anyhow!("empty choices from OpenAI"))
    }

    fn evidence_text(cards: &[EvidenceCard]) -> String {
        cards
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "[{}] [{}] {} — {}",
                    i + 1,
                    c.source_name,
                    c.title,
                    c.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn filter_by_type<'a>(
        cards: &'a [EvidenceCard],
        types: &[EvidenceSourceType],
    ) -> Vec<&'a EvidenceCard> {
        cards.iter().filter(|c| types.contains(&c.source_type)).collect()
    }
}

#[async_trait]
impl LlmProvider for OpenAiLlmProvider {
    // ── 기존 단일 호출 (하위 호환) ────────────────────────────────────────────

    async fn generate_scenario(&self, input: ScenarioPromptInput) -> Result<ScenarioOutput> {
        let system = include_str!("../../../../prompts/scenario_system_prompt.md");
        let evidence_text = Self::evidence_text(&input.evidence_cards);

        let user = format!(
            "종목: {}\n현재가: {}\n\n근거 카드:\n{}\n\n위 데이터를 분석하여 JSON 형식으로 시나리오를 생성하세요.",
            input.symbol_code, input.base_price, evidence_text
        );

        let raw = self.chat(system, &user).await?;
        parse_scenario_output(&raw, &input)
    }

    // ── Step 1: Fundamental 분석 ──────────────────────────────────────────────

    async fn analyze_fundamentals(
        &self,
        symbol_code: &str,
        base_price: &str,
        cards: &[EvidenceCard],
    ) -> Result<FundamentalAnalysis> {
        let fund_cards = Self::filter_by_type(
            cards,
            &[EvidenceSourceType::Financial, EvidenceSourceType::Disclosure],
        );
        let evidence_ids: Vec<_> = fund_cards.iter().map(|c| c.id).collect();

        let system = "당신은 재무/공시 분석 전문가입니다. \
            주어진 재무제표, 공시 데이터를 분석하여 기업 펀더멘털을 요약하세요. \
            반드시 JSON만 출력하세요: \
            {\"health_summary\":string, \"key_observations\":[string], \"risks\":[string]}";

        let evidence_text = fund_cards
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[{}] {} — {}", i + 1, c.title, c.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let user = format!(
            "종목: {} | 현재가: {}\n\n재무/공시 데이터:\n{}",
            symbol_code, base_price, evidence_text
        );

        let raw = self.chat(system, &user).await?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .context("fundamental analysis parse failed")?;

        Ok(FundamentalAnalysis {
            health_summary: v["health_summary"].as_str().unwrap_or("").to_string(),
            key_observations: v["key_observations"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            risks: v["risks"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            evidence_ids,
        })
    }

    // ── Step 2: News/Event 분석 ───────────────────────────────────────────────

    async fn analyze_news_events(
        &self,
        symbol_code: &str,
        cards: &[EvidenceCard],
    ) -> Result<NewsEventAnalysis> {
        let news_cards = Self::filter_by_type(
            cards,
            &[EvidenceSourceType::News, EvidenceSourceType::Community],
        );
        let evidence_ids: Vec<_> = news_cards.iter().map(|c| c.id).collect();

        let system = "당신은 뉴스/이벤트 분석 전문가입니다. \
            주어진 뉴스와 커뮤니티 데이터에서 투자 촉매와 리스크를 요약하세요. \
            고위험 키워드(거래정지, 상장폐지, 횡령, 배임, 유상증자, 전환사채, 파산, 부도, 리콜, 소송, 압수수색)를 감지하세요. \
            반드시 JSON만 출력하세요: \
            {\"catalyst_summary\":string, \"sentiment\":\"positive\"|\"negative\"|\"neutral\"|\"mixed\", \
            \"high_risk_detected\":bool, \"high_risk_keywords\":[string]}";

        let evidence_text = news_cards
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[{}] {} — {}", i + 1, c.title, c.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let user = format!("종목: {}\n\n뉴스/커뮤니티 데이터:\n{}", symbol_code, evidence_text);

        let raw = self.chat(system, &user).await?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .context("news analysis parse failed")?;

        let high_risk_keywords: Vec<String> = v["high_risk_keywords"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Ok(NewsEventAnalysis {
            catalyst_summary: v["catalyst_summary"].as_str().unwrap_or("").to_string(),
            sentiment: v["sentiment"].as_str().unwrap_or("neutral").to_string(),
            high_risk_detected: v["high_risk_detected"].as_bool().unwrap_or(!high_risk_keywords.is_empty()),
            high_risk_keywords,
            evidence_ids,
        })
    }

    // ── Step 3: Strategy 초안 생성 ────────────────────────────────────────────

    async fn draft_strategy(
        &self,
        input: &ScenarioPromptInput,
        fundamental: &FundamentalAnalysis,
        news: &NewsEventAnalysis,
    ) -> Result<StrategyDraft> {
        let system = include_str!("../../../../prompts/scenario_system_prompt.md");

        let evidence_text = Self::evidence_text(&input.evidence_cards);
        let user = format!(
            "종목: {} | 현재가: {}\n\n[펀더멘털 분석]\n{}\n주요 관찰: {}\n리스크: {}\n\n\
            [뉴스/이벤트]\n감성: {} | 촉매: {}\n고위험: {}\n\n\
            [전체 근거 카드]\n{}\n\n\
            위 분석을 바탕으로 bullish/sideways/bearish 시나리오와 추천 액션을 JSON으로 생성하세요. \
            형식: {{\"scenarios\":[...], \"recommended_action\":{{...}}, \"strategy_rationale\":string}}",
            input.symbol_code,
            input.base_price,
            fundamental.health_summary,
            fundamental.key_observations.join(", "),
            fundamental.risks.join(", "),
            news.sentiment,
            news.catalyst_summary,
            if news.high_risk_detected {
                format!("감지됨: {}", news.high_risk_keywords.join(", "))
            } else {
                "없음".to_string()
            },
            evidence_text,
        );

        let raw = self.chat(system, &user).await?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .context("strategy draft parse failed")?;

        let scenarios = parse_scenario_items(&v["scenarios"], input)?;
        let recommended_action = parse_recommended_action(&v["recommended_action"])?;
        let strategy_rationale = v["strategy_rationale"]
            .as_str()
            .unwrap_or("전략 근거 없음")
            .to_string();

        Ok(StrategyDraft { scenarios, recommended_action, strategy_rationale })
    }

    // ── Step 4: Critic 검토 ───────────────────────────────────────────────────

    async fn critic_review(
        &self,
        input: &ScenarioPromptInput,
        draft: &StrategyDraft,
        fundamental: &FundamentalAnalysis,
        news: &NewsEventAnalysis,
    ) -> Result<CriticReview> {
        let system = "당신은 투자 전략 Critic입니다. \
            주어진 시나리오 전략 초안을 검토하고 편향, 과적합, 논리 비약을 지적하세요. \
            필요하면 시나리오와 추천 액션을 수정하세요. \
            반드시 JSON만 출력하세요: \
            {\"accepted\":bool, \"critique\":string, \"issues\":[string], \
            \"revised_scenarios\":null|[...], \"revised_action\":null|{...}}";

        let draft_json = serde_json::to_string_pretty(&json!({
            "scenarios": draft.scenarios.iter().map(|s| json!({
                "scenario_type": format!("{:?}", s.scenario_type).to_lowercase(),
                "action": format!("{:?}", s.action).to_lowercase(),
                "probability_pct": s.probability_pct,
                "condition_text": s.condition_text,
                "strategy_text": s.strategy_text,
                "risk_text": s.risk_text,
            })).collect::<Vec<_>>(),
            "recommended_action": {
                "action": format!("{:?}", draft.recommended_action.action).to_lowercase(),
                "reason": draft.recommended_action.reason,
                "confidence_pct": draft.recommended_action.confidence_pct,
            },
            "strategy_rationale": draft.strategy_rationale,
        }))
        .unwrap_or_default();

        let user = format!(
            "종목: {} | 현재가: {}\n\n[펀더멘털]\n{}\n고위험키워드: {}\n\n[전략 초안]\n{}",
            input.symbol_code,
            input.base_price,
            fundamental.health_summary,
            if news.high_risk_detected {
                news.high_risk_keywords.join(", ")
            } else {
                "없음".to_string()
            },
            draft_json,
        );

        let raw = self.chat(system, &user).await?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .context("critic review parse failed")?;

        let accepted = v["accepted"].as_bool().unwrap_or(true);
        let critique = v["critique"].as_str().unwrap_or("").to_string();
        let issues: Vec<String> = v["issues"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let revised_scenarios = if !accepted && !v["revised_scenarios"].is_null() {
            parse_scenario_items(&v["revised_scenarios"], input).ok()
        } else {
            None
        };
        let revised_action = if !accepted && !v["revised_action"].is_null() {
            parse_recommended_action(&v["revised_action"]).ok()
        } else {
            None
        };

        Ok(CriticReview { accepted, revised_scenarios, revised_action, critique, issues })
    }
}

// ── 파싱 헬퍼 ────────────────────────────────────────────────────────────────

fn parse_scenario_output(raw: &str, input: &ScenarioPromptInput) -> Result<ScenarioOutput> {
    let v: serde_json::Value = serde_json::from_str(raw)
        .context("scenario output parse failed")?;

    let scenarios = parse_scenario_items(&v["scenarios"], input)?;
    let recommended_action = parse_recommended_action(&v["recommended_action"])?;

    let analysis_summary = v["analysis_summary"]
        .as_str()
        .unwrap_or("분석 완료")
        .to_string();
    let analysis_detail = v["analysis_detail"].as_str().map(|s| s.to_string());

    Ok(ScenarioOutput {
        symbol: input.symbol_code.clone(),
        base_price: input.base_price.clone(),
        analyzed_at: chrono::Utc::now(),
        analysis_summary,
        analysis_detail,
        scenarios,
        recommended_action,
    })
}

fn parse_scenario_items(
    v: &serde_json::Value,
    input: &ScenarioPromptInput,
) -> Result<Vec<ScenarioItem>> {
    let arr = v.as_array().ok_or_else(|| anyhow::anyhow!("scenarios is not an array"))?;
    if arr.is_empty() {
        bail!("empty scenarios array");
    }

    let mut items: Vec<ScenarioItem> = arr
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let scenario_type = match s["scenario_type"].as_str().unwrap_or("sideways") {
                "bullish" => ScenarioType::Bullish,
                "bearish" => ScenarioType::Bearish,
                _ => ScenarioType::Sideways,
            };
            let action = match s["action"].as_str().unwrap_or("watch") {
                "buy" => ScenarioAction::Buy,
                "sell" => ScenarioAction::Sell,
                "hold" => ScenarioAction::Hold,
                _ => ScenarioAction::Watch,
            };
            let probability_pct = s["probability_pct"]
                .as_f64()
                .map(rust_decimal::Decimal::from_f64_retain)
                .flatten()
                .unwrap_or(rust_decimal_macros::dec!(33));

            let target_price = s["target_price"]
                .as_f64()
                .map(rust_decimal::Decimal::from_f64_retain)
                .flatten();
            let stop_loss_price = s["stop_loss_price"]
                .as_f64()
                .map(rust_decimal::Decimal::from_f64_retain)
                .flatten();

            ScenarioItem {
                id: uuid::Uuid::new_v4(),
                scenario_run_id: uuid::Uuid::nil(),
                analysis_report_id: None,
                symbol_id: uuid::Uuid::nil(),
                scenario_type,
                action,
                probability_pct,
                target_price,
                stop_loss_price,
                condition_text: s["condition_text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                strategy_text: s["strategy_text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                risk_text: s["risk_text"].as_str().map(|s| s.to_string()),
                rank_order: (i + 1) as i32,
            }
        })
        .collect();

    // 확률 합계 검증 (허용 오차 ±10)
    let total: rust_decimal::Decimal = items.iter().map(|s| s.probability_pct).sum();
    let diff = (total - rust_decimal_macros::dec!(100)).abs();
    if diff > rust_decimal_macros::dec!(10) {
        tracing::warn!(
            symbol = %input.symbol_code,
            total = %total,
            "scenario probability sum is far from 100"
        );
    }

    Ok(items)
}

fn parse_recommended_action(v: &serde_json::Value) -> Result<RecommendedAction> {
    let action = match v["action"].as_str().unwrap_or("watch") {
        "buy" => ScenarioAction::Buy,
        "sell" => ScenarioAction::Sell,
        "hold" => ScenarioAction::Hold,
        _ => ScenarioAction::Watch,
    };
    let reason = v["reason"].as_str().unwrap_or("").to_string();
    let confidence_pct = v["confidence_pct"]
        .as_f64()
        .map(rust_decimal::Decimal::from_f64_retain)
        .flatten()
        .unwrap_or(rust_decimal_macros::dec!(50));

    Ok(RecommendedAction {
        action,
        reason,
        confidence_pct,
        order_intent: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scenario_items_valid() {
        let input = ScenarioPromptInput {
            manager_id: uuid::Uuid::new_v4(),
            symbol_code: "AAPL".to_string(),
            base_price: "200".to_string(),
            evidence_cards: vec![],
            prompt_version: "v1".to_string(),
        };

        let v = serde_json::json!([
            {
                "scenario_type": "bullish",
                "action": "buy",
                "probability_pct": 50.0,
                "condition_text": "강세 조건",
                "strategy_text": "매수 전략",
            },
            {
                "scenario_type": "sideways",
                "action": "hold",
                "probability_pct": 30.0,
                "condition_text": "횡보 조건",
                "strategy_text": "관망 전략",
            },
            {
                "scenario_type": "bearish",
                "action": "watch",
                "probability_pct": 20.0,
                "condition_text": "약세 조건",
                "strategy_text": "약세 전략",
            }
        ]);

        let items = parse_scenario_items(&v, &input).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].scenario_type, ScenarioType::Bullish);
        assert_eq!(items[0].action, ScenarioAction::Buy);
    }

    #[test]
    fn parse_recommended_action_valid() {
        let v = serde_json::json!({
            "action": "buy",
            "reason": "강세 시나리오 우세",
            "confidence_pct": 55.0
        });
        let action = parse_recommended_action(&v).unwrap();
        assert_eq!(action.action, ScenarioAction::Buy);
    }
}
