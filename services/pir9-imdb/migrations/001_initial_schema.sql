-- IMDB Service Schema
-- This database is separate from the main pir9 database

-- TV Series from title.basics
CREATE TABLE IF NOT EXISTS imdb_series (
    imdb_id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    original_title TEXT,
    start_year INTEGER,
    end_year INTEGER,
    runtime_minutes INTEGER,
    genres TEXT,
    is_adult BOOLEAN NOT NULL DEFAULT FALSE,
    title_type TEXT NOT NULL DEFAULT 'tvSeries',
    rating DOUBLE PRECISION,
    votes BIGINT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Episodes from title.episode + title.basics
CREATE TABLE IF NOT EXISTS imdb_episodes (
    imdb_id BIGINT PRIMARY KEY,
    parent_imdb_id BIGINT NOT NULL REFERENCES imdb_series(imdb_id) ON DELETE CASCADE,
    season_number INTEGER,
    episode_number INTEGER,
    title TEXT,
    runtime_minutes INTEGER,
    rating DOUBLE PRECISION,
    votes BIGINT,
    air_date DATE,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sync status tracking
CREATE TABLE IF NOT EXISTS imdb_sync_status (
    id BIGSERIAL PRIMARY KEY,
    dataset_name TEXT NOT NULL,
    file_date TEXT,
    rows_processed BIGINT NOT NULL DEFAULT 0,
    rows_inserted BIGINT NOT NULL DEFAULT 0,
    rows_updated BIGINT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'running',
    error_message TEXT,
    last_processed_id BIGINT
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_imdb_series_title ON imdb_series(title);
CREATE INDEX IF NOT EXISTS idx_imdb_series_title_lower ON imdb_series(LOWER(title));
CREATE INDEX IF NOT EXISTS idx_imdb_series_votes ON imdb_series(votes DESC NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_imdb_series_rating ON imdb_series(rating DESC NULLS LAST);

CREATE INDEX IF NOT EXISTS idx_imdb_episodes_parent ON imdb_episodes(parent_imdb_id);
CREATE INDEX IF NOT EXISTS idx_imdb_episodes_season_ep ON imdb_episodes(parent_imdb_id, season_number, episode_number);
CREATE INDEX IF NOT EXISTS idx_imdb_episodes_air_date ON imdb_episodes(air_date);

CREATE INDEX IF NOT EXISTS idx_imdb_sync_status_dataset ON imdb_sync_status(dataset_name);
CREATE INDEX IF NOT EXISTS idx_imdb_sync_status_status ON imdb_sync_status(status);
