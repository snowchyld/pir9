-- Add resumability support to IMDB sync
-- Tracks last processed ID to allow resuming interrupted syncs

ALTER TABLE imdb_sync_status ADD COLUMN last_processed_id BIGINT;

-- Index for finding resumable syncs
CREATE INDEX idx_imdb_sync_status_resumable
    ON imdb_sync_status(dataset_name, status, last_processed_id);
