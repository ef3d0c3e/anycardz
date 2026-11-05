DROP INDEX trades_done_idx;
DROP INDEX owned_cards_obtained_idx;
ALTER TABLE users DROP COLUMN roulette_streak, DROP COLUMN packs_opened;
