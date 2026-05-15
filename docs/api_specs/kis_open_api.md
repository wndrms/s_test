# KIS Open API Implementation Spec

Source references:
- API portal: https://apiportal.koreainvestment.com/apiservice-summary
- Official sample repository: https://github.com/koreainvestment/open-trading-api
- Config sample: https://github.com/koreainvestment/open-trading-api/blob/main/kis_devlp.yaml

## Base URLs

```text
real REST:  https://openapi.koreainvestment.com:9443
paper REST: https://openapivts.koreainvestment.com:29443
real WS:    ws://ops.koreainvestment.com:21000
paper WS:   ws://ops.koreainvestment.com:31000
```

## Authentication

### REST access token

```text
POST /oauth2/tokenP
Content-Type: application/json
```

Request body:

```json
{
  "grant_type": "client_credentials",
  "appkey": "USER_APP_KEY",
  "appsecret": "USER_APP_SECRET"
}
```

Expected response fields used by adapter:

```json
{
  "access_token": "...",
  "access_token_token_expired": "YYYY-MM-DD HH:MM:SS",
  "token_type": "Bearer"
}
```

### WebSocket approval key

```text
POST /oauth2/Approval
```

Request body:

```json
{
  "grant_type": "client_credentials",
  "appkey": "USER_APP_KEY",
  "secretkey": "USER_APP_SECRET"
}
```

### Hash key

```text
POST /uapi/hashkey
```

The official sample says hashkey is currently not mandatory, but implement it for POST order calls as an optional safety feature.

## Common REST headers

```text
Content-Type: application/json
Accept: text/plain
charset: UTF-8
authorization: Bearer {access_token}
appkey: {app_key}
appsecret: {app_secret}
tr_id: {TR_ID}
custtype: P
tr_cont: "" | "N" | continuation value
hashkey: {HASH} for POST when enabled
```

## Common response

KIS responses are generally wrapped with:

```json
{
  "rt_cd": "0",
  "msg_cd": "...",
  "msg1": "...",
  "output": {},
  "output1": {},
  "output2": []
}
```

`rt_cd == "0"` means success in the official sample wrapper.

## Domestic stock endpoints needed for MVP

### KR current price

```text
GET /uapi/domestic-stock/v1/quotations/inquire-price
TR_ID: FHKST01010100
```

Query:

```text
FID_COND_MRKT_DIV_CODE=J   # KRX only, product decision
FID_INPUT_ISCD=005930
```

Use for:

```text
- quote snapshot
- auto-trade fresh price check
- UI current price
```

### KR current executions / tick information

```text
GET /uapi/domestic-stock/v1/quotations/inquire-ccnl
TR_ID: FHKST01010300
```

Query:

```text
FID_COND_MRKT_DIV_CODE=J
FID_INPUT_ISCD=005930
```

### KR daily/weekly/monthly/yearly bars

```text
GET /uapi/domestic-stock/v1/quotations/inquire-daily-itemchartprice
TR_ID: FHKST03010100
```

Query:

```text
FID_COND_MRKT_DIV_CODE=J
FID_INPUT_ISCD=005930
FID_INPUT_DATE_1=20240101
FID_INPUT_DATE_2=20240430
FID_PERIOD_DIV_CODE=D      # D/W/M/Y
FID_ORG_ADJ_PRC=0          # 0 adjusted, 1 original
```

### KR intraday minute bars

```text
GET /uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice
TR_ID: FHKST03010200
```

Query:

```text
FID_COND_MRKT_DIV_CODE=J
FID_INPUT_ISCD=005930
FID_INPUT_HOUR_1=093000
FID_PW_DATA_INCU_YN=Y
FID_ETC_CLS_CODE=
```

Notes:

```text
- Provides same-day minute data.
- Official sample notes that past-day minute data is not provided by this endpoint.
```

### KR holiday/open day check

```text
GET /uapi/domestic-stock/v1/quotations/chk-holiday
TR_ID: CTCA0903R
```

Query:

```text
BASS_DT=20260430
CTX_AREA_FK=
CTX_AREA_NK=
```

Use `opnd_yn` from output to decide if order is allowed. The official sample warns to call this lightly, preferably once per day.

### KR balance

```text
GET /uapi/domestic-stock/v1/trading/inquire-balance
TR_ID real: TTTC8434R
TR_ID demo: VTTC8434R
```

Query:

```text
CANO=account first 8 digits
ACNT_PRDT_CD=account last 2 digits
AFHR_FLPR_YN=N          # N only for KRX regular strategy
OFL_YN=
INQR_DVSN=01            # 01 loan-date, 02 symbol
UNPR_DVSN=01
FUND_STTL_ICLD_YN=N
FNCG_AMT_AUTO_RDPT_YN=N
PRCS_DVSN=00
CTX_AREA_FK100=
CTX_AREA_NK100=
```

Pagination:

```text
If response header tr_cont is M/F, call next with tr_cont=N and ctx_area fields.
```

### KR buying power

```text
GET /uapi/domestic-stock/v1/trading/inquire-psbl-order
TR_ID real: TTTC8908R
TR_ID demo: VTTC8908R
```

Query:

```text
CANO=...
ACNT_PRDT_CD=01
PDNO=005930
ORD_UNPR=70000
ORD_DVSN=00             # limit order, product decision
CMA_EVLU_AMT_ICLD_YN=N
OVRS_ICLD_YN=N
```

### KR cash order - limit only

```text
POST /uapi/domestic-stock/v1/trading/order-cash
TR_ID real sell: TTTC0011U
TR_ID real buy:  TTTC0012U
TR_ID demo sell: VTTC0011U
TR_ID demo buy:  VTTC0012U
```

