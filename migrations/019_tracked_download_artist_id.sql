-- Add artist_id to tracked_downloads for music download matching
ALTER TABLE tracked_downloads ADD COLUMN artist_id INTEGER REFERENCES artists(id) ON DELETE SET NULL;
