-- Add content_type discriminator to root_folders
-- Allows filtering root folders by content type (series, movie, anime, etc.)
-- Existing rows default to 'series' for backward compatibility

ALTER TABLE root_folders ADD COLUMN IF NOT EXISTS content_type TEXT NOT NULL DEFAULT 'series';

CREATE INDEX IF NOT EXISTS idx_root_folders_content_type ON root_folders(content_type)
  WHERE content_type != 'series';
