-- Import exclusion list: prevent specific movies/series from being automatically imported
CREATE TABLE IF NOT EXISTS import_exclusions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_id INTEGER,
    imdb_id TEXT,
    tvdb_id INTEGER,
    title TEXT NOT NULL,
    year INTEGER,
    content_type TEXT NOT NULL DEFAULT 'movie',
    added DATETIME NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_import_exclusions_tmdb ON import_exclusions(tmdb_id);
CREATE INDEX IF NOT EXISTS idx_import_exclusions_tvdb ON import_exclusions(tvdb_id);
CREATE INDEX IF NOT EXISTS idx_import_exclusions_imdb ON import_exclusions(imdb_id);
