CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    coins BIGINT NOT NULL DEFAULT 1000 CHECK (coins >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Names and addresses are unique whatever their case
CREATE UNIQUE INDEX users_username_key ON users (lower(username));
CREATE UNIQUE INDEX users_email_key ON users (lower(email));

CREATE TABLE sessions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- SHA-256 of the cookie token, the token itself is never stored
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);

-- One row per copy. `card` is the card's folder name in the data repository.
CREATE TABLE owned_cards (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    card TEXT NOT NULL,
    -- Foil pattern (foil1..foil6), NULL for a plain copy
    foil TEXT,
    obtained_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX owned_cards_user_card_idx ON owned_cards (user_id, card);

-- Unopened boosters: the pile
CREATE TABLE boosters (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    collection TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX boosters_user_id_idx ON boosters (user_id, id);

CREATE TABLE streaks (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- Days in a row, as of last_claim
    day INTEGER NOT NULL DEFAULT 0,
    best INTEGER NOT NULL DEFAULT 0,
    last_claim DATE
);
