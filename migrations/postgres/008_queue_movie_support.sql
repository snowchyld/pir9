-- Add movie support to queue/tracked downloads
-- Allows queue items to be matched to movies (not just series)

-- Drop series FK constraint (allows series_id=0 for movie downloads)
-- The code validates series existence before inserting, so referential
-- integrity is enforced at the application layer.
ALTER TABLE tracked_downloads DROP CONSTRAINT IF EXISTS tracked_downloads_series_id_fkey;

-- Add movie_id column for movie-matched downloads
ALTER TABLE tracked_downloads ADD COLUMN IF NOT EXISTS movie_id BIGINT REFERENCES movies(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_movie_id ON tracked_downloads(movie_id);
