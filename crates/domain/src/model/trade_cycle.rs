use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::broker::OrderSide;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeCycleStatus {
    Open,
    Closed,
}

/// 매매 사이클: 한 종목의 신규 진입(보유 0 → 매수)부터
/// 전량 청산(보유 0)까지를 하나의 단위로 추적한다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeCycle {
    pub id: Uuid,
    pub manager_id: Uuid,
    pub symbol_id: Uuid,
    pub status: TradeCycleStatus,
    /// 현재 보유 수량 (open이면 > 0, closed면 0)
    pub open_quantity: Decimal,
    pub total_buy_quantity: Decimal,
    pub total_sell_quantity: Decimal,
    pub total_buy_amount: Decimal,
    pub avg_entry_price: Decimal,
    pub total_sell_amount: Decimal,
    pub avg_exit_price: Decimal,
    pub total_fee: Decimal,
    pub total_tax: Decimal,
    /// 청산된 수량에 대한 실현손익 (수수료/세금 반영)
    pub realized_pnl: Decimal,
    pub fill_count: i32,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl TradeCycle {
    /// 신규 사이클을 생성한다 (아직 fill 미반영 상태).
    pub fn new(id: Uuid, manager_id: Uuid, symbol_id: Uuid, opened_at: DateTime<Utc>) -> Self {
        Self {
            id,
            manager_id,
            symbol_id,
            status: TradeCycleStatus::Open,
            open_quantity: Decimal::ZERO,
            total_buy_quantity: Decimal::ZERO,
            total_sell_quantity: Decimal::ZERO,
            total_buy_amount: Decimal::ZERO,
            avg_entry_price: Decimal::ZERO,
            total_sell_amount: Decimal::ZERO,
            avg_exit_price: Decimal::ZERO,
            total_fee: Decimal::ZERO,
            total_tax: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            fill_count: 0,
            opened_at,
            closed_at: None,
            updated_at: opened_at,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.status == TradeCycleStatus::Closed
    }
}

/// 사이클에 반영할 단일 체결 정보.
#[derive(Debug, Clone)]
pub struct CycleFill {
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub tax: Decimal,
    pub filled_at: DateTime<Utc>,
}

/// 체결 하나를 사이클에 적용한 새 사이클 상태를 반환한다 (불변 — 원본을 수정하지 않음).
///
/// 평균원가법(average cost) 기준:
/// - 매수: 보유 수량/평균 진입가를 갱신
/// - 매도: 청산 수량에 대해 실현손익 = qty * (sell_price - avg_entry_price) 계산,
///   매도 수수료/세금을 실현손익에서 차감
/// - 매도 후 보유 수량이 0이 되면 status=closed, closed_at 설정
pub fn apply_fill(cycle: &TradeCycle, fill: &CycleFill) -> TradeCycle {
    let mut next = cycle.clone();
    next.fill_count += 1;
    next.total_fee += fill.fee;
    next.total_tax += fill.tax;
    next.updated_at = fill.filled_at;

    match fill.side {
        OrderSide::Buy => {
            let buy_amount = fill.quantity * fill.price;
            next.total_buy_quantity += fill.quantity;
            next.total_buy_amount += buy_amount;
            next.open_quantity += fill.quantity;
            // 평균 진입가 = 누적 매수금액 / 누적 매수수량
            if next.total_buy_quantity > Decimal::ZERO {
                next.avg_entry_price = next.total_buy_amount / next.total_buy_quantity;
            }
        }
        OrderSide::Sell => {
            // 보유 수량을 초과해 매도할 수는 없다 (상위 리스크 게이트가 막지만 방어적으로 클램프).
            let sell_qty = fill.quantity.min(next.open_quantity.max(Decimal::ZERO));
            let sell_amount = sell_qty * fill.price;
            next.total_sell_quantity += sell_qty;
            next.total_sell_amount += sell_amount;
            next.open_quantity -= sell_qty;
            if next.total_sell_quantity > Decimal::ZERO {
                next.avg_exit_price = next.total_sell_amount / next.total_sell_quantity;
            }
            // 청산분 실현손익 (평균원가 대비) - 이번 매도 수수료/세금
            let gross = sell_qty * (fill.price - next.avg_entry_price);
            next.realized_pnl += gross - fill.fee - fill.tax;

            if next.open_quantity <= Decimal::ZERO {
                next.open_quantity = Decimal::ZERO;
                next.status = TradeCycleStatus::Closed;
                next.closed_at = Some(fill.filled_at);
            }
        }
    }

    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn cycle() -> TradeCycle {
        let t = Utc::now();
        TradeCycle::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), t)
    }

    fn buy(qty: Decimal, price: Decimal, fee: Decimal) -> CycleFill {
        CycleFill { side: OrderSide::Buy, quantity: qty, price, fee, tax: Decimal::ZERO, filled_at: Utc::now() }
    }

    fn sell(qty: Decimal, price: Decimal, fee: Decimal, tax: Decimal) -> CycleFill {
        CycleFill { side: OrderSide::Sell, quantity: qty, price, fee, tax, filled_at: Utc::now() }
    }

    #[test]
    fn buy_opens_and_sets_avg_entry() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(105)));
        assert_eq!(c.status, TradeCycleStatus::Open);
        assert_eq!(c.open_quantity, dec!(10));
        assert_eq!(c.avg_entry_price, dec!(70_000));
        assert_eq!(c.total_fee, dec!(105));
        assert_eq!(c.fill_count, 1);
    }

    #[test]
    fn additional_buy_averages_entry() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(0)));
        let c = apply_fill(&c, &buy(dec!(10), dec!(80_000), dec!(0)));
        assert_eq!(c.open_quantity, dec!(20));
        assert_eq!(c.avg_entry_price, dec!(75_000));
    }

    #[test]
    fn partial_sell_keeps_open_and_realizes_pnl() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(0)));
        let c = apply_fill(&c, &sell(dec!(4), dec!(75_000), dec!(0), dec!(0)));
        assert_eq!(c.status, TradeCycleStatus::Open);
        assert_eq!(c.open_quantity, dec!(6));
        // 4 * (75000 - 70000) = 20000
        assert_eq!(c.realized_pnl, dec!(20_000));
    }

    #[test]
    fn full_sell_closes_cycle() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(0)));
        let c = apply_fill(&c, &sell(dec!(10), dec!(72_000), dec!(0), dec!(0)));
        assert_eq!(c.status, TradeCycleStatus::Closed);
        assert_eq!(c.open_quantity, dec!(0));
        assert!(c.closed_at.is_some());
        // 10 * (72000 - 70000) = 20000
        assert_eq!(c.realized_pnl, dec!(20_000));
    }

    #[test]
    fn sell_fees_reduce_realized_pnl() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(0)));
        let c = apply_fill(&c, &sell(dec!(10), dec!(72_000), dec!(500), dec!(1_000)));
        // 20000 - 500(fee) - 1000(tax) = 18500
        assert_eq!(c.realized_pnl, dec!(18_500));
    }

    #[test]
    fn oversell_is_clamped_to_holdings() {
        let c = apply_fill(&cycle(), &buy(dec!(10), dec!(70_000), dec!(0)));
        // 보유 10인데 15 매도 시도 → 10만 청산되고 closed
        let c = apply_fill(&c, &sell(dec!(15), dec!(72_000), dec!(0), dec!(0)));
        assert_eq!(c.status, TradeCycleStatus::Closed);
        assert_eq!(c.open_quantity, dec!(0));
        assert_eq!(c.total_sell_quantity, dec!(10));
    }
}
