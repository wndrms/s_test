use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use lumos_domain::model::broker::{
    BrokerAccount, BrokerFill, BrokerOrderResponse, BrokerOrderStatus, BrokerPosition,
    BuyingPower, BuyingPowerRequest, CancelOrderRequest, LimitOrderRequest, OrderFillQuery,
    OrderSide, PortfolioSnapshot,
};
use lumos_domain::model::symbol::Currency;
use lumos_domain::port::broker::Broker;

#[derive(Debug, Clone)]
struct PaperPosition {
    symbol_code: String,
    quantity: Decimal,
    avg_price: Decimal,
    current_price: Decimal,
}

impl PaperPosition {
    fn market_value(&self) -> Decimal {
        self.quantity * self.current_price
    }

    fn unrealized_pnl(&self) -> Decimal {
        (self.current_price - self.avg_price) * self.quantity
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // idempotency_key/created_at은 주문 메타 보존용 (현재 미참조)
struct PaperOrder {
    id: Uuid,
    symbol_code: String,
    side: OrderSide,
    quantity: Decimal,
    limit_price: Decimal,
    idempotency_key: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug)]
struct PaperState {
    broker_connection_id: Uuid,
    currency: Currency,
    cash: Decimal,
    positions: HashMap<String, PaperPosition>,
    fills: Vec<BrokerFill>,
    pending_orders: HashMap<Uuid, PaperOrder>,
    realized_pnl: Decimal,
}

impl PaperState {
    fn new(broker_connection_id: Uuid, initial_cash: Decimal, currency: Currency) -> Self {
        Self {
            broker_connection_id,
            currency,
            cash: initial_cash,
            positions: HashMap::new(),
            fills: Vec::new(),
            pending_orders: HashMap::new(),
            realized_pnl: Decimal::ZERO,
        }
    }

    fn total_equity(&self) -> Decimal {
        self.cash + self.invested_value()
    }

    fn invested_value(&self) -> Decimal {
        self.positions.values().map(|p| p.market_value()).sum()
    }

    fn unrealized_pnl(&self) -> Decimal {
        self.positions.values().map(|p| p.unrealized_pnl()).sum()
    }
}

pub struct PaperBroker {
    state: Arc<RwLock<PaperState>>,
    quote_source: Arc<dyn Fn(&str) -> Decimal + Send + Sync>,
}

impl PaperBroker {
    pub fn new(
        broker_connection_id: Uuid,
        initial_cash: Decimal,
        currency: Currency,
        quote_source: Arc<dyn Fn(&str) -> Decimal + Send + Sync>,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(PaperState::new(
                broker_connection_id,
                initial_cash,
                currency,
            ))),
            quote_source,
        }
    }

    pub fn with_static_quotes(
        broker_connection_id: Uuid,
        initial_cash: Decimal,
        currency: Currency,
        quotes: HashMap<String, Decimal>,
    ) -> Self {
        let quotes = Arc::new(quotes);
        let source = Arc::new(move |code: &str| {
            quotes.get(code).copied().unwrap_or(dec!(0))
        });
        Self::new(broker_connection_id, initial_cash, currency, source)
    }

    /// 현재 시점의 포트폴리오 스냅샷을 생성한다.
    /// 보유 종목은 quote_source의 현재가로 평가(mark-to-market)한다.
    pub async fn portfolio_snapshot(&self) -> PortfolioSnapshot {
        let mut state = self.state.write().await;
        // 보유 종목을 최신 시세로 평가 갱신
        let codes: Vec<String> = state.positions.keys().cloned().collect();
        for code in codes {
            let price = (self.quote_source)(&code);
            if price > Decimal::ZERO {
                if let Some(pos) = state.positions.get_mut(&code) {
                    pos.current_price = price;
                }
            }
        }
        PortfolioSnapshot {
            equity: state.total_equity(),
            cash: state.cash,
            invested_value: state.invested_value(),
            unrealized_pnl: state.unrealized_pnl(),
            realized_pnl: state.realized_pnl,
            currency: state.currency.clone(),
            as_of: Utc::now(),
        }
    }

    /// Immediately simulates fill at limit_price (paper mode: instant fill).
    async fn execute_fill(&self, order: &PaperOrder) -> Result<BrokerFill> {
        let mut state = self.state.write().await;
        let fill_price = order.limit_price;
        let amount = fill_price * order.quantity;

        match order.side {
            OrderSide::Buy => {
                if state.cash < amount {
                    bail!("insufficient cash: have {}, need {}", state.cash, amount);
                }
                state.cash -= amount;

                let pos = state
                    .positions
                    .entry(order.symbol_code.clone())
                    .or_insert(PaperPosition {
                        symbol_code: order.symbol_code.clone(),
                        quantity: Decimal::ZERO,
                        avg_price: fill_price,
                        current_price: fill_price,
                    });

                let total_cost = pos.avg_price * pos.quantity + fill_price * order.quantity;
                let new_qty = pos.quantity + order.quantity;
                pos.avg_price = if new_qty > Decimal::ZERO {
                    total_cost / new_qty
                } else {
                    fill_price
                };
                pos.quantity = new_qty;
                pos.current_price = fill_price;
            }
            OrderSide::Sell => {
                let pos = state
                    .positions
                    .get_mut(&order.symbol_code)
                    .ok_or_else(|| anyhow::anyhow!("no position for {}", order.symbol_code))?;

                if pos.quantity < order.quantity {
                    bail!(
                        "oversell: have {}, selling {}",
                        pos.quantity,
                        order.quantity
                    );
                }

                let realized = (fill_price - pos.avg_price) * order.quantity;
                pos.quantity -= order.quantity;
                let remaining_qty = pos.quantity;
                if remaining_qty != Decimal::ZERO {
                    pos.current_price = fill_price;
                }

                state.realized_pnl += realized;
                state.cash += amount;

                if remaining_qty == Decimal::ZERO {
                    state.positions.remove(&order.symbol_code);
                }
            }
        }

        let fill = BrokerFill {
            external_order_id: order.id.to_string(),
            symbol_code: order.symbol_code.clone(),
            side: order.side.clone(),
            quantity: order.quantity,
            price: fill_price,
            fee: Decimal::ZERO,
            tax: Decimal::ZERO,
            filled_at: Utc::now(),
        };
        state.fills.push(fill.clone());
        Ok(fill)
    }
}

