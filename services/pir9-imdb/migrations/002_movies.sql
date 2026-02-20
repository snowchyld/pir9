-- Movies table from title.basics (titleType = "movie")
CREATE TABLE IF NOT EXISTS imdb_movies (
    imdb_id BIGINT PRIMARY KEY,
    title TEXT NOT NULL,
    original_title TEXT,
    year INTEGER,
    runtime_minutes INTEGER,
    genres TEXT,
    is_adult BOOLEAN NOT NULL DEFAULT FALSE,
    rating DOUBLE PRECISION,
    votes BIGINT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_imdb_movies_title ON imdb_movies(title);
CREATE INDEX IF NOT EXISTS idx_imdb_movies_title_lower ON imdb_movies(LOWER(title));
CREATE INDEX IF NOT EXISTS idx_imdb_movies_votes ON imdb_movies(votes DESC NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_imdb_movies_rating ON imdb_movies(rating DESC NULLS LAST);
CREATE INDEX IF NOT EXISTS idx_imdb_movies_year ON imdb_movies(year);
