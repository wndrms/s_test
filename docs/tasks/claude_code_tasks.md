# Claude Code Task Plan

## Task 0. Repository Bootstrap

```text
Create a Rust workspace for a PC/mobile PWA AI trading manager.

Crates:
- domain
- app
- infra
- api
- web
- worker

Use:
- axum
- leptos
- tokio
- sqlx
- postgres
- serde
- rust_decimal
- async-trait
- tracing
- secrecy
- aes-gcm or age-compatible encryption wrapper

Read all files in docs/spec pack before coding.
Do not implement real trading before PaperBroker and Risk Engine exist.
```

Acceptance:

- `cargo check --workspace` passes.
- `api` server has `/health`.
- `web` renders basic shell.
- `domain` has pure models without HTTP/DB dependencies.

## Task 1. Database and Migrations

```text
Create SQLx migrations from db/schema_v1.sql.
Add repository interfaces and Postgres implementations for:
- users
- secret_keys
- broker_connections
- managers
- symbols
- manager_schedule
- schedule_slots
- risk_policies
```

Acceptance:

- `sqlx migrate run` works.
- Basic CRUD integration tests pass with local PostgreSQL.

## Task 2. BYOK Secret Store

```text
Implement encrypted secret storage.
All user-provided API keys must be encrypted at rest.
Never log raw keys.
Show only masked_hint in API responses.
```

Acceptance:

- app key, app secret, LLM key, Naver key, Telegram bot token can be stored.
- raw key never appears in logs.

## Task 3. KIS Client Skeleton

```text
Implement KisClient with environment profiles:
- real REST: https://openapi.koreainvestment.com:9443
- paper REST: https://openapivts.koreainvestment.com:29443

Functions:
- issue_access_token
- hashkey
- domestic_quote
- overseas_quote
- domestic_balance
- overseas_balance
- domestic_buy_limit_order
- domestic_sell_limit_order
- overseas_buy_limit_order
- overseas_sell_limit_order
```

Acceptance:

- Offline tests run against fixture JSON.
- Online tests are behind feature `online-kis` and ignored by default.

## Task 4. PaperBroker

```text
Implement PaperBroker with:
- cash ledger
- positions
- limit order simulation
- fills
- realized PnL
- unrealized PnL
- portfolio snapshots
```

Acceptance:

- Buy/sell cycles produce correct cash and position state.
- Oversell and insufficient cash are rejected.

## Task 5. Manager UI

```text
Implement responsive PWA screens:
- manager list
- manager detail header
- tabs: scenario, holdings, trades, analysis, settings
- bottom nav mobile
- sidebar PC
```

Acceptance:

- Mobile width matches screenshots structurally.
- PC layout uses same routes with wider tables/cards.

## Task 6. Scenario Engine

```text
Implement:
- evidence card builder
- scenario run creation
- multi-scenario output validation using contracts/scenario_output.schema.json
- analysis report storage
- chart annotation storage
```

Acceptance:

- Given fixture evidence cards, LLM mock output validates and persists.
- Invalid schema is rejected and logged.

## Task 7. Schedule Worker

```text
Implement 5-minute schedule slots.
A manager can enable scenario and trade separately per slot.
Worker should avoid duplicate runs with idempotency keys.
```

Acceptance:

- Schedule UI saves selected slots.
- Worker creates scenario jobs at selected times.
- Duplicate process restart does not create duplicate order plans.

## Task 8. Risk Engine

```text
Implement hard-coded defaults:
- limit order only
- regular session only
- max position 5%
- max single order 1,000,000 KRW equivalent
- daily loss stop -2%
- stale price/account blocks order
```

Acceptance:

- Risk rejection reasons are deterministic.
- Order cannot be generated if any blocking condition fails.

## Task 9. Live Order Execution

```text
After PaperBroker and Risk Engine are complete, implement KIS live execution.
Only execute approved order_plans.
Record broker_orders and trade_fills.
```

Acceptance:

- Feature `live-trading` must be explicitly enabled.
- Live trading is disabled in dev/test by default.

## Task 10. External Data Providers

```text
Implement providers:
- OpenDART disclosure/financial
- SEC EDGAR filings/financial facts
- Naver News search
- KIS domestic/overseas news title
- Telegram notifier
```

Acceptance:

- All providers support fixture-based offline tests.
- Raw full article/community body storage is avoided by default.
