-- pg_trgm GIN indexes for ILIKE '%query%' search acceleration
-- Requires pg_trgm extension (created by superuser, not by migration)
-- Without these, ILIKE search on 1.2M movies is a full seq scan (~200ms)
-- With trgm: ~2ms via Bitmap Index Scan

CREATE INDEX IF NOT EXISTS idx_imdb_series_title_trgm ON imdb_series USING gin (title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_imdb_movies_title_trgm ON imdb_movies USING gin (title gin_trgm_ops);
