-- Movie enrichment fields cached from TMDB API
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS overview TEXT;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS studio TEXT;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS certification TEXT;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS in_cinemas DATE;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS physical_release DATE;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS digital_release DATE;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS logo_url TEXT;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS clearart_url TEXT;
ALTER TABLE imdb_movies ADD COLUMN IF NOT EXISTS banner_url TEXT;
