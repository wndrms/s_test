# Fixtures

Place sanitized API response fixtures here.

Recommended structure:

```text
fixtures/
  kis/
    domestic_quote_005930.json
    domestic_balance.json
    domestic_order_cash_success.json
    overseas_quote_aapl.json
    overseas_balance.json
  opendart/
    list_005930.json
    fnltt_singl_acnt_005930_2023.json
  sec/
    submissions_aapl.json
    companyfacts_aapl.json
  naver/
    news_005930.json
  llm/
    scenario_output_005930.json
```

Rules:

- Remove or mask account numbers.
- Remove access tokens, app keys, app secrets, client IDs, client secrets.
- Keep enough fields for parsers and tests.
- Store both success and failure examples.
