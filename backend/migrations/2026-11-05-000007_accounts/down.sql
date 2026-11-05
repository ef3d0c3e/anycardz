ALTER TABLE sessions DROP COLUMN last_seen_at, DROP COLUMN user_agent;
ALTER TABLE users DROP COLUMN terms_accepted_at, DROP COLUMN terms_version;
