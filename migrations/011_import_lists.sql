-- Import lists table for external list integrations (IMDB watchlist, Trakt, etc.)
CREATE TABLE IF NOT EXISTS import_lists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    list_type TEXT NOT NULL,
    list_url TEXT,
    root_folder_path TEXT NOT NULL,
    quality_profile_id INTEGER NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    search_on_add BOOLEAN NOT NULL DEFAULT 1,
    content_type TEXT NOT NULL DEFAULT 'movie',
    sync_interval_hours INTEGER NOT NULL DEFAULT 24,
    last_synced_at TEXT,
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Import list exclusions to track items that should not be re-added
CREATE TABLE IF NOT EXISTS import_list_exclusions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    import_list_id INTEGER NOT NULL REFERENCES import_lists(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'movie',
    excluded_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_import_list_exclusions_list_id ON import_list_exclusions(import_list_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_import_list_exclusions_unique ON import_list_exclusions(import_list_id, external_id);
