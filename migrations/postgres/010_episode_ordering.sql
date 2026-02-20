-- Add episode ordering support to series
-- Allows per-series selection of TVDB episode ordering (aired, dvd, absolute, etc.)
ALTER TABLE series ADD COLUMN IF NOT EXISTS episode_ordering TEXT NOT NULL DEFAULT 'aired';

-- Partial index: only index non-default orderings (most series use 'aired')
CREATE INDEX IF NOT EXISTS idx_series_episode_ordering ON series(episode_ordering) WHERE episode_ordering != 'aired';