#[async_trait]
impl Broker for PaperBroker {
    async fn get_account(&self) -> Result<BrokerAccount> {
        let state = self.state.read().await;
        Ok(BrokerAccount {
            broker_connection_id: state.broker_connection_id,
            total_equity: state.total_equity(),
            cash: state.cash,
            currency: state.currency.clone(),
            as_of: Utc::now(),
        })
    }

    async fn get_positions(&self) -> Result<Vec<BrokerPosition>> {
        let state = self.state.read().await;
        Ok(state
            .positions
            .values()
            .map(|p| BrokerPosition {
                symbol_code: p.symbol_code.clone(),
                quantity: p.quantity,
                avg_price: p.avg_price,
                current_price: p.current_price,
                market_value: p.market_value(),
                unrealized_pnl: p.unrealized_pnl(),
            })
            .collect())
    }

    async fn get_buying_power(&self, req: BuyingPowerRequest) -> Result<BuyingPower> {
        let state = self.state.read().await;
        let qty = if req.price > Decimal::ZERO {
            state.cash / req.price
        } else {
            Decimal::ZERO
        };
        Ok(BuyingPower {
            max_quantity: qty.floor(),
            available_cash: state.cash,
            currency: state.currency.clone(),
        })
    }

    async fn place_limit_order(&self, req: LimitOrderRequest) -> Result<BrokerOrderResponse> {
        let order = PaperOrder {
            id: Uuid::new_v4(),
            symbol_code: req.symbol_code.clone(),
            side: req.side.clone(),
            quantity: req.quantity,
            limit_price: req.limit_price,
            idempotency_key: req.idempotency_key.clone(),
            created_at: Utc::now(),
        };

        let fill = self.execute_fill(&order).await?;

        Ok(BrokerOrderResponse {
            external_order_id: Some(order.id.to_string()),
            external_org_no: None,
            status: BrokerOrderStatus::Filled,
            submitted_at: fill.filled_at,
        })
    }

    async fn cancel_order(&self, req: CancelOrderRequest) -> Result<BrokerOrderResponse> {
        let mut state = self.state.write().await;
        let order_uuid = Uuid::parse_str(&req.external_order_id)
            .map_err(|_| anyhow::anyhow!("invalid order id"))?;
        state.pending_orders.remove(&order_uuid);
        Ok(BrokerOrderResponse {
            external_order_id: Some(req.external_order_id),
            external_org_no: None,
            status: BrokerOrderStatus::Canceled,
            submitted_at: Utc::now(),
        })
    }

