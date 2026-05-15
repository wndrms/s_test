# Source Manifest for Offline Verification

This file lists the online sources used to prepare the offline implementation notes. Save or mirror these pages before entering a fully offline environment if possible.

## KIS Open API

- API portal summary: `https://apiportal.koreainvestment.com/apiservice-summary`
- Official sample repo: `https://github.com/koreainvestment/open-trading-api`
- Config sample: `https://github.com/koreainvestment/open-trading-api/blob/main/kis_devlp.yaml`
- Auth sample: `examples_user/auth/oauth2tokenP.py`
- Common auth wrapper: `examples_llm/kis_auth.py`
- Domestic order: `examples_user/domestic_stock/order_cash.py`
- Domestic balance: `examples_user/domestic_stock/inquire_balance.py`
- Domestic quote: `examples_user/domestic_stock/inquire_price.py`
- Domestic order revise/cancel: `examples_user/domestic_stock/order_rvsecncl.py`
- Domestic daily bars: `examples_user/domestic_stock/inquire_daily_itemchartprice.py`
- Domestic minute bars: `examples_user/domestic_stock/inquire_time_itemchartprice.py`
- Domestic holiday: `examples_user/domestic_stock/chk_holiday.py`
- Domestic news title: `examples_user/domestic_stock/news_title.py`
- Overseas order: `examples_user/overseas_stock/order.py`
- Overseas quote: `examples_user/overseas_stock/price.py`
- Overseas daily bars: `examples_user/overseas_stock/dailyprice.py`
- Overseas balance: `examples_user/overseas_stock/inquire_balance.py`
- Overseas fill history: `examples_user/overseas_stock/inquire_ccnl.py`
- Overseas revise/cancel: `examples_user/overseas_stock/order_rvsecncl.py`
- Overseas buying power: `examples_user/overseas_stock/inquire_psamount.py`
- Overseas holiday: `examples_user/overseas_stock/countries_holiday.py`
- Overseas news title: `examples_user/overseas_stock/news_title.py`

## OpenDART

- Main: `https://opendart.fss.or.kr/intro/main.do`
- Disclosure guide: `https://opendart.fss.or.kr/guide/main.do?apiGrpCd=DS001`
- Disclosure search: `https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS001&apiId=2019001`
- Financial guide: `https://opendart.fss.or.kr/guide/main.do?apiGrpCd=DS003`
- Single company key accounts: `https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS003&apiId=2019017`
- Full financial statements: `https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS003&apiId=2019020`
- Financial indicators: `https://opendart.fss.or.kr/guide/detail.do?apiGrpCd=DS003&apiId=2022001`

## SEC EDGAR

- API page: `https://www.sec.gov/search-filings/edgar-application-programming-interfaces`
- Developer resources: `https://www.sec.gov/developer`
- Fair access policy: `https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data`

## Naver Search API

- News search docs: `https://developers.naver.com/docs/serviceapi/search/news/news.md`
- Search API product overview: `https://developers.naver.com/products/service-api/search/search.md`

## Telegram

- Bot API: `https://core.telegram.org/bots/api`
- sendMessage method: `https://core.telegram.org/bots/api#sendmessage`

## LLM Providers

- OpenAI Structured Outputs: official OpenAI API documentation
- Anthropic tool use / structured output pattern: official Anthropic documentation
- Gemini structured output / response schema: official Google AI documentation
