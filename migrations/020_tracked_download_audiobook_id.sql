-- Add audiobook_id to tracked_downloads for audiobook download matching
ALTER TABLE tracked_downloads ADD COLUMN audiobook_id INTEGER REFERENCES audiobooks(id) ON DELETE SET NULL;
