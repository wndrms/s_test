use anyhow::{bail, Result};
use serde_json::Value;

/// LLM이 반환한 JSON을 scenario_output.schema.json 계약에 따라 검증
pub fn validate_scenario_output(json: &Value) -> Result<()> {
    let obj = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("root must be an object"))?;

    require_string(obj, "symbol", 1, None)?;
    require_string_pattern(obj, "base_price", r"^\d+(\.\d+)?$")?;
    require_string(obj, "analyzed_at", 1, None)?;
    require_string(obj, "analysis_summary", 1, Some(2000))?;

    if let Some(detail) = obj.get("analysis_detail") {
        if !detail.is_null() {
            let s = detail
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("analysis_detail must be string or null"))?;
            if s.len() > 12000 {
                bail!("analysis_detail exceeds 12000 chars");
            }
        }
    }

    validate_data_freshness(obj)?;
    validate_scenarios(obj)?;
    validate_recommended_action(obj)?;

    Ok(())
}

fn validate_data_freshness(root: &serde_json::Map<String, Value>) -> Result<()> {
    let df = root
        .get("data_freshness")
        .ok_or_else(|| anyhow::anyhow!("missing data_freshness"))?
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("data_freshness must be an object"))?;

    require_string(df, "price_as_of", 1, None)?;
    require_string(df, "account_as_of", 1, None)?;

    let level = df
        .get("freshness_level")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing freshness_level"))?;
    if !matches!(level, "fresh" | "stale" | "blocking") {
        bail!("freshness_level must be fresh|stale|blocking, got: {level}");
    }

    Ok(())
}

fn validate_scenarios(root: &serde_json::Map<String, Value>) -> Result<()> {
    let arr = root
        .get("scenarios")
        .ok_or_else(|| anyhow::anyhow!("missing scenarios"))?
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("scenarios must be an array"))?;

    if arr.len() != 3 {
        bail!("scenarios must have exactly 3 items, got {}", arr.len());
    }

    let prob_sum: f64 = arr
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let obj = item
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("scenario[{i}] must be an object"))?;

            let stype = obj
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("scenario[{i}] missing type"))?;
            if !matches!(stype, "bullish" | "sideways" | "bearish") {
                bail!("scenario[{i}].type must be bullish|sideways|bearish, got: {stype}");
            }

            let action = obj
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("scenario[{i}] missing action"))?;
            if !matches!(action, "buy" | "sell" | "hold" | "watch") {
                bail!("scenario[{i}].action must be buy|sell|hold|watch, got: {action}");
            }

            let prob = obj
                .get("probability_pct")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| anyhow::anyhow!("scenario[{i}] missing probability_pct"))?;
            if !(0.0..=100.0).contains(&prob) {
                bail!("scenario[{i}].probability_pct out of range: {prob}");
            }

            require_string(obj, "condition", 1, Some(1600))?;
            require_string(obj, "strategy", 1, Some(1200))?;
            require_string(obj, "reason", 1, Some(1600))?;

            let refs = obj
                .get("evidence_refs")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("scenario[{i}] missing evidence_refs"))?;
            if refs.is_empty() || refs.len() > 12 {
                bail!("scenario[{i}].evidence_refs must have 1-12 items");
            }

            Ok(prob)
        })
        .collect::<Result<Vec<f64>>>()?
        .into_iter()
        .sum();

    if (prob_sum - 100.0).abs() > 0.01 {
        bail!("scenario probabilities must sum to 100, got {prob_sum:.2}");
    }

    Ok(())
}

fn validate_recommended_action(root: &serde_json::Map<String, Value>) -> Result<()> {
    let ra = root
        .get("recommended_action")
        .ok_or_else(|| anyhow::anyhow!("missing recommended_action"))?
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("recommended_action must be an object"))?;

    let action = ra
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("recommended_action missing action"))?;
    if !matches!(action, "buy" | "sell" | "hold" | "watch") {
        bail!("recommended_action.action invalid: {action}");
    }

    require_string(ra, "reason", 1, Some(1200))?;

    let conf = ra
        .get("confidence_pct")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| anyhow::anyhow!("recommended_action missing confidence_pct"))?;
    if !(0.0..=100.0).contains(&conf) {
        bail!("recommended_action.confidence_pct out of range: {conf}");
    }

    if let Some(intent) = ra.get("order_intent") {
        if !intent.is_null() {
            validate_order_intent(intent)?;
        }
    }

    Ok(())
}

