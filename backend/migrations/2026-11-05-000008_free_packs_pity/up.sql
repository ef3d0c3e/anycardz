-- When the player last claimed free packs; NULL: never, so they can now
ALTER TABLE users ADD COLUMN free_packs_at TIMESTAMPTZ,
    -- Packs opened in a row without a legendary, for the pity guarantee
    ADD COLUMN dry_packs INT NOT NULL DEFAULT 0;
