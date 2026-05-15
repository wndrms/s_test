# Data Ingestion Design

## Goal

Do not pass raw news/community/disclosure data directly to the LLM. Normalize all external facts into `evidence_cards` first.

## Pipeline

```text
Collector -> Raw payload -> Normalizer -> Deduplicator -> Symbol mapper -> Scorer -> Evidence card -> AI context builder
```

## Source policy

```text
KIS
- account, orders, fills, quotes, KRX/US prices
- source of truth for automatic trading

OpenDART
- Korean disclosures and financial statements
- official corporate data

SEC EDGAR
- US submissions and XBRL company facts
- official corporate data

Naver News API
- Korean news search result title/link/description/pubDate
- store summary/link, not full article body

KIS news endpoints
- Korean market/news titles and US news titles

Community/social
- weak sentiment signal only
- store summary/link/statistics, not full raw text if avoidable
```

## Evidence card model

```sql
CREATE TABLE evidence_cards (
    id UUID PRIMARY KEY,
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    source_type TEXT NOT NULL CHECK (source_type IN ('price','technical','news','disclosure','financial','community')),
    source_name TEXT NOT NULL,
    source_ref_table TEXT,
    source_ref_id UUID,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    url TEXT,
    sentiment_label TEXT CHECK (sentiment_label IN ('positive','neutral','negative','mixed')),
    importance_score NUMERIC(6,4) NOT NULL DEFAULT 0,
    reliability_score NUMERIC(6,4) NOT NULL DEFAULT 0,
    as_of TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

## Scoring guide

```text
reliability_score
- KIS quote/account: 0.95
- OpenDART/SEC: 0.90
- Naver/KIS news title: 0.65
- Community/social: 0.35

importance_score
- price breakout / risk event / earnings / major disclosure: high
- general commentary / old news / duplicate posts: low
```

## LLM context budget

For each symbol in one scenario run:

```text
- price/technical cards: up to 5
- disclosure cards: up to 3
- financial cards: up to 3
- news cards: up to 5
- community cards: up to 3
```

Always include `as_of` and `source_type` so the LLM can reason about stale data.

## Storage rule

For news/community:

```text
Store:
- title
- url
- publisher/source
- published_at/posted_at
- snippet/description
- summary
- sentiment
- importance

Avoid by default:
- full article body
- full community post body
- personal identifiers beyond hashed author id
```