Request body:

```json
{
  "CANO": "12345678",
  "ACNT_PRDT_CD": "01",
  "PDNO": "005930",
  "ORD_DVSN": "00",
  "ORD_QTY": "1",
  "ORD_UNPR": "70000",
  "EXCG_ID_DVSN_CD": "KRX",
  "SLL_TYPE": "",
  "CNDT_PRIC": ""
}
```

Policy:

```text
- ORD_DVSN must be 00.
- EXCG_ID_DVSN_CD must be KRX.
- Market orders, NXT, SOR are blocked by Risk Gate.
```

### KR order revise/cancel

```text
POST /uapi/domestic-stock/v1/trading/order-rvsecncl
TR_ID real: TTTC0013U
TR_ID demo: VTTC0013U
```

Body:

```json
{
  "CANO": "12345678",
  "ACNT_PRDT_CD": "01",
  "KRX_FWDG_ORD_ORGNO": "...",
  "ORGN_ODNO": "...",
  "ORD_DVSN": "00",
  "RVSE_CNCL_DVSN_CD": "02",
  "ORD_QTY": "1",
  "ORD_UNPR": "0",
  "QTY_ALL_ORD_YN": "Y",
  "EXCG_ID_DVSN_CD": "KRX"
}
```

### KR news/disclosure titles from KIS

```text
GET /uapi/domestic-stock/v1/quotations/news-title
TR_ID: FHKST01011800
```

Use as secondary news/disclosure-title source; OpenDART remains the official filing source.

### KR financial statement helper endpoints from KIS

Examples from official repo:

```text
GET /uapi/domestic-stock/v1/finance/income-statement   TR_ID FHKST66430200
GET /uapi/domestic-stock/v1/finance/balance-sheet      check official sample before implementation
GET /uapi/domestic-stock/v1/finance/financial-ratio    check official sample before implementation
```

OpenDART should be the primary source for financial statements. KIS finance endpoints are useful for quick UI enrichment.

## Overseas stock endpoints needed for MVP

### US current price

```text
GET /uapi/overseas-price/v1/quotations/price
TR_ID: HHDFS00000300
```

Query:

```text
AUTH=
EXCD=NAS     # examples use NAS for Nasdaq quote, NASD for market-wide account/order in some APIs
SYMB=AAPL
```

### US daily bars

```text
GET /uapi/overseas-price/v1/quotations/dailyprice
TR_ID: HHDFS76240000
```

Query:

```text
AUTH=
EXCD=NAS
SYMB=AAPL
GUBN=0       # see official docs for day/week/month mapping
BYMD=20260430
MODP=0
```

### US balance

```text
GET /uapi/overseas-stock/v1/trading/inquire-balance
TR_ID real: TTTS3012R
TR_ID demo: VTTS3012R
```

Query:

```text
CANO=account first 8 digits
ACNT_PRDT_CD=01
OVRS_EXCG_CD=NASD       # US all in real; official sample notes NASD/NAS/NYSE/AMEX distinctions
TR_CRCY_CD=USD
CTX_AREA_FK200=
CTX_AREA_NK200=
```

### US buying power

```text
GET /uapi/overseas-stock/v1/trading/inquire-psamount
TR_ID real: TTTS3007R
TR_ID demo: VTTS3007R
```

Query:

```text
CANO=...
ACNT_PRDT_CD=01
OVRS_EXCG_CD=NASD
OVRS_ORD_UNPR=145.00
ITEM_CD=AAPL
```

### US limit order

```text
POST /uapi/overseas-stock/v1/trading/order
```

US TR IDs from sample:

```text
real buy:  TTTT1002U
real sell: TTTT1006U
demo buy:  VTTT1002U
demo sell: VTTT1006U
```

Body:

```json
{
  "CANO": "12345678",
  "ACNT_PRDT_CD": "01",
  "OVRS_EXCG_CD": "NASD",
  "PDNO": "AAPL",
  "ORD_QTY": "1",
  "OVRS_ORD_UNPR": "145.00",
  "CTAC_TLNO": "",
  "MGCO_APTM_ODNO": "",
  "SLL_TYPE": "",
  "ORD_SVR_DVSN_CD": "0",
  "ORD_DVSN": "00"
}
```

Policy:

```text
- ORD_DVSN must be 00.
- Product decision blocks MOO/LOO/MOC/LOC even if supported.
- Only NASD/NAS/NYSE/AMEX should be enabled for US MVP.
```

### US revise/cancel

```text
POST /uapi/overseas-stock/v1/trading/order-rvsecncl
TR_ID real: TTTT1004U
TR_ID demo: VTTT1004U
```

### US order/fill history

```text
GET /uapi/overseas-stock/v1/trading/inquire-ccnl
TR_ID real: TTTS3035R
TR_ID demo: VTTS3035R
```

### US holiday/settlement-date check

```text
GET /uapi/overseas-stock/v1/quotations/countries-holiday
TR_ID: CTOS5011R
```

### US news titles from KIS

```text
GET /uapi/overseas-price/v1/quotations/news-title
TR_ID: HHPSTH60100C1
```

Query supports:

```text
INFO_GB
CLASS_CD
NATION_CD=US
EXCHANGE_CD
SYMB=AAPL
DATA_DT
DATA_TM
CTS
```

## Implementation notes

1. Save app key/secret encrypted per user.
2. Store token encrypted or regenerate; never expose token to frontend.
3. Mask all logs: appkey, appsecret, token, account number.
4. `KisClient` should take `KisEnvironment::{Real, Paper}`.
5. Always store raw request/response in encrypted or redacted audit logs for order-related calls.
6. Treat official KIS Excel/API portal as final authority if fields differ from this summary.
