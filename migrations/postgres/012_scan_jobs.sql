-- Persistent scan job tracking
-- Stores scan job state in the database so jobs survive server restarts
-- and can be rehydrated into the in-memory consumer on startup.

CREATE TABLE IF NOT EXISTS scan_jobs (
    id              BIGSERIAL PRIMARY KEY,
    job_id          TEXT NOT NULL UNIQUE,
    scan_type       TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'queued',
    entity_ids      BIGINT[] NOT NULL DEFAULT '{}',
    worker_id       TEXT,
    progress_json   TEXT,
    message_json    TEXT NOT NULL,
    attempts        INT NOT NULL DEFAULT 0,
    max_attempts    INT NOT NULL DEFAULT 3,
    error           TEXT,
    download_id     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_heartbeat  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

-- Partial index for stale job queries (only running jobs)
CREATE INDEX IF NOT EXISTS idx_scan_jobs_stale ON scan_jobs(last_heartbeat) WHERE status = 'running';
-- Index for filtering by status (active jobs query)
CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status);