    async fn get_order_fills(&self, _req: OrderFillQuery) -> Result<Vec<BrokerFill>> {
        let state = self.state.read().await;
        Ok(state.fills.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_broker(cash: Decimal) -> PaperBroker {
        let mut quotes = HashMap::new();
        quotes.insert("005930".to_string(), dec!(75000));
        quotes.insert("AAPL".to_string(), dec!(200));
        PaperBroker::with_static_quotes(Uuid::new_v4(), cash, Currency::Krw, quotes)
    }

    fn buy_req(symbol: &str, qty: Decimal, price: Decimal) -> LimitOrderRequest {
        LimitOrderRequest {
            symbol_code: symbol.to_string(),
            side: OrderSide::Buy,
            quantity: qty,
            limit_price: price,
            idempotency_key: Uuid::new_v4().to_string(),
            market: None,
        }
    }

    fn sell_req(symbol: &str, qty: Decimal, price: Decimal) -> LimitOrderRequest {
        LimitOrderRequest {
            symbol_code: symbol.to_string(),
            side: OrderSide::Sell,
            quantity: qty,
            limit_price: price,
            idempotency_key: Uuid::new_v4().to_string(),
            market: None,
        }
    }

    #[tokio::test]
    async fn buy_updates_cash_and_position() {
        let broker = make_broker(dec!(1000000));
        broker.place_limit_order(buy_req("005930", dec!(10), dec!(75000))).await.unwrap();

        let account = broker.get_account().await.unwrap();
        assert_eq!(account.cash, dec!(250000));

        let positions = broker.get_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].quantity, dec!(10));
    }

    #[tokio::test]
    async fn sell_after_buy_updates_pnl() {
        let broker = make_broker(dec!(1000000));
        broker.place_limit_order(buy_req("005930", dec!(10), dec!(70000))).await.unwrap();
        broker.place_limit_order(sell_req("005930", dec!(10), dec!(75000))).await.unwrap();

        let positions = broker.get_positions().await.unwrap();
        assert!(positions.is_empty());

        let account = broker.get_account().await.unwrap();
        assert_eq!(account.cash, dec!(1050000));
    }

    #[tokio::test]
    async fn insufficient_cash_rejected() {
        let broker = make_broker(dec!(100));
        let result = broker.place_limit_order(buy_req("005930", dec!(10), dec!(75000))).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn oversell_rejected() {
        let broker = make_broker(dec!(1000000));
        broker.place_limit_order(buy_req("005930", dec!(5), dec!(75000))).await.unwrap();
        let result = broker.place_limit_order(sell_req("005930", dec!(10), dec!(75000))).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn portfolio_snapshot_captures_state() {
        let broker = make_broker(dec!(1000000));
        // 10주 @70000 매수 → 현금 300000, 현재가 75000 평가
        broker.place_limit_order(buy_req("005930", dec!(10), dec!(70000))).await.unwrap();

        let snap = broker.portfolio_snapshot().await;
        assert_eq!(snap.cash, dec!(300000));
        // 평가액 = 10 * 75000(quote) = 750000
        assert_eq!(snap.invested_value, dec!(750000));
        assert_eq!(snap.equity, dec!(1050000));
        // 평가손익 = (75000 - 70000) * 10 = 50000
        assert_eq!(snap.unrealized_pnl, dec!(50000));
        assert_eq!(snap.realized_pnl, Decimal::ZERO);
    }

    #[tokio::test]
    async fn snapshot_realized_pnl_after_sell() {
        let broker = make_broker(dec!(1000000));
        broker.place_limit_order(buy_req("005930", dec!(10), dec!(70000))).await.unwrap();
        broker.place_limit_order(sell_req("005930", dec!(10), dec!(75000))).await.unwrap();

        let snap = broker.portfolio_snapshot().await;
        // 전량 청산: 보유 없음, 실현손익 = (75000-70000)*10 = 50000
        assert_eq!(snap.invested_value, Decimal::ZERO);
        assert_eq!(snap.unrealized_pnl, Decimal::ZERO);
        assert_eq!(snap.realized_pnl, dec!(50000));
        assert_eq!(snap.cash, dec!(1050000));
        assert_eq!(snap.equity, dec!(1050000));
    }
}
