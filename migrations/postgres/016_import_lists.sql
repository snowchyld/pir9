-- Import lists table for external list integrations (IMDB watchlist, Trakt, etc.)
CREATE TABLE IF NOT EXISTS import_lists (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    list_type TEXT NOT NULL,             -- 'imdb_watchlist', 'imdb_list', 'trakt_watchlist', 'trakt_list'
    list_url TEXT,                        -- URL or list ID
    root_folder_path TEXT NOT NULL,
    quality_profile_id BIGINT NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT true,
    search_on_add BOOLEAN NOT NULL DEFAULT true,
    content_type TEXT NOT NULL DEFAULT 'movie',  -- 'movie' or 'series'
    sync_interval_hours INTEGER NOT NULL DEFAULT 24,
    last_synced_at TIMESTAMPTZ,
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Import list exclusions to track items that should not be re-added
CREATE TABLE IF NOT EXISTS import_list_exclusions (
    id BIGSERIAL PRIMARY KEY,
    import_list_id BIGINT NOT NULL REFERENCES import_lists(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,            -- IMDB ID (tt1234567) or other external ID
    title TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'movie',
    excluded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_import_list_exclusions_list_id ON import_list_exclusions(import_list_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_import_list_exclusions_unique ON import_list_exclusions(import_list_id, external_id);
