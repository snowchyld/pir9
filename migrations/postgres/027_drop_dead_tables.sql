-- Drop dead/legacy tables that are no longer used (ADR-006)
--
-- imdb_titles, imdb_ratings, imdb_episodes: Replaced by pir9-imdb service (external DB)
-- imdb_sync_status: Legacy singleton row, replaced by pir9-imdb service
-- download_queue: Replaced by tracked_downloads table
-- schema_migrations: Legacy migration tracker, SQLx uses _sqlx_migrations instead

DROP TABLE IF EXISTS imdb_ratings;    -- FK to imdb_titles, drop first
DROP TABLE IF EXISTS imdb_episodes;   -- FK to imdb_titles, drop first
DROP TABLE IF EXISTS imdb_titles;
DROP TABLE IF EXISTS imdb_sync_status;
DROP TABLE IF EXISTS download_queue;
DROP TABLE IF EXISTS schema_migrations;
