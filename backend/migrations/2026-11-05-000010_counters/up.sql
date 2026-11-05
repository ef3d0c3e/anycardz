-- Counters for achievements: packs opened since now (older ones were not
-- counted), and foil roulette wins in a row
ALTER TABLE users ADD COLUMN packs_opened INT NOT NULL DEFAULT 0,
    ADD COLUMN roulette_streak INT NOT NULL DEFAULT 0;
-- Leaderboards by period
CREATE INDEX owned_cards_obtained_idx ON owned_cards (obtained_at);
CREATE INDEX trades_done_idx ON trades (updated_at) WHERE state = 'done';
