-- Migration to sync PostgreSQL schema with SQLite schema
-- Fixes type mismatches and adds missing columns

-- ============================================================================
-- FIX download_clients TABLE
-- ============================================================================

-- Add missing columns
ALTER TABLE download_clients ADD COLUMN IF NOT EXISTS config_contract TEXT NOT NULL DEFAULT '';
ALTER TABLE download_clients ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';

-- Fix protocol column: TEXT -> INTEGER (only if it's currently TEXT)
DO $$
BEGIN
    -- Check if protocol column exists and is TEXT type
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'download_clients'
        AND column_name = 'protocol'
        AND data_type = 'text'
    ) THEN
        -- Add temporary integer column
        ALTER TABLE download_clients ADD COLUMN protocol_new INTEGER;

        -- Migrate data (torrent=0, usenet=1)
        UPDATE download_clients
        SET protocol_new = CASE
            WHEN protocol = 'usenet' THEN 1
            ELSE 0
        END;

        -- Drop old TEXT column and rename new one
        ALTER TABLE download_clients DROP COLUMN protocol;
        ALTER TABLE download_clients RENAME COLUMN protocol_new TO protocol;
        ALTER TABLE download_clients ALTER COLUMN protocol SET NOT NULL;
        ALTER TABLE download_clients ALTER COLUMN protocol SET DEFAULT 0;
    END IF;
END $$;

-- Create missing index
CREATE INDEX IF NOT EXISTS idx_download_clients_name ON download_clients(name);

-- ============================================================================
-- FIX indexers TABLE
-- ============================================================================

-- Add missing columns
ALTER TABLE indexers ADD COLUMN IF NOT EXISTS config_contract TEXT NOT NULL DEFAULT '';
ALTER TABLE indexers ADD COLUMN IF NOT EXISTS protocol INTEGER NOT NULL DEFAULT 0;
ALTER TABLE indexers ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';

-- Fix download_client_id: make NOT NULL with default 0, and ensure BIGINT type
UPDATE indexers SET download_client_id = 0 WHERE download_client_id IS NULL;
ALTER TABLE indexers ALTER COLUMN download_client_id TYPE BIGINT;
ALTER TABLE indexers ALTER COLUMN download_client_id SET DEFAULT 0;
ALTER TABLE indexers ALTER COLUMN download_client_id SET NOT NULL;

-- Create missing index
CREATE INDEX IF NOT EXISTS idx_indexers_name ON indexers(name);

-- ============================================================================
-- FIX notifications TABLE
-- ============================================================================

-- Add missing columns
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS config_contract TEXT NOT NULL DEFAULT '';
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS on_manual_interaction_required BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS include_health_warnings BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';

-- ============================================================================
-- FIX delay_profiles TABLE
-- ============================================================================

-- Fix preferred_protocol: TEXT -> INTEGER
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'delay_profiles'
        AND column_name = 'preferred_protocol'
        AND data_type = 'text'
    ) THEN
        ALTER TABLE delay_profiles ALTER COLUMN preferred_protocol DROP DEFAULT;
        ALTER TABLE delay_profiles ALTER COLUMN preferred_protocol TYPE INTEGER USING
            CASE WHEN preferred_protocol = 'usenet' THEN 1 ELSE 0 END;
        ALTER TABLE delay_profiles ALTER COLUMN preferred_protocol SET DEFAULT 0;
    END IF;
END $$;

-- Fix bypass_if_above_custom_format_score: BOOLEAN -> INTEGER
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'delay_profiles'
        AND column_name = 'bypass_if_above_custom_format_score'
        AND data_type = 'boolean'
    ) THEN
        ALTER TABLE delay_profiles ALTER COLUMN bypass_if_above_custom_format_score DROP DEFAULT;
        ALTER TABLE delay_profiles ALTER COLUMN bypass_if_above_custom_format_score TYPE INTEGER USING
            CASE WHEN bypass_if_above_custom_format_score THEN 1 ELSE 0 END;
        ALTER TABLE delay_profiles ALTER COLUMN bypass_if_above_custom_format_score SET DEFAULT 0;
    END IF;
END $$;

-- ============================================================================
-- FIX history TABLE
-- ============================================================================

-- Fix series_id and episode_id: INTEGER -> BIGINT
ALTER TABLE history ALTER COLUMN series_id TYPE BIGINT;
ALTER TABLE history ALTER COLUMN episode_id TYPE BIGINT;

