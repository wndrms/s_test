-- 매매 사이클(round-trip) 추적
-- 한 종목을 신규 진입(보유 0 → 매수)부터 전량 청산(보유 0)까지 하나의 사이클로 추적한다.
-- trade_fill이 기록될 때마다 누적 수량/평균가/실현손익과 상태(open/closed)가 갱신된다.

CREATE TABLE trade_cycles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_id UUID NOT NULL REFERENCES managers(id),
    symbol_id UUID NOT NULL REFERENCES symbols(id),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','closed')),
    -- 현재 보유 수량 (open이면 > 0, closed면 0)
    open_quantity NUMERIC(24,8) NOT NULL DEFAULT 0,
    total_buy_quantity NUMERIC(24,8) NOT NULL DEFAULT 0,
    total_sell_quantity NUMERIC(24,8) NOT NULL DEFAULT 0,
    -- 매수 누적 금액 / 평균 진입가
    total_buy_amount NUMERIC(24,8) NOT NULL DEFAULT 0,
    avg_entry_price NUMERIC(24,8) NOT NULL DEFAULT 0,
    -- 매도 누적 금액 / 평균 청산가
    total_sell_amount NUMERIC(24,8) NOT NULL DEFAULT 0,
    avg_exit_price NUMERIC(24,8) NOT NULL DEFAULT 0,
    -- 수수료/세금 누적
    total_fee NUMERIC(24,8) NOT NULL DEFAULT 0,
    total_tax NUMERIC(24,8) NOT NULL DEFAULT 0,
    -- 청산된 수량에 대한 실현손익 (수수료/세금 반영)
    realized_pnl NUMERIC(24,8) NOT NULL DEFAULT 0,
    fill_count INTEGER NOT NULL DEFAULT 0,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 매니저+종목당 동시에 open 사이클은 최대 1개만 존재할 수 있다.
CREATE UNIQUE INDEX uq_trade_cycles_open_per_symbol
    ON trade_cycles(manager_id, symbol_id)
    WHERE status = 'open';

CREATE INDEX idx_trade_cycles_manager_id ON trade_cycles(manager_id);
CREATE INDEX idx_trade_cycles_symbol_id ON trade_cycles(symbol_id);
CREATE INDEX idx_trade_cycles_status ON trade_cycles(status);

-- trade_fill을 사이클에 연결
ALTER TABLE trade_fills
    ADD COLUMN trade_cycle_id UUID REFERENCES trade_cycles(id);

CREATE INDEX idx_trade_fills_trade_cycle_id ON trade_fills(trade_cycle_id);
