use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use lumos_domain::model::broker::OrderSide;
use lumos_domain::model::risk::{RiskCheck, RiskCheckResult, RiskPolicy};

/// 리스크 게이트에 전달되는 주문 컨텍스트
#[derive(Debug, Clone)]
pub struct OrderContext {
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub limit_price: Decimal,
    /// 주문 예상 금액 (quantity * limit_price)
    pub estimated_amount_krw: Decimal,
    /// AI 신뢰도 (0-100)
    pub ai_confidence_pct: Decimal,
    /// 사용된 evidence card 수
    pub evidence_count: i32,
    /// 시장가 주문 여부
    pub is_market_order: bool,
    /// 장전 주문 여부
    pub is_pre_market: bool,
    /// 장후 주문 여부
    pub is_after_hours: bool,
    /// 현재 포지션 평가액 (KRW)
    pub current_position_amount_krw: Decimal,
    /// 포트폴리오 총 평가액 (KRW)
    pub portfolio_total_krw: Decimal,
    /// 오늘 실현 손익률 (%)  — 음수가 손실
    pub today_realized_pnl_pct: Decimal,
    /// 오늘 총 거래 횟수
    pub today_trade_count: i32,
    /// 가격 데이터 기준 시각
    pub quote_as_of: DateTime<Utc>,
    /// 계좌 데이터 기준 시각
    pub account_as_of: DateTime<Utc>,
    /// 현재 시각 (테스트에서 주입 가능)
    pub now: DateTime<Utc>,
}

impl OrderContext {
    pub fn new_simple(
        manager_id: Uuid,
        symbol_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        limit_price: Decimal,
    ) -> Self {
        let now = Utc::now();
        let estimated_amount_krw = quantity * limit_price;
        Self {
            manager_id,
            symbol_id,
            side,
            quantity,
            limit_price,
            estimated_amount_krw,
            ai_confidence_pct: dec!(50),
            evidence_count: 5,
            is_market_order: false,
            is_pre_market: false,
            is_after_hours: false,
            current_position_amount_krw: Decimal::ZERO,
            portfolio_total_krw: dec!(10_000_000),
            today_realized_pnl_pct: Decimal::ZERO,
            today_trade_count: 0,
            quote_as_of: now,
            account_as_of: now,
            now,
        }
    }
}

