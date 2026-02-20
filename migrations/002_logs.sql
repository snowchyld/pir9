-- Logs table for storing application events
-- Used by System -> Events page

CREATE TABLE IF NOT EXISTS logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    time DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    level TEXT NOT NULL DEFAULT 'info',
    logger TEXT NOT NULL DEFAULT '',
    message TEXT NOT NULL,
    exception TEXT,
    exception_type TEXT
);

CREATE INDEX idx_logs_time ON logs(time DESC);
CREATE INDEX idx_logs_level ON logs(level);

-- Log files table for tracking log file references
CREATE TABLE IF NOT EXISTS log_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    filename TEXT NOT NULL UNIQUE,
    last_write_time DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