fn validate_order_intent(v: &Value) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("order_intent must be an object"))?;

    let side = obj
        .get("side")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("order_intent missing side"))?;
    if !matches!(side, "buy" | "sell") {
        bail!("order_intent.side must be buy|sell, got: {side}");
    }

    let order_type = obj
        .get("order_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("order_intent missing order_type"))?;
    if order_type != "limit" {
        bail!("order_intent.order_type must be limit, got: {order_type}");
    }

    require_string_pattern(obj, "limit_price", r"^\d+(\.\d+)?$")?;

    Ok(())
}

fn require_string(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    min_len: usize,
    max_len: Option<usize>,
) -> Result<()> {
    let s = obj
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing or non-string field: {key}"))?;

    if s.len() < min_len {
        bail!("{key} too short (min {min_len})");
    }
    if let Some(max) = max_len {
        if s.len() > max {
            bail!("{key} too long (max {max})");
        }
    }
    Ok(())
}

fn require_string_pattern(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    pattern: &str,
) -> Result<()> {
    let s = obj
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing or non-string field: {key}"))?;

    let re = regex_lite(pattern);
    if !re.is_match(s) {
        bail!("{key} does not match pattern {pattern}: got {s}");
    }
    Ok(())
}

fn regex_lite(pattern: &str) -> SimpleRegex {
    SimpleRegex {
        pattern: pattern.to_string(),
    }
}

struct SimpleRegex {
    pattern: String,
}

impl SimpleRegex {
    fn is_match(&self, s: &str) -> bool {
        match self.pattern.as_str() {
            r"^\d+(\.\d+)?$" => {
                if s.is_empty() {
                    return false;
                }
                let parts: Vec<&str> = s.splitn(2, '.').collect();
                parts[0].chars().all(|c| c.is_ascii_digit())
                    && parts.get(1).map_or(true, |frac| {
                        !frac.is_empty() && frac.chars().all(|c| c.is_ascii_digit())
                    })
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_output() -> Value {
        json!({
            "symbol": "005930",
            "base_price": "75000",
            "analyzed_at": "2024-01-01T09:00:00Z",
            "analysis_summary": "테스트 분석 요약",
            "data_freshness": {
                "price_as_of": "2024-01-01T09:00:00Z",
                "account_as_of": "2024-01-01T09:00:00Z",
                "freshness_level": "fresh"
            },
            "scenarios": [
                {
                    "type": "bullish",
                    "probability_pct": 45,
                    "action": "buy",
                    "target_price": "82500",
                    "stop_loss_price": "71250",
                    "condition": "강세 조건 텍스트",
                    "strategy": "강세 전략",
                    "reason": "강세 이유",
                    "evidence_refs": ["ev-1"]
                },
                {
                    "type": "sideways",
                    "probability_pct": 35,
                    "action": "hold",
                    "target_price": null,
                    "condition": "횡보 조건 텍스트",
                    "strategy": "횡보 전략",
                    "reason": "횡보 이유",
                    "evidence_refs": ["ev-2"]
                },
                {
                    "type": "bearish",
                    "probability_pct": 20,
                    "action": "watch",
                    "target_price": "67500",
                    "condition": "약세 조건 텍스트",
                    "strategy": "약세 전략",
                    "reason": "약세 이유",
                    "evidence_refs": ["ev-3"]
                }
            ],
            "recommended_action": {
                "action": "buy",
                "reason": "강세 우위",
                "confidence_pct": 45
            }
        })
    }

    #[test]
    fn valid_json_passes() {
        assert!(validate_scenario_output(&valid_output()).is_ok());
    }

    #[test]
    fn missing_symbol_fails() {
        let mut v = valid_output();
        v.as_object_mut().unwrap().remove("symbol");
        assert!(validate_scenario_output(&v).is_err());
    }

    #[test]
    fn wrong_scenario_count_fails() {
        let mut v = valid_output();
        v["scenarios"] = json!([v["scenarios"][0].clone(), v["scenarios"][1].clone()]);
        assert!(validate_scenario_output(&v).is_err());
    }

    #[test]
    fn probabilities_not_100_fails() {
        let mut v = valid_output();
        v["scenarios"][0]["probability_pct"] = json!(50);
        assert!(validate_scenario_output(&v).is_err());
    }

    #[test]
    fn invalid_freshness_level_fails() {
        let mut v = valid_output();
        v["data_freshness"]["freshness_level"] = json!("unknown");
        assert!(validate_scenario_output(&v).is_err());
    }

    #[test]
    fn invalid_base_price_format_fails() {
        let mut v = valid_output();
        v["base_price"] = json!("abc");
        assert!(validate_scenario_output(&v).is_err());
    }
}
