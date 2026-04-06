-- Add content_type column to tracked_downloads so the queue can distinguish
-- music, audiobook, and podcast downloads from series/movie downloads.
ALTER TABLE tracked_downloads ADD COLUMN IF NOT EXISTS content_type TEXT NOT NULL DEFAULT 'series';

-- Backfill existing rows: if movie_id is set, it's a movie download
UPDATE tracked_downloads SET content_type = 'movie' WHERE movie_id IS NOT NULL;

-- Backfill: series_id=0 and no movie_id means it was a music/audiobook grab
-- (series always have series_id > 0, movies always have movie_id set)
UPDATE tracked_downloads SET content_type = 'music' WHERE series_id = 0 AND movie_id IS NULL;
