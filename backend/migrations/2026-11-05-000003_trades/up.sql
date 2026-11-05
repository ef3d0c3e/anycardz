-- Offers between two players. `version` grows with every edit, and a seal is
-- the version a player agreed to: an edit clears both seals.
CREATE TABLE trades (
    id BIGSERIAL PRIMARY KEY,
    -- Who opened the trade, and with whom
    from_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    to_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'done', 'cancelled')),
    version INTEGER NOT NULL DEFAULT 1,
    from_coins BIGINT NOT NULL DEFAULT 0 CHECK (from_coins >= 0),
    to_coins BIGINT NOT NULL DEFAULT 0 CHECK (to_coins >= 0),
    from_sealed INTEGER,
    to_sealed INTEGER,
    edited_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (from_id <> to_id)
);
CREATE INDEX trades_from_idx ON trades (from_id, state);
CREATE INDEX trades_to_idx ON trades (to_id, state);
-- At most one open trade per pair of players, whoever opened it
CREATE UNIQUE INDEX trades_open_pair ON trades (LEAST(from_id, to_id), GREATEST(from_id, to_id)) WHERE state = 'open';

-- Cards a side gives
CREATE TABLE trade_lines (
    id BIGSERIAL PRIMARY KEY,
    trade_id BIGINT NOT NULL REFERENCES trades (id) ON DELETE CASCADE,
    side TEXT NOT NULL CHECK (side IN ('from', 'to')),
    card TEXT NOT NULL,
    foil BOOLEAN NOT NULL DEFAULT FALSE,
    count INTEGER NOT NULL CHECK (count > 0)
);
CREATE INDEX trade_lines_trade_idx ON trade_lines (trade_id);
