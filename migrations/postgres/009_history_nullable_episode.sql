-- Make history.episode_id nullable and add movie_id column
-- Movies don't have episodes, so episode_id must be nullable for movie history.
-- Also drop the strict FK so episode_id=0 or NULL doesn't violate constraints.

-- Drop the existing FK constraint on episode_id
ALTER TABLE history DROP CONSTRAINT IF EXISTS history_episode_id_fkey;

-- Allow NULL episode_id (for movie history entries)
ALTER TABLE history ALTER COLUMN episode_id DROP NOT NULL;

-- Drop the existing FK constraint on series_id (movies have series_id=0)
ALTER TABLE history DROP CONSTRAINT IF EXISTS history_series_id_fkey;

-- Allow NULL series_id
ALTER TABLE history ALTER COLUMN series_id DROP NOT NULL;

-- Add movie_id column for movie history entries
ALTER TABLE history ADD COLUMN IF NOT EXISTS movie_id BIGINT REFERENCES movies(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_history_movie_id ON history(movie_id);
