-- Scratch tickets: how many were held at tickets_since, refilled from there
-- (see src/scratch.rs). New and existing players start with a full hand.
ALTER TABLE users
    ADD COLUMN tickets INT NOT NULL DEFAULT 3,
    ADD COLUMN tickets_since TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Three sparkles: the foil roulette is doubled until then
    ADD COLUMN roulette_boost_until TIMESTAMPTZ;
