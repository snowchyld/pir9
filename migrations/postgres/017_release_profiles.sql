-- Release profiles table for preferred words, must-contain, and must-not-contain rules
CREATE TABLE IF NOT EXISTS release_profiles (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    enabled BOOLEAN NOT NULL DEFAULT true,
    required TEXT NOT NULL DEFAULT '[]',
    ignored TEXT NOT NULL DEFAULT '[]',
    preferred TEXT NOT NULL DEFAULT '[]',
    indexer_id BIGINT NOT NULL DEFAULT 0,
    tags TEXT NOT NULL DEFAULT '[]'
);
