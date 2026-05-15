use anyhow::Result;
use async_trait::async_trait;

use crate::model::broker::{
    BrokerAccount, BrokerFill, BrokerOrderResponse, BrokerPosition, BuyingPower,
    BuyingPowerRequest, CancelOrderRequest, LimitOrderRequest, OrderFillQuery,
};

#[async_trait]
pub trait Broker: Send + Sync {
    async fn get_account(&self) -> Result<BrokerAccount>;
    async fn get_positions(&self) -> Result<Vec<BrokerPosition>>;
    async fn get_buying_power(&self, req: BuyingPowerRequest) -> Result<BuyingPower>;
    async fn place_limit_order(&self, req: LimitOrderRequest) -> Result<BrokerOrderResponse>;
    async fn cancel_order(&self, req: CancelOrderRequest) -> Result<BrokerOrderResponse>;
    async fn get_order_fills(&self, req: OrderFillQuery) -> Result<Vec<BrokerFill>>;
}
