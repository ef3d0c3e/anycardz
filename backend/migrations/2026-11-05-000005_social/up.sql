ALTER TABLE users
    ADD COLUMN bio TEXT NOT NULL DEFAULT '',
    -- 256 px JPEG, made from the upload
    ADD COLUMN avatar BYTEA,
    ADD COLUMN avatar_version INTEGER NOT NULL DEFAULT 0;

-- A friend request, then a friendship once accepted. One row per pair:
-- `requester_id` is whoever asked first.
CREATE TABLE friendships (
    requester_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    addressee_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    accepted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (requester_id, addressee_id),
    CHECK (requester_id <> addressee_id)
);
CREATE UNIQUE INDEX friendships_pair ON friendships (LEAST(requester_id, addressee_id), GREATEST(requester_id, addressee_id));
CREATE INDEX friendships_addressee_idx ON friendships (addressee_id);

CREATE TABLE blocks (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    blocked_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, blocked_id),
    CHECK (user_id <> blocked_id)
);
CREATE INDEX blocks_blocked_idx ON blocks (blocked_id);
