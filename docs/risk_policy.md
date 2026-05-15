# Risk Policy v1

## Hard decisions from product owner

- Only limit orders are allowed.
- KRX regular session only for Korean stocks.
- US regular session only for US stocks.
- Default risk limits:
  - max position per symbol: 5% of manager equity
  - max single order amount: 1,000,000 KRW equivalent
  - max daily loss: -2% of manager equity
  - auto-pause when daily loss limit is reached

## Mandatory risk gate order

Before creating a broker order:

```text
1. manager.status == active
2. manager.auto_trade_enabled == true
3. global_kill_switch == false
4. mode/broker route is valid: paper -> PaperBroker, live -> KisBroker
5. symbol is in manager_universe
6. market session is regular session only
7. latest quote is fresh
8. account/balance snapshot is fresh
9. order side is buy or sell
10. order type is limit only
11. order quantity > 0
12. buy order amount <= max_single_order_amount
13. resulting position value <= 5% equity
14. daily realized + unrealized loss > -2% equity
15. daily trade count <= configured max
16. idempotency key not used before
17. data warnings are not blocking
18. AI output schema was validated
19. scenario evidence_refs exist
20. order_plan persisted before broker call
```

## Suggested default config

```json
{
  "max_position_pct": "5.0",
  "max_single_order_amount_krw": "1000000",
  "max_daily_loss_pct": "2.0",
  "max_daily_trade_count": 20,
  "allow_market_order": false,
  "allow_pre_market": false,
  "allow_after_hours": false,
  "require_fresh_quote_seconds": 60,
  "require_fresh_account_seconds": 60,
  "min_ai_confidence_pct": 40,
  "min_evidence_count": 2
}
```

## Idempotency key

Use a deterministic key before broker order:

```text
manager_id + trading_date + schedule_slot + symbol_id + side + limit_price + scenario_run_id
```

`order_plans.idempotency_key` must be UNIQUE.

## Data freshness policy

```text
Blocking:
- KIS current price missing
- KIS account/balance missing
- market calendar cannot be verified
- broker order state sync failed for same symbol today
- quote older than 60 seconds during auto-trade

Non-blocking but score penalty:
- news older than 2 hours
- community older than 6 hours
- financial statements older than last reported quarter
```

## Live trading escalation

Even though automatic execution is the target, implement a feature flag per manager:

```text
paper_auto_trade: allowed
live_auto_trade: disabled by default until user explicitly enables
live_order_amount_cap: default 1,000,000 KRW equivalent
```