-- Fix event_type: TEXT -> INTEGER
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'history'
        AND column_name = 'event_type'
        AND data_type = 'text'
    ) THEN
        ALTER TABLE history ALTER COLUMN event_type DROP DEFAULT;
        ALTER TABLE history ALTER COLUMN event_type TYPE INTEGER USING 0;
        ALTER TABLE history ALTER COLUMN event_type SET DEFAULT 0;
    END IF;
END $$;

-- Add missing columns to history
ALTER TABLE history ADD COLUMN IF NOT EXISTS languages TEXT NOT NULL DEFAULT '[]';
ALTER TABLE history ADD COLUMN IF NOT EXISTS custom_formats TEXT NOT NULL DEFAULT '[]';
ALTER TABLE history ADD COLUMN IF NOT EXISTS custom_format_score INTEGER NOT NULL DEFAULT 0;
ALTER TABLE history ADD COLUMN IF NOT EXISTS quality_cutoff_not_met BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE history ADD COLUMN IF NOT EXISTS download_id TEXT;

-- ============================================================================
-- ADD logs TABLE (from 002_logs.sql)
-- ============================================================================

CREATE TABLE IF NOT EXISTS logs (
    id BIGSERIAL PRIMARY KEY,
    time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    exception TEXT,
    exception_type TEXT,
    level TEXT NOT NULL,
    logger TEXT,
    message TEXT,
    method TEXT,
    url TEXT
);

CREATE INDEX IF NOT EXISTS idx_logs_time ON logs(time);
CREATE INDEX IF NOT EXISTS idx_logs_level ON logs(level);

-- ============================================================================
-- ADD IMDB data tables (from 003_imdb_data.sql)
-- ============================================================================

CREATE TABLE IF NOT EXISTS imdb_titles (
    tconst TEXT PRIMARY KEY,
    title_type TEXT NOT NULL,
    primary_title TEXT NOT NULL,
    original_title TEXT,
    is_adult BOOLEAN NOT NULL DEFAULT false,
    start_year INTEGER,
    end_year INTEGER,
    runtime_minutes INTEGER,
    genres TEXT
);

CREATE INDEX IF NOT EXISTS idx_imdb_titles_primary_title ON imdb_titles(primary_title);
CREATE INDEX IF NOT EXISTS idx_imdb_titles_title_type ON imdb_titles(title_type);

CREATE TABLE IF NOT EXISTS imdb_ratings (
    tconst TEXT PRIMARY KEY REFERENCES imdb_titles(tconst) ON DELETE CASCADE,
    average_rating REAL NOT NULL,
    num_votes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS imdb_episodes (
    tconst TEXT PRIMARY KEY,
    parent_tconst TEXT NOT NULL,
    season_number INTEGER,
    episode_number INTEGER,
    FOREIGN KEY (parent_tconst) REFERENCES imdb_titles(tconst) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_imdb_episodes_parent ON imdb_episodes(parent_tconst);
CREATE INDEX IF NOT EXISTS idx_imdb_episodes_season_episode ON imdb_episodes(parent_tconst, season_number, episode_number);

CREATE TABLE IF NOT EXISTS imdb_sync_status (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_sync TIMESTAMPTZ,
    titles_count INTEGER DEFAULT 0,
    ratings_count INTEGER DEFAULT 0,
    episodes_count INTEGER DEFAULT 0,
    sync_duration_secs INTEGER,
    last_error TEXT,
    is_syncing BOOLEAN DEFAULT false,
    last_processed_line BIGINT DEFAULT 0,
    current_file TEXT
);

INSERT INTO imdb_sync_status (id, titles_count, ratings_count, episodes_count)
VALUES (1, 0, 0, 0)
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- ADD download_queue TABLE (from 006_download_queue.sql)
-- ============================================================================

CREATE TABLE IF NOT EXISTS download_queue (
    id BIGSERIAL PRIMARY KEY,
    download_id TEXT NOT NULL,
    download_client_id BIGINT NOT NULL REFERENCES download_clients(id) ON DELETE CASCADE,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    episode_id BIGINT REFERENCES episodes(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    sizeleft BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued',
    added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    estimated_completion_time TIMESTAMPTZ,
    error_message TEXT,
    output_path TEXT,
    UNIQUE(download_id, download_client_id)
);

CREATE INDEX IF NOT EXISTS idx_download_queue_series ON download_queue(series_id);
CREATE INDEX IF NOT EXISTS idx_download_queue_status ON download_queue(status);
CREATE INDEX IF NOT EXISTS idx_download_queue_download_id ON download_queue(download_id);
