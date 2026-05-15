-- Lumos initial schema
-- PostgreSQL 15+

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE,
    display_name TEXT,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE secret_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    provider TEXT NOT NULL,
    label TEXT NOT NULL,
    encrypted_payload BYTEA NOT NULL,
    masked_hint TEXT,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, provider, label)
);

CREATE TABLE broker_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    broker TEXT NOT NULL CHECK (broker IN ('kis')),
    environment TEXT NOT NULL CHECK (environment IN ('real','paper')),
    account_no_masked TEXT NOT NULL,
    account_no_encrypted BYTEA NOT NULL,
    app_key_secret_id UUID NOT NULL REFERENCES secret_keys(id),
    app_secret_secret_id UUID NOT NULL REFERENCES secret_keys(id),
    hts_id_encrypted BYTEA,
    access_token_encrypted BYTEA,
    token_expires_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE managers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    broker_connection_id UUID NOT NULL REFERENCES broker_connections(id),
    name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('paper','live')),
    region TEXT NOT NULL CHECK (region IN ('KR','US','GLOBAL')),
    base_currency TEXT NOT NULL CHECK (base_currency IN ('KRW','USD')),
    initial_capital NUMERIC(24,8) NOT NULL,
    auto_trade_enabled BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL CHECK (status IN ('active','paused','deleted')) DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE symbols (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    region TEXT NOT NULL CHECK (region IN ('KR','US')),
    market TEXT NOT NULL,
    code TEXT NOT NULL,
    display_code TEXT NOT NULL,
    name_ko TEXT,
    name_en TEXT,
    currency TEXT NOT NULL CHECK (currency IN ('KRW','USD')),
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(region, market, code)
);

CREATE TABLE symbol_identifiers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    id_type TEXT NOT NULL CHECK (id_type IN ('KIS_CODE','DART_CORP_CODE','ISIN','CIK','FIGI')),
    id_value TEXT NOT NULL,
    source TEXT NOT NULL,
    valid_from DATE,
    valid_to DATE,
    UNIQUE(id_type, id_value)
);

CREATE TABLE manager_universe (
    manager_id UUID NOT NULL REFERENCES managers(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(manager_id, symbol_id)
);

CREATE TABLE risk_policies (
    manager_id UUID PRIMARY KEY REFERENCES managers(id),
    max_position_pct NUMERIC(8,4) NOT NULL DEFAULT 5.0,
    max_single_order_amount_krw NUMERIC(24,4) NOT NULL DEFAULT 1000000,
    max_daily_loss_pct NUMERIC(8,4) NOT NULL DEFAULT 2.0,
    max_daily_trade_count INTEGER NOT NULL DEFAULT 20,
    allow_market_order BOOLEAN NOT NULL DEFAULT false,
    allow_pre_market BOOLEAN NOT NULL DEFAULT false,
    allow_after_hours BOOLEAN NOT NULL DEFAULT false,
    require_fresh_quote_seconds INTEGER NOT NULL DEFAULT 60,
    require_fresh_account_seconds INTEGER NOT NULL DEFAULT 60,
    min_ai_confidence_pct NUMERIC(8,4) NOT NULL DEFAULT 40.0,
    min_evidence_count INTEGER NOT NULL DEFAULT 2,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE manager_schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    market TEXT NOT NULL CHECK (market IN ('KRX','US')),
    timezone TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE schedule_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    schedule_id UUID NOT NULL REFERENCES manager_schedules(id),
    time_of_day TIME NOT NULL,
    run_scenario BOOLEAN NOT NULL DEFAULT false,
    run_trade BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(schedule_id, time_of_day)
);

CREATE TABLE schedule_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

CREATE TABLE quote_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    source TEXT NOT NULL,
    last_price NUMERIC(24,8) NOT NULL,
    bid NUMERIC(24,8),
    ask NUMERIC(24,8),
    volume NUMERIC(24,8),
    as_of TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE price_bars (
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    timeframe TEXT NOT NULL CHECK (timeframe IN ('1m','5m','1d','1w','1mo')),
    ts TIMESTAMPTZ NOT NULL,
    open NUMERIC(24,8) NOT NULL,
    high NUMERIC(24,8) NOT NULL,
    low NUMERIC(24,8) NOT NULL,
    close NUMERIC(24,8) NOT NULL,
    volume NUMERIC(24,8),
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(symbol_id, timeframe, ts)
);

CREATE TABLE portfolio_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    equity NUMERIC(24,8) NOT NULL,
    cash NUMERIC(24,8) NOT NULL,
    invested_value NUMERIC(24,8) NOT NULL,
    unrealized_pnl NUMERIC(24,8) NOT NULL,
    realized_pnl_day NUMERIC(24,8) NOT NULL,
    currency TEXT NOT NULL,
    as_of TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    quantity NUMERIC(24,8) NOT NULL,
    avg_price NUMERIC(24,8) NOT NULL,
    current_price NUMERIC(24,8),
    market_value NUMERIC(24,8),
    unrealized_pnl NUMERIC(24,8),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(manager_id, symbol_id)
);

