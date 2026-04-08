-- Fix TIMESTAMP columns that should be TIMESTAMPTZ (ADR-006)
-- SQLx DateTime<Utc> requires TIMESTAMPTZ; TIMESTAMP causes runtime decode errors

ALTER TABLE blocklist
    ALTER COLUMN date TYPE TIMESTAMPTZ USING date AT TIME ZONE 'UTC';

ALTER TABLE scheduled_tasks
    ALTER COLUMN last_execution TYPE TIMESTAMPTZ USING last_execution AT TIME ZONE 'UTC',
    ALTER COLUMN last_start_time TYPE TIMESTAMPTZ USING last_start_time AT TIME ZONE 'UTC';
