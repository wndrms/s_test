# SEC EDGAR API Spec

Source references:
- https://www.sec.gov/search-filings/edgar-application-programming-interfaces
- https://www.sec.gov/about/developer-resources
- https://data.sec.gov/

## Role in this product

SEC EDGAR is the primary official source for US disclosures and XBRL company facts.

Use SEC for:

```text
- CIK mapping
- recent filings: 10-K, 10-Q, 8-K, 20-F, 6-K
- company facts: revenue, net income, EPS, assets, liabilities, cash flow
- filing metadata and links
```

SEC data APIs are JSON REST endpoints under `data.sec.gov` and do not require API keys. Still, implement a proper User-Agent and fair-access throttling.

## Required headers

```text
User-Agent: {app_name} {contact_email}
Accept-Encoding: gzip, deflate
Host: data.sec.gov
```

## Endpoints

### Submissions by company

```text
GET https://data.sec.gov/submissions/CIK##########.json
```

`##########` is 10-digit zero-padded CIK.

Example:

```text
CIK0000320193.json for Apple Inc.
```

Use fields:

```text
name
cik
tickers
exchanges
filings.recent.accessionNumber
filings.recent.filingDate
filings.recent.reportDate
filings.recent.form
filings.recent.primaryDocument
```

### Company facts

```text
GET https://data.sec.gov/api/xbrl/companyfacts/CIK##########.json
```

Use for broad financial metric extraction.

Important namespaces:

```text
us-gaap
ifrs-full
dei
```

Common tags to normalize:

```text
Revenues
RevenueFromContractWithCustomerExcludingAssessedTax
SalesRevenueNet
OperatingIncomeLoss
NetIncomeLoss
EarningsPerShareDiluted
Assets
Liabilities
StockholdersEquity
NetCashProvidedByUsedInOperatingActivities
PaymentsToAcquirePropertyPlantAndEquipment
```

### Company concept

```text
GET https://data.sec.gov/api/xbrl/companyconcept/CIK##########/us-gaap/Revenues.json
```

Use when only one metric is needed.

### Frames

```text
GET https://data.sec.gov/api/xbrl/frames/us-gaap/Revenues/USD/CY2023.json
```

Use later for cross-company comparisons. Not required in MVP.

## Database mapping

```sql
CREATE TABLE sec_companies (
    cik TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    tickers TEXT[] NOT NULL DEFAULT '{}',
    exchanges TEXT[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sec_filings (
    id UUID PRIMARY KEY,
    symbol_id UUID REFERENCES symbols(id),
    cik TEXT NOT NULL,
    accession_number TEXT NOT NULL,
    form TEXT NOT NULL,
    filing_date DATE,
    report_date DATE,
    primary_document TEXT,
    document_url TEXT,
    summary TEXT,
    raw_payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(cik, accession_number)
);
```

## Filing document URL construction

For many SEC filing documents, you can construct an archive URL from CIK and accession number by removing dashes from accession number:

```text
https://www.sec.gov/Archives/edgar/data/{cik_without_leading_zeros}/{accession_no_without_dashes}/{primary_document}
```

Verify against `submissions` metadata before using in production.

## Collector strategy

Daily before US market open:

```text
1. Ensure ticker -> CIK mapping exists.
2. Fetch submissions for all US universe symbols.
3. Store recent 10-K, 10-Q, 8-K.
4. Fetch companyfacts for active holdings and watchlist.
```

Intraday:

```text
- Poll submissions for active holdings every 30-60 minutes.
- Convert new 8-K or 10-Q/10-K into evidence cards.
```

## Evidence scoring

```text
10-K: high importance, high reliability
10-Q: high importance, high reliability
8-K: high importance if item indicates material event
Form 4: optional later, insider trading signal
```

## Notes

- Keep all SEC times in UTC and convert to exchange timezone only for UI.
- Company facts may contain multiple possible revenue tags. Normalize with a tag preference list per sector when needed.
- Store raw JSON for companyfacts selectively because it can be large.