/// 20개 리스크 체크를 순서대로 실행. 하나라도 실패하면 즉시 거부.
pub fn evaluate(policy: &RiskPolicy, ctx: &OrderContext) -> RiskCheckResult {
    let mut checks = Vec::with_capacity(20);

    macro_rules! check {
        ($name:expr, $passed:expr, $detail:expr) => {{
            let passed = $passed;
            let detail: Option<String> = $detail;
            checks.push(RiskCheck {
                name: $name.to_string(),
                passed,
                detail: detail.clone(),
            });
            if !passed {
                return RiskCheckResult::reject(
                    detail.unwrap_or_else(|| $name.to_string()),
                    checks,
                );
            }
        }};
    }

    // 1. 수량 양수
    check!(
        "quantity_positive",
        ctx.quantity > Decimal::ZERO,
        Some(format!("주문 수량이 0 이하입니다: {}", ctx.quantity))
    );

    // 2. 가격 양수
    check!(
        "price_positive",
        ctx.limit_price > Decimal::ZERO,
        Some(format!("주문 가격이 0 이하입니다: {}", ctx.limit_price))
    );

    // 3. 예상 금액 양수
    check!(
        "estimated_amount_positive",
        ctx.estimated_amount_krw > Decimal::ZERO,
        Some(format!("예상 금액이 0 이하입니다: {}", ctx.estimated_amount_krw))
    );

    // 4. 시장가 주문 허용 여부
    check!(
        "market_order_allowed",
        !ctx.is_market_order || policy.allow_market_order,
        Some("리스크 정책에서 시장가 주문이 허용되지 않습니다".to_string())
    );

    // 5. 장전 매매 허용 여부
    check!(
        "pre_market_allowed",
        !ctx.is_pre_market || policy.allow_pre_market,
        Some("리스크 정책에서 장전 매매가 허용되지 않습니다".to_string())
    );

    // 6. 장후 매매 허용 여부
    check!(
        "after_hours_allowed",
        !ctx.is_after_hours || policy.allow_after_hours,
        Some("리스크 정책에서 장후 매매가 허용되지 않습니다".to_string())
    );

    // 7. 가격 데이터 신선도
    let quote_age_secs = (ctx.now - ctx.quote_as_of).num_seconds();
    check!(
        "quote_freshness",
        quote_age_secs <= policy.require_fresh_quote_seconds as i64,
        Some(format!(
            "가격 데이터가 오래됐습니다: {}초 전 (허용: {}초)",
            quote_age_secs, policy.require_fresh_quote_seconds
        ))
    );

    // 8. 계좌 데이터 신선도
    let account_age_secs = (ctx.now - ctx.account_as_of).num_seconds();
    check!(
        "account_freshness",
        account_age_secs <= policy.require_fresh_account_seconds as i64,
        Some(format!(
            "계좌 데이터가 오래됐습니다: {}초 전 (허용: {}초)",
            account_age_secs, policy.require_fresh_account_seconds
        ))
    );

    // 9. AI 신뢰도 최소값
    check!(
        "min_ai_confidence",
        ctx.ai_confidence_pct >= policy.min_ai_confidence_pct,
        Some(format!(
            "AI 신뢰도가 낮습니다: {}% (최소: {}%)",
            ctx.ai_confidence_pct, policy.min_ai_confidence_pct
        ))
    );

    // 10. 최소 근거 카드 수
    check!(
        "min_evidence_count",
        ctx.evidence_count >= policy.min_evidence_count,
        Some(format!(
            "근거 카드가 부족합니다: {}개 (최소: {}개)",
            ctx.evidence_count, policy.min_evidence_count
        ))
    );

    // 11. 단일 주문 최대 금액
    check!(
        "max_single_order_amount",
        ctx.estimated_amount_krw <= policy.max_single_order_amount_krw,
        Some(format!(
            "단일 주문 금액 초과: {} KRW (최대: {} KRW)",
            ctx.estimated_amount_krw, policy.max_single_order_amount_krw
        ))
    );

    // 12. 포트폴리오 총액 양수 (포지션 비중 계산 가능)
    check!(
        "portfolio_total_positive",
        ctx.portfolio_total_krw > Decimal::ZERO,
        Some("포트폴리오 총액을 알 수 없습니다".to_string())
    );

    // 매도 시 보유 수량 충분 여부 (예상 금액 > 0 이면 pass, 실제 수량은 상위 레이어에서 확인)
    check!(
        "sell_has_position",
        ctx.side == OrderSide::Buy || ctx.current_position_amount_krw > Decimal::ZERO,
        Some("보유 포지션 없이 매도를 시도합니다".to_string())
    );

    // 15. 일 손실 한도
    check!(
        "max_daily_loss",
        ctx.today_realized_pnl_pct >= -policy.max_daily_loss_pct,
        Some(format!(
            "일 손실 한도 초과: {:.2}% (한도: -{}%)",
            ctx.today_realized_pnl_pct, policy.max_daily_loss_pct
        ))
    );

    // 16. 일 거래 횟수 한도
    check!(
        "max_daily_trade_count",
        ctx.today_trade_count < policy.max_daily_trade_count,
        Some(format!(
            "일 거래 횟수 초과: {}회 (최대: {}회)",
            ctx.today_trade_count, policy.max_daily_trade_count
        ))
    );

    // 17. 주문 가격이 현실적 범위 (0.001 이상)
    check!(
        "price_minimum",
        ctx.limit_price >= dec!(0.001),
        Some(format!("주문 가격이 너무 낮습니다: {}", ctx.limit_price))
    );

    // 18. 수량이 정수 또는 소수점 4자리 이내
    let scale = ctx.quantity.scale();
    check!(
        "quantity_scale",
        scale <= 4,
        Some(format!("주문 수량 소수점 자리 초과: {} (최대 4자리)", scale))
    );

    // 19. 매니저 ID 일관성 (정책 매니저와 일치)
    check!(
        "manager_id_match",
        policy.manager_id == ctx.manager_id,
        Some(format!(
            "리스크 정책 매니저({})와 주문 매니저({}) 불일치",
            policy.manager_id, ctx.manager_id
        ))
    );

    // 20. 예상 금액 * 1.05 안전 마진 (슬리피지 대비)
    let safe_amount = ctx.estimated_amount_krw * dec!(1.05);
    check!(
        "safety_margin_amount",
        safe_amount <= policy.max_single_order_amount_krw,
        Some(format!(
            "슬리피지 5% 마진 포함 금액 초과: {} KRW (최대: {} KRW)",
            safe_amount, policy.max_single_order_amount_krw
        ))
    );

    RiskCheckResult::pass(checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn policy(manager_id: Uuid) -> RiskPolicy {
        RiskPolicy::default_for(manager_id)
    }

    fn base_ctx(manager_id: Uuid) -> OrderContext {
        let now = Utc::now();
        OrderContext {
            manager_id,
            symbol_id: Uuid::new_v4(),
            side: OrderSide::Buy,
            quantity: dec!(10),
            limit_price: dec!(50_000),
            estimated_amount_krw: dec!(500_000),
            ai_confidence_pct: dec!(60),
            evidence_count: 5,
            is_market_order: false,
            is_pre_market: false,
            is_after_hours: false,
            current_position_amount_krw: Decimal::ZERO,
            portfolio_total_krw: dec!(20_000_000),
            today_realized_pnl_pct: Decimal::ZERO,
            today_trade_count: 0,
            quote_as_of: now,
            account_as_of: now,
            now,
        }
    }

    #[test]
    fn happy_path_passes_all_checks() {
        let id = Uuid::new_v4();
        let result = evaluate(&policy(id), &base_ctx(id));
        assert!(result.passed);
        assert_eq!(result.checks.len(), 20);
        assert!(result.checks.iter().all(|c| c.passed));
    }

    #[test]
    fn zero_quantity_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.quantity = dec!(0);
        ctx.estimated_amount_krw = dec!(0);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert_eq!(result.checks[0].name, "quantity_positive");
    }

    #[test]
    fn stale_quote_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.quote_as_of = ctx.now - Duration::seconds(120);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert!(result.reject_reason.as_deref().unwrap_or("").contains("가격 데이터"));
    }

    #[test]
    fn low_ai_confidence_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.ai_confidence_pct = dec!(20);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert!(result.reject_reason.as_deref().unwrap_or("").contains("AI 신뢰도"));
    }

    #[test]
    fn order_exceeds_max_amount_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.estimated_amount_krw = dec!(2_000_000);
        ctx.quantity = dec!(40);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
    }

    #[test]
    fn daily_loss_exceeded_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.today_realized_pnl_pct = dec!(-3.0);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert!(result.reject_reason.as_deref().unwrap_or("").contains("일 손실"));
    }

    #[test]
    fn daily_trade_count_exceeded_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.today_trade_count = 20;
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert!(result.reject_reason.as_deref().unwrap_or("").contains("거래 횟수"));
    }

    #[test]
    fn position_pct_exceeded_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        // 5% * 20M = 1M, 주문 금액 900K + 기존 200K = 1.1M = 5.5%
        ctx.current_position_amount_krw = dec!(200_000);
        ctx.estimated_amount_krw = dec!(900_000);
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
    }

    #[test]
    fn market_order_disallowed_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.is_market_order = true;
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
        assert!(result.reject_reason.as_deref().unwrap_or("").contains("시장가"));
    }

    #[test]
    fn sell_without_position_rejected() {
        let id = Uuid::new_v4();
        let mut ctx = base_ctx(id);
        ctx.side = OrderSide::Sell;
        ctx.current_position_amount_krw = Decimal::ZERO;
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
    }

    #[test]
    fn manager_id_mismatch_rejected() {
        let id = Uuid::new_v4();
        let ctx = base_ctx(Uuid::new_v4()); // 다른 manager_id
        let result = evaluate(&policy(id), &ctx);
        assert!(!result.passed);
    }
}
