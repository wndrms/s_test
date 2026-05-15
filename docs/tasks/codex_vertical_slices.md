# Codex Vertical Slices

Use these as small implementation tickets. Each ticket should compile and test independently.

## Slice 1. Manager List

- API: `GET /api/managers`
- API: `POST /api/managers`
- UI: manager card with name, mode, masked account, current equity
- Test: create manager and list it

## Slice 2. Risk Policy Defaults

- Add default risk policy on manager creation
- Defaults:
  - max_position_pct = 5
  - max_single_order_amount_krw = 1000000
  - max_daily_loss_pct = 2
  - order_type = limit only
- Test: new manager has default policy

## Slice 3. 5-Minute Schedule UI

- Generate slots from market open to close in 5-minute increments
- Columns: scenario, trade
- API: `GET/PATCH /api/managers/:id/schedule`
- Test: save 10 checked slots

## Slice 4. Paper Buy Order

- API: `POST /api/paper/orders`
- Use limit order and latest quote fixture
- Update cash ledger and position
- Test: insufficient cash rejected

## Slice 5. Holdings Tab

- API: `GET /api/managers/:id/holdings`
- Include cash row
- Include weight, avg price, current price, unrealized PnL
- UI: mobile table/card hybrid

## Slice 6. Trade History Tab

- API: `GET /api/managers/:id/trades?type=&from=&to=`
- Filters: buy, sell, hold/watch
- UI: list with date, type, symbol, quantity, price, realized PnL

## Slice 7. Scenario Mock

- Create fixture evidence cards
- Mock LLM returns `scenario_output.schema.json`
- Store scenario_run, scenario_items, analysis_report
- UI: scenario cards like screenshot

## Slice 8. Analysis Modal

- API: `GET /api/managers/:id/analysis-reports/:report_id`
- UI: symbol header, chart placeholder, analysis text, bullish/sideways/bearish cards, CTA button
- Test: modal opens from scenario item

## Slice 9. Telegram Notification

- Store Telegram connection token encrypted
- Send test message
- Send risk rejection and order filled message
- Online test behind feature flag

## Slice 10. KIS Domestic Quote Fixture

- Implement parser for domestic quote response
- Add fixture JSON
- Convert to normalized `QuoteSnapshot`
- Test: price parsed correctly

## Slice 11. KIS Overseas Quote Fixture

- Implement parser for overseas quote response
- Add fixture JSON
- Convert to normalized `QuoteSnapshot`
- Test: price parsed correctly

## Slice 12. OpenDART Disclosure Search

- Implement request builder for `/api/list.json`
- Add fixture JSON
- Convert to `DisclosureItem`
- Test: recent filings mapped to symbol

## Slice 13. SEC Submissions

- Implement CIK zero-padding
- Add fixture JSON
- Extract latest 10-K, 10-Q, 8-K
- Test: filings mapped to symbol

## Slice 14. Naver News Search

- Implement search request builder
- Add fixture JSON
- Convert to `NewsItem`
- Test: title/description HTML stripped and link preserved

## Slice 15. Evidence Builder

- Convert quote/news/disclosure/financial/community items to evidence cards
- Add importance and reliability defaults by source type
- Test: evidence refs are stable and deterministic
