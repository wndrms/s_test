# Naver News Search API Spec

Source reference:
- https://developers.naver.com/docs/serviceapi/search/news/news.md

## Role in this product

Naver News API is the first Korean news source for event detection. Store title/link/description/pubDate and generate summaries/evidence cards. Do not store full article bodies by default.

## Endpoint

```text
GET https://openapi.naver.com/v1/search/news.json
```

XML is also supported, but use JSON for this project.

## Headers

```text
X-Naver-Client-Id: {client_id}
X-Naver-Client-Secret: {client_secret}
```

Product decision: user registers their own Naver client id/secret. Store encrypted.

## Query params

```text
query    required, UTF-8 encoded search query
display  optional, default 10, max 100
start    optional, default 1, max 1000
sort     optional, sim or date
```

Recommended defaults:

```text
display=20
start=1
sort=date
```

## Response fields

JSON response includes:

```text
lastBuildDate
total
start
display
items[].title
items[].originallink
items[].link
items[].description
items[].pubDate
```

## Search query strategy

For each Korean symbol:

```text
{company_name}
{company_name} 실적
{company_name} 공시
{company_name} 목표가
{company_name} 외국인
{company_name} 수주
{company_name} 파업
{company_name} 리콜
{stock_code}
```

For Samsung Electronics example:

```text
삼성전자
삼성전자 HBM
삼성전자 실적
삼성전자 파업
005930
```

## Normalization

Strip HTML highlight tags from title/description:

```text
<b>...</b>
```

Normalize item into:

```json
{
  "source": "NAVER_NEWS",
  "title": "...",
  "summary": "description cleaned",
  "original_url": "originallink",
  "provider_url": "link",
  "published_at": "pubDate parsed",
  "fetched_at": "now",
  "language": "ko"
}
```

## Deduplication

Dedup key:

```text
hash(normalized_title + original_url)
```

If original_url is missing, use:

```text
hash(normalized_title + pubDate)
```

## Evidence card mapping

```text
source_type = news
source_name = NAVER_NEWS
title = cleaned title
summary = short LLM or rule-based summary
url = original_url if present else link
as_of = pubDate
reliability_score = 0.65 default
importance_score = based on keywords and recency
```

## Blocking policy

News data should not block trading by itself unless a high-risk keyword is detected and no official source has clarified it.

High-risk keywords:

```text
거래정지
상장폐지
횡령
배임
감사의견 거절
유상증자
전환사채
파산
부도
리콜
소송
압수수색
```

## Error handling

Naver docs list common errors:

```text
SE01 incorrect query
SE02 invalid display
SE03 invalid start
SE04 invalid sort
SE05 invalid search API
SE06 malformed encoding
SE99 system error
403 API permission not enabled for the app
```
