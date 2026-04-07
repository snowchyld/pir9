-- Add artist_id to tracked_downloads for music download matching
ALTER TABLE tracked_downloads ADD COLUMN IF NOT EXISTS artist_id BIGINT REFERENCES artists(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_artist_id ON tracked_downloads(artist_id);
