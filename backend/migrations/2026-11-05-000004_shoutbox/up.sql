-- Public chat. A message can show off a card (a "brag"): `card` and its foil pattern.
CREATE TABLE shout_messages (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    card TEXT,
    foil TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX shout_messages_created_idx ON shout_messages (created_at DESC);
CREATE INDEX shout_messages_user_idx ON shout_messages (user_id, created_at DESC);
