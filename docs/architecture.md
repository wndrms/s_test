# Architecture v1

## Recommended stack

```text
Rust workspace
- domain: pure business domain, no network/db dependency
- app: application services, ports/traits
- infra: KIS/OpenDART/SEC/Naver/Telegram/LLM implementations
- api: Axum REST/SSE server
- web: Leptos PWA frontend
- worker: scheduler, collectors, order executor
```

## Runtime topology

초기 개인 서버:

```text
[Leptos PWA] -> [Axum API + Worker single binary] -> [PostgreSQL]
                                    |              
                                    +-> KIS / LLM / DART / SEC / Naver / Telegram
```

VPS 전환 후:

```text
[Leptos PWA]
     |
[Axum API Server] ---- [PostgreSQL]
     |
[SSE]

[Worker Process]
 - schedule runner
 - data collectors
 - AI scenario generator
 - risk engine
 - order executor
 - telegram notifier
```

## Crate dependency rule

```text
domain: no dependency on app/infra/api/web/worker
app: depends on domain only
infra: depends on domain + app
api: depends on domain + app + infra
worker: depends on domain + app + infra
web: API DTO only, never uses broker secrets
```

## External service abstraction

All external integrations must be behind traits:

```rust
#[async_trait::async_trait]
pub trait Broker {
    async fn get_account(&self) -> Result<BrokerAccount>;
    async fn get_positions(&self) -> Result<Vec<BrokerPosition>>;
    async fn get_buying_power(&self, req: BuyingPowerRequest) -> Result<BuyingPower>;
    async fn place_limit_order(&self, req: LimitOrderRequest) -> Result<BrokerOrderResponse>;
    async fn cancel_order(&self, req: CancelOrderRequest) -> Result<BrokerOrderResponse>;
    async fn get_order_fills(&self, req: OrderFillQuery) -> Result<Vec<BrokerFill>>;
}

#[async_trait::async_trait]
pub trait LlmProvider {
    async fn generate_scenario(&self, input: ScenarioPromptInput) -> Result<ScenarioOutput>;
}

#[async_trait::async_trait]
pub trait NewsProvider {
    async fn search_news(&self, query: NewsQuery) -> Result<Vec<NewsItem>>;
}

#[async_trait::async_trait]
pub trait DisclosureProvider {
    async fn recent_filings(&self, symbol: &Symbol) -> Result<Vec<DisclosureItem>>;
}
```

## Data source hierarchy

```text
Tier 0: KIS account/order/quote -> source of truth for trading
Tier 1: DART/SEC filings and financial statements -> official corporate facts
Tier 2: Naver/KIS news -> event and catalyst signal
Tier 3: community/social -> weak sentiment signal only
```

## Safety boundary

LLM never places an order. Flow must be:

```text
LLM scenario -> parsed JSON -> stored scenario -> risk gate -> order_plan -> broker executor
```

## Modes

```text
paper manager -> PaperBroker only
live manager  -> KisBroker only
```

Do not implement a single broker with `if mode == live`. Use separate concrete implementations and explicit construction to avoid accidental live trading.
