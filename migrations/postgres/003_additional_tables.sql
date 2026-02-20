-- Additional tables for custom filters, remote path mappings, and tracked downloads

-- ============================================================================
-- Custom filters table
-- ============================================================================
CREATE TABLE IF NOT EXISTS custom_filters (
    id BIGSERIAL PRIMARY KEY,
    filter_type VARCHAR(255) NOT NULL DEFAULT 'series',
    label VARCHAR(255) NOT NULL,
    filters TEXT NOT NULL DEFAULT '[]'
);

-- ============================================================================
-- Remote path mappings table
-- ============================================================================
CREATE TABLE IF NOT EXISTS remote_path_mappings (
    id BIGSERIAL PRIMARY KEY,
    host VARCHAR(255) NOT NULL,
    remote_path VARCHAR(1024) NOT NULL,
    local_path VARCHAR(1024) NOT NULL
);

-- ============================================================================
-- Tracked downloads table (used by TrackedDownloadService)
-- ============================================================================
CREATE TABLE IF NOT EXISTS tracked_downloads (
    id BIGSERIAL PRIMARY KEY,
    download_id TEXT NOT NULL,
    download_client_id BIGINT NOT NULL REFERENCES download_clients(id) ON DELETE CASCADE,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    episode_ids TEXT NOT NULL DEFAULT '[]',
    title TEXT NOT NULL,
    indexer TEXT,
    size BIGINT NOT NULL DEFAULT 0,
    protocol INTEGER NOT NULL DEFAULT 0,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    status INTEGER NOT NULL DEFAULT 0,
    status_messages TEXT NOT NULL DEFAULT '[]',
    error_message TEXT,
    output_path TEXT,
    is_upgrade BOOLEAN NOT NULL DEFAULT false,
    added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(download_id, download_client_id)
);

CREATE INDEX IF NOT EXISTS idx_tracked_downloads_series ON tracked_downloads(series_id);
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_status ON tracked_downloads(status);
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_download_id ON tracked_downloads(download_id);
