# Schedule Design

## Product decision

Users can configure scenario and trade jobs in 5-minute slots, matching the captured schedule UI.

## Market sessions

```text
KRX regular session: 09:00-15:30 Asia/Seoul
US regular session: 09:30-16:00 America/New_York
```

Use exchange timezone, not server timezone. Server may run in Korea while US session follows Eastern Time and daylight saving time.

## Tables

```sql
CREATE TABLE manager_schedules (
    id UUID PRIMARY KEY,
    manager_id UUID NOT NULL REFERENCES managers(id),
    market TEXT NOT NULL CHECK (market IN ('KRX','US')),
    timezone TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE schedule_slots (
    id UUID PRIMARY KEY,
    schedule_id UUID NOT NULL REFERENCES manager_schedules(id),
    time_of_day TIME NOT NULL,
    run_scenario BOOLEAN NOT NULL DEFAULT false,
    run_trade BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(schedule_id, time_of_day)
);

CREATE TABLE schedule_runs (
    id UUID PRIMARY KEY,
    manager_id UUID NOT NULL REFERENCES managers(id),
    schedule_slot_id UUID NOT NULL REFERENCES schedule_slots(id),
    run_type TEXT NOT NULL CHECK (run_type IN ('scenario','trade')),
    scheduled_for TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (status IN ('pending','running','success','failed','skipped')),
    error_message TEXT,
    idempotency_key TEXT NOT NULL UNIQUE
);
```

## Slot generation

For KRX:

```text
09:00, 09:05, 09:10, ..., 15:30
```

For US:

```text
09:30, 09:35, ..., 16:00 in America/New_York
```

## Worker behavior

Every 30 seconds:

```text
1. Load active managers.
2. Resolve current exchange time.
3. Find matching 5-minute slot.
4. Create schedule_run with unique idempotency key.
5. If run_scenario: enqueue scenario job.
6. If run_trade: enqueue trade job.
```

## Scenario/trade separation

Recommended pattern:

```text
09:00 scenario
09:15 trade
10:00 scenario
10:15 trade
14:30 scenario
15:00 trade
```

If the user checks both scenario and trade at the same slot, execute scenario first, then trade only if scenario completed and risk gate passed.

## Trade job rule

A trade job must use either:

```text
- scenario generated at the same slot, or
- latest scenario generated within configurable max age, default 30 minutes
```

If no valid scenario exists, skip trade.
