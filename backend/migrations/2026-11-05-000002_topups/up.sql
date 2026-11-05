-- Coin top-ups. No payment is taken yet: rows record what was granted, so a
-- payment provider can later fill in the reference and the status.
CREATE TABLE topups (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    pack TEXT NOT NULL,
    coins BIGINT NOT NULL CHECK (coins > 0),
    price_cents INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX topups_user_id_idx ON topups (user_id, created_at);
