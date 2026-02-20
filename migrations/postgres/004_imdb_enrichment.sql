-- IMDB enrichment columns for series and episodes
-- Adds IMDB ratings, vote counts, and episode-level IMDB IDs

-- Series: add IMDB rating and vote count
ALTER TABLE series ADD COLUMN IF NOT EXISTS imdb_rating REAL;
ALTER TABLE series ADD COLUMN IF NOT EXISTS imdb_votes INTEGER;

-- Index on imdb_id for fast lookups (column already exists from initial schema)
CREATE INDEX IF NOT EXISTS idx_series_imdb_id ON series(imdb_id);

-- Episodes: add IMDB ID, rating, and vote count
ALTER TABLE episodes ADD COLUMN IF NOT EXISTS imdb_id TEXT;
ALTER TABLE episodes ADD COLUMN IF NOT EXISTS imdb_rating REAL;
ALTER TABLE episodes ADD COLUMN IF NOT EXISTS imdb_votes INTEGER;

CREATE INDEX IF NOT EXISTS idx_episodes_imdb_id ON episodes(imdb_id);
