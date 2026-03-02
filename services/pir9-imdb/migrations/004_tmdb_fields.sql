-- Add TMDB metadata cache columns to imdb_movies
ALTER TABLE imdb_movies ADD COLUMN tmdb_id BIGINT;
ALTER TABLE imdb_movies ADD COLUMN poster_url TEXT;
ALTER TABLE imdb_movies ADD COLUMN fanart_url TEXT;
ALTER TABLE imdb_movies ADD COLUMN tmdb_fetched_at TIMESTAMPTZ;

-- Partial index for efficient TMDB ID lookups
CREATE INDEX idx_imdb_movies_tmdb_id ON imdb_movies(tmdb_id) WHERE tmdb_id IS NOT NULL;
