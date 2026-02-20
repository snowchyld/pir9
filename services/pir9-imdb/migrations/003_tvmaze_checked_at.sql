-- Add a dedicated column for tracking TVMaze lookup attempts.
-- Separate from last_synced_at which is set during IMDB sync.
ALTER TABLE imdb_episodes ADD COLUMN IF NOT EXISTS tvmaze_checked_at TIMESTAMPTZ;
