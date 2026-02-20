-- Migration: Download Queue Tracking
-- Tracks downloads sent to clients with their series/episode relationships

CREATE TABLE IF NOT EXISTS tracked_downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id TEXT NOT NULL,              -- ID from download client (hash/nzo_id)
    download_client_id INTEGER NOT NULL,    -- FK to download_clients
    series_id INTEGER NOT NULL,             -- FK to series
    episode_ids TEXT NOT NULL DEFAULT '[]', -- JSON array of episode IDs
    title TEXT NOT NULL,                    -- Release title
    indexer TEXT,                           -- Indexer name
    size INTEGER NOT NULL DEFAULT 0,        -- Size in bytes
    protocol INTEGER NOT NULL DEFAULT 0,    -- 1=Usenet, 2=Torrent
    quality TEXT NOT NULL DEFAULT '{}',     -- JSON QualityModel
    languages TEXT NOT NULL DEFAULT '[]',   -- JSON array of languages
    status INTEGER NOT NULL DEFAULT 0,      -- TrackedDownloadState enum
    status_messages TEXT NOT NULL DEFAULT '[]', -- JSON array of StatusMessage
    error_message TEXT,                     -- Error message if failed
    output_path TEXT,                       -- Download output path
    is_upgrade BOOLEAN NOT NULL DEFAULT 0,  -- Whether this is an upgrade
    added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    FOREIGN KEY (download_client_id) REFERENCES download_clients(id) ON DELETE CASCADE
);

-- Index for looking up by download client's ID
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_download_id ON tracked_downloads(download_id);

-- Index for looking up by series
CREATE INDEX IF NOT EXISTS idx_tracked_downloads_series_id ON tracked_downloads(series_id);

-- Unique constraint: same download can't be tracked twice for same client
CREATE UNIQUE INDEX IF NOT EXISTS idx_tracked_downloads_client_download
    ON tracked_downloads(download_client_id, download_id);
