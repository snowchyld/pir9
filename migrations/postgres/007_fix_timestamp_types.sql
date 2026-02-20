-- Fix TIMESTAMP columns that should be TIMESTAMPTZ
-- The Rust models use DateTime<Utc> which SQLx maps to TIMESTAMPTZ.
-- These columns were created as TIMESTAMP (without timezone) in migration 001
-- but need to match the Rust type for proper deserialization.

ALTER TABLE episode_files ALTER COLUMN date_added TYPE TIMESTAMPTZ;