CREATE TABLE cash_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    ledger_type TEXT NOT NULL CHECK (ledger_type IN ('deposit','withdraw','buy','sell','fee','tax','dividend','adjustment')),
    amount NUMERIC(24,8) NOT NULL,
    currency TEXT NOT NULL,
    ref_table TEXT,
    ref_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE evidence_cards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

CREATE TABLE scenario_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    schedule_slot_id UUID REFERENCES schedule_slots(id),
    model_provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    prompt_version TEXT,
    status TEXT NOT NULL CHECK (status IN ('generated','validated','rejected','executed','failed')),
    base_context_json JSONB NOT NULL DEFAULT '{}',
    raw_prompt_ref TEXT,
    raw_response_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE analysis_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    scenario_run_id UUID NOT NULL REFERENCES scenario_runs(id),
    base_price NUMERIC(24,8) NOT NULL,
    analyzed_at TIMESTAMPTZ NOT NULL,
    report_text TEXT NOT NULL,
    report_summary TEXT,
    data_freshness_level TEXT CHECK (data_freshness_level IN ('fresh','stale','blocking')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE analysis_report_evidence (
    report_id UUID NOT NULL REFERENCES analysis_reports(id),
    evidence_card_id UUID NOT NULL REFERENCES evidence_cards(id),
    PRIMARY KEY(report_id, evidence_card_id)
);

CREATE TABLE scenario_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_run_id UUID NOT NULL REFERENCES scenario_runs(id),
    analysis_report_id UUID REFERENCES analysis_reports(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    scenario_type TEXT NOT NULL CHECK (scenario_type IN ('bullish','sideways','bearish')),
    action TEXT NOT NULL CHECK (action IN ('buy','sell','hold','watch')),
    probability_pct NUMERIC(8,4) NOT NULL,
    target_price NUMERIC(24,8),
    stop_loss_price NUMERIC(24,8),
    condition_text TEXT NOT NULL,
    strategy_text TEXT NOT NULL,
    risk_text TEXT,
    rank_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE chart_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_report_id UUID NOT NULL REFERENCES analysis_reports(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    annotation_type TEXT NOT NULL CHECK (annotation_type IN ('target','support','resistance','stop_loss','entry')),
    price NUMERIC(24,8) NOT NULL,
    label TEXT NOT NULL,
    color_hint TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE order_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    scenario_run_id UUID REFERENCES scenario_runs(id),
    scenario_item_id UUID REFERENCES scenario_items(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side TEXT NOT NULL CHECK (side IN ('buy','sell')),
    order_type TEXT NOT NULL CHECK (order_type IN ('limit')),
    quantity NUMERIC(24,8) NOT NULL,
    limit_price NUMERIC(24,8) NOT NULL,
    estimated_amount NUMERIC(24,8) NOT NULL,
    ai_reason TEXT,
    risk_status TEXT NOT NULL CHECK (risk_status IN ('pending','approved','rejected')),
    risk_reject_reason TEXT,
    auto_execution BOOLEAN NOT NULL DEFAULT true,
    idempotency_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE broker_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_plan_id UUID NOT NULL REFERENCES order_plans(id),
    broker_connection_id UUID NOT NULL REFERENCES broker_connections(id),
    external_order_id TEXT,
    external_org_no TEXT,
    status TEXT NOT NULL CHECK (status IN ('submitted','filled','partial','canceled','rejected','unknown')),
    submitted_at TIMESTAMPTZ,
    synced_at TIMESTAMPTZ,
    raw_response_json JSONB
);

CREATE TABLE trade_fills (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    broker_order_id UUID NOT NULL REFERENCES broker_orders(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    side TEXT NOT NULL CHECK (side IN ('buy','sell')),
    quantity NUMERIC(24,8) NOT NULL,
    price NUMERIC(24,8) NOT NULL,
    fee NUMERIC(24,8) NOT NULL DEFAULT 0,
    tax NUMERIC(24,8) NOT NULL DEFAULT 0,
    filled_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    manager_id UUID REFERENCES managers(id),
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id UUID,
    payload_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for common queries
CREATE INDEX idx_managers_user_id ON managers(user_id);
CREATE INDEX idx_managers_status ON managers(status);
CREATE INDEX idx_secret_keys_user_id ON secret_keys(user_id);
CREATE INDEX idx_evidence_cards_symbol_id ON evidence_cards(symbol_id);
CREATE INDEX idx_evidence_cards_as_of ON evidence_cards(as_of DESC);
CREATE INDEX idx_quote_snapshots_symbol_as_of ON quote_snapshots(symbol_id, as_of DESC);
CREATE INDEX idx_scenario_runs_manager_id ON scenario_runs(manager_id);
CREATE INDEX idx_order_plans_manager_id ON order_plans(manager_id);
CREATE INDEX idx_broker_orders_order_plan_id ON broker_orders(order_plan_id);
CREATE INDEX idx_trade_fills_broker_order_id ON trade_fills(broker_order_id);
CREATE INDEX idx_audit_logs_manager_id ON audit_logs(manager_id);
CREATE INDEX idx_schedule_runs_idempotency ON schedule_runs(idempotency_key);
