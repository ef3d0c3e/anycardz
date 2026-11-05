-- Earned once, kept forever: set completions (set:<slug>, foilset:<slug>) and
-- achievements. Losing cards later takes nothing back.
CREATE TABLE achievements (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    -- Coins paid when it was earned
    coins BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, key)
);
-- Leaderboards count achievements by kind and period
CREATE INDEX achievements_created_idx ON achievements (created_at);

-- What a notification is about when it is not a player or a trade
ALTER TABLE notifications ADD COLUMN detail TEXT;
