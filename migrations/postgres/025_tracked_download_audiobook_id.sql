-- Add audiobook_id to tracked_downloads for audiobook download matching
ALTER TABLE tracked_downloads ADD COLUMN IF NOT EXISTS audiobook_id BIGINT REFERENCES audiobooks(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_audiobook_id ON tracked_downloads(audiobook_id);
