-- Which version of the terms each player accepted, and when. NULL: none yet
-- (accounts made before terms existed), asked on their next visit.
ALTER TABLE users ADD COLUMN terms_version INT, ADD COLUMN terms_accepted_at TIMESTAMPTZ;

-- Shown in settings so players can tell their devices apart and sign them out
ALTER TABLE sessions ADD COLUMN user_agent TEXT NOT NULL DEFAULT '',
    ADD COLUMN last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now();
