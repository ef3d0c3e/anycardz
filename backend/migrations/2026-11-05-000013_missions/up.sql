-- The day's missions per player, with how far they got. The row keyed
-- '*bonus' marks the all-three bonus as given.
CREATE TABLE daily_missions (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day DATE NOT NULL,
    key TEXT NOT NULL,
    progress INT NOT NULL DEFAULT 0,
    done_at TIMESTAMPTZ,
    PRIMARY KEY (user_id, day, key)
);
