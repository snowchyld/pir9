-- Add order column to delay_profiles
ALTER TABLE delay_profiles ADD COLUMN IF NOT EXISTS "order" INTEGER NOT NULL DEFAULT 0;

-- Pending releases table — stores releases that are being delayed before grabbing
CREATE TABLE IF NOT EXISTS pending_releases (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    series_id BIGINT,                    -- FK to series (NULL for movies)
    episode_ids TEXT NOT NULL DEFAULT '[]', -- JSON array of episode IDs
    movie_id BIGINT,                     -- FK to movies (NULL for series)
    release_json TEXT NOT NULL,           -- Full serialized ReleaseInfo
    protocol INTEGER NOT NULL DEFAULT 0, -- 1=usenet, 2=torrent
    quality_weight INTEGER NOT NULL DEFAULT 0, -- Quality weight for comparison
    found_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delay_until TIMESTAMPTZ NOT NULL,
    processed BOOLEAN NOT NULL DEFAULT false,
    added TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for the scheduler job that checks expired delays
CREATE INDEX IF NOT EXISTS idx_pending_releases_delay_until
    ON pending_releases (delay_until)
    WHERE processed = false;

-- Index for finding pending releases for a specific series/episode
CREATE INDEX IF NOT EXISTS idx_pending_releases_series
    ON pending_releases (series_id)
    WHERE processed = false;

-- Index for finding pending releases for a specific movie
CREATE INDEX IF NOT EXISTS idx_pending_releases_movie
    ON pending_releases (movie_id)
    WHERE processed = false;
