-- Add order column to delay_profiles
ALTER TABLE delay_profiles ADD COLUMN "order" INTEGER NOT NULL DEFAULT 0;

-- Pending releases table — stores releases that are being delayed before grabbing
CREATE TABLE IF NOT EXISTS pending_releases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    series_id INTEGER,
    episode_ids TEXT NOT NULL DEFAULT '[]',
    movie_id INTEGER,
    release_json TEXT NOT NULL,
    protocol INTEGER NOT NULL DEFAULT 0,
    quality_weight INTEGER NOT NULL DEFAULT 0,
    found_at TEXT NOT NULL,
    delay_until TEXT NOT NULL,
    processed INTEGER NOT NULL DEFAULT 0,
    added TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_pending_releases_delay_until
    ON pending_releases (delay_until)
    WHERE processed = 0;
