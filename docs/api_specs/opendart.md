# OpenDART API Spec

Source references:
- https://opendart.fss.or.kr/intro/main.do
- https://opendart.fss.or.kr/guide/main.do?apiGrpCd=DS001
- https://opendart.fss.or.kr/guide/main.do?apiGrpCd=DS003

## Role in this product

OpenDART is the primary official source for Korean disclosures and financial statements.

Use OpenDART for:

```text
- corp_code mapping
- recent disclosures
- disclosure original XML/document
- single-company key accounts
- full financial statements
- financial ratios/indices
```

## Auth

All OpenDART API requests use:

```text
crtfc_key={user_or_service_opendart_api_key}
```

Product decision: all external API keys are user-registered. Store `crtfc_key` encrypted per user.

## Endpoints

### Corporate code file

```text
GET https://opendart.fss.or.kr/api/corpCode.xml
```

Query:

```text
crtfc_key=...
```

Response is a ZIP/XML file. Use it to map:

```text
stock_code -> corp_code
corp_code -> corp_name
```

Store in `symbol_identifiers`:

```text
id_type='DART_CORP_CODE'
id_value=corp_code
```

### Disclosure search

```text
GET https://opendart.fss.or.kr/api/list.json
```

Important query params:

```text
crtfc_key     required
corp_code     optional, 8 digits
bgn_de        optional YYYYMMDD
end_de        optional YYYYMMDD
last_reprt_at optional Y/N
pblntf_ty     optional A/B/C/D/E/F/G/H/I/J
pblntf_detail_ty optional
corp_cls      optional Y/K/N/E
sort          optional date/crp/rpt
sort_mth      optional asc/desc
page_no       optional
page_count    optional, max 100
```

Useful pblntf_ty:

```text
A: regular disclosure
B: material event report
C: securities issuance
D: equity ownership disclosure
E: other disclosure
I: exchange disclosure
```

Output fields to store:

```text
corp_cls
corp_name
corp_code
stock_code
report_nm
rcept_no
flr_nm
rcept_dt
rm
```

Viewer link:

```text
https://dart.fss.or.kr/dsaf001/main.do?rcpNo={rcept_no}
```

### Company overview

```text
GET https://opendart.fss.or.kr/api/company.json
```

Query:

```text
crtfc_key
corp_code
```

Use for enrichment only.

### Disclosure original file

```text
GET https://opendart.fss.or.kr/api/document.xml
```

Query:

```text
crtfc_key
rcept_no
```

Use only when you need to summarize a specific filing. Store extracted summary, not necessarily entire raw document in normal mode.

### Single-company key accounts

```text
GET https://opendart.fss.or.kr/api/fnlttSinglAcnt.json
```

Query:

```text
crtfc_key
corp_code
bsns_year      YYYY
reprt_code     11013 Q1, 11012 half, 11014 Q3, 11011 annual
```

Output includes:

```text
rcept_no
bsns_year
stock_code
reprt_code
account_nm
fs_div         OFS/CFS
sj_div         BS/IS
thstrm_amount
frmtrm_amount
currency
```

### Single-company full financial statements

```text
GET https://opendart.fss.or.kr/api/fnlttSinglAcntAll.json
```

Query:

```text
crtfc_key
corp_code
bsns_year
reprt_code
fs_div       OFS or CFS
```

Output includes statement division:

```text
BS  balance sheet
IS  income statement
CIS comprehensive income
CF  cash flow
SCE statement of changes in equity
```

### Single-company financial indicators

```text
GET https://opendart.fss.or.kr/api/fnlttSinglIndx.json
```

Query:

```text
crtfc_key
corp_code
bsns_year
reprt_code
idx_cl_code
```

`idx_cl_code`:

```text
M210000 profitability
M220000 stability
M230000 growth
M240000 activity
```

## Database mapping

```sql
CREATE TABLE dart_corp_codes (
    corp_code TEXT PRIMARY KEY,
    corp_name TEXT NOT NULL,
    stock_code TEXT,
    modify_date TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE disclosures (
    id UUID PRIMARY KEY,
    source TEXT NOT NULL DEFAULT 'OPENDART',
    symbol_id UUID REFERENCES symbols(id),
    external_id TEXT NOT NULL, -- rcept_no
    form_type TEXT,
    title TEXT NOT NULL,
    filed_at DATE,
    document_url TEXT,
    summary TEXT,
    event_type TEXT,
    importance_score NUMERIC(6,4),
    raw_payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(source, external_id)
);
```

## Collector strategy

Daily before market open:

```text
1. Refresh corpCode.xml if older than 1 day.
2. For each KR symbol in manager_universe, call list.json for last 7 days.
3. Create evidence cards for important filings.
```

During market hours:

```text
- Call list.json every 15-30 minutes for universe symbols.
- Do not call document.xml for every filing immediately.
- Only fetch document.xml for high-impact report_nm.
```

High-impact keyword examples:

```text
유상증자
무상증자
전환사채
신주인수권
합병
분할
영업정지
소송
횡령
배임
실적
잠정실적
주주총회
자기주식
```
