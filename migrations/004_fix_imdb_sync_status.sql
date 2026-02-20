-- Fix imdb_sync_status table to allow multiple sync records per dataset (sync history)
-- The original UNIQUE constraint on dataset_name prevented this

-- SQLite doesn't support DROP CONSTRAINT, so we need to recreate the table

-- Step 1: Create new table without UNIQUE constraint
CREATE TABLE IF NOT EXISTS imdb_sync_status_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dataset_name TEXT NOT NULL,             -- Removed UNIQUE to allow history
    file_date TEXT,
    rows_processed INTEGER DEFAULT 0,
    rows_inserted INTEGER DEFAULT 0,
    rows_updated INTEGER DEFAULT 0,
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    status TEXT NOT NULL DEFAULT 'running', -- running, completed, failed
    error_message TEXT
);

-- Step 2: Copy existing data
INSERT INTO imdb_sync_status_new
SELECT * FROM imdb_sync_status;

-- Step 3: Drop old table
DROP TABLE imdb_sync_status;

-- Step 4: Rename new table
ALTER TABLE imdb_sync_status_new RENAME TO imdb_sync_status;

-- Step 5: Add index for efficient lookups
CREATE INDEX idx_imdb_sync_status_dataset ON imdb_sync_status(dataset_name);
CREATE INDEX idx_imdb_sync_status_status ON imdb_sync_status(status);
CREATE INDEX idx_imdb_sync_status_completed ON imdb_sync_status(completed_at);
