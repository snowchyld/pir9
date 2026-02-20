-- IMDB Non-Commercial Dataset Tables
-- These tables store data synced from https://datasets.imdbws.com/
-- Updated daily/weekly via the IMDB sync service

-- IMDB Series (TV Shows)
-- Source: title.basics.tsv.gz (filtered to tvSeries, tvMiniSeries)
CREATE TABLE IF NOT EXISTS imdb_series (
    imdb_id BIGINT PRIMARY KEY,           -- Numeric part only (e.g., 10234724 from tt10234724)
    title TEXT NOT NULL,                   -- primaryTitle
    original_title TEXT,                   -- originalTitle
    start_year INTEGER,                    -- startYear
    end_year INTEGER,                      -- endYear (NULL if ongoing)
    runtime_minutes INTEGER,               -- runtimeMinutes
    genres TEXT,                           -- Comma-separated genres (e.g., "Drama,Thriller")
    is_adult BOOLEAN NOT NULL DEFAULT 0,   -- isAdult
    title_type TEXT NOT NULL,              -- tvSeries, tvMiniSeries, etc.
    rating REAL,                           -- From title.ratings.tsv.gz
    votes INTEGER,                         -- From title.ratings.tsv.gz
    last_synced_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_imdb_series_title ON imdb_series(title);
CREATE INDEX idx_imdb_series_start_year ON imdb_series(start_year);
CREATE INDEX idx_imdb_series_rating ON imdb_series(rating);

-- IMDB Episodes
-- Source: title.episode.tsv.gz joined with title.basics.tsv.gz for episode titles
CREATE TABLE IF NOT EXISTS imdb_episodes (
    imdb_id BIGINT PRIMARY KEY,            -- Episode's own tconst (numeric)
    parent_imdb_id BIGINT NOT NULL,        -- Parent series tconst (numeric)
    season_number INTEGER,                  -- seasonNumber (can be NULL for specials)
    episode_number INTEGER,                 -- episodeNumber (can be NULL)
    title TEXT,                             -- Episode title from title.basics
    runtime_minutes INTEGER,                -- Episode runtime
    rating REAL,                            -- From title.ratings.tsv.gz
    votes INTEGER,                          -- From title.ratings.tsv.gz
    -- Note: air_date is NOT in IMDB data, will come from supplemental sources
    air_date DATE,                          -- Populated from TMDB/TVMaze
    last_synced_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_imdb_id) REFERENCES imdb_series(imdb_id) ON DELETE CASCADE
);

CREATE INDEX idx_imdb_episodes_parent ON imdb_episodes(parent_imdb_id);
CREATE INDEX idx_imdb_episodes_season ON imdb_episodes(parent_imdb_id, season_number);
CREATE INDEX idx_imdb_episodes_air_date ON imdb_episodes(air_date);

-- IMDB Alternative Titles (AKAs)
-- Source: title.akas.tsv.gz
CREATE TABLE IF NOT EXISTS imdb_akas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    imdb_id BIGINT NOT NULL,               -- tconst (numeric)
    ordering INTEGER NOT NULL,              -- ordering
    title TEXT NOT NULL,                    -- title
    region TEXT,                            -- region (e.g., "US", "GB", "JP")
    language TEXT,                          -- language
    types TEXT,                             -- types (e.g., "imdbDisplay", "original")
    attributes TEXT,                        -- attributes
    is_original_title BOOLEAN DEFAULT 0,   -- isOriginalTitle
    FOREIGN KEY (imdb_id) REFERENCES imdb_series(imdb_id) ON DELETE CASCADE
);

CREATE INDEX idx_imdb_akas_imdb_id ON imdb_akas(imdb_id);
CREATE INDEX idx_imdb_akas_region ON imdb_akas(region);

-- IMDB Crew (Directors/Writers for series)
-- Source: title.crew.tsv.gz
CREATE TABLE IF NOT EXISTS imdb_crew (
    imdb_id BIGINT PRIMARY KEY,            -- tconst (numeric)
    directors TEXT,                         -- Comma-separated nconst values
    writers TEXT,                           -- Comma-separated nconst values
    FOREIGN KEY (imdb_id) REFERENCES imdb_series(imdb_id) ON DELETE CASCADE
);

-- IMDB People (Names)
-- Source: name.basics.tsv.gz
CREATE TABLE IF NOT EXISTS imdb_people (
    imdb_id BIGINT PRIMARY KEY,            -- nconst numeric (e.g., 123456 from nm0123456)
    name TEXT NOT NULL,                     -- primaryName
    birth_year INTEGER,                     -- birthYear
    death_year INTEGER,                     -- deathYear
    primary_profession TEXT,                -- Comma-separated professions
    known_for_titles TEXT,                  -- Comma-separated tconst values
    last_synced_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_imdb_people_name ON imdb_people(name);

-- Sync metadata tracking
CREATE TABLE IF NOT EXISTS imdb_sync_status (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dataset_name TEXT NOT NULL UNIQUE,      -- e.g., "title.basics", "title.episode"
    file_date TEXT,                         -- Date from filename or Last-Modified header
    rows_processed INTEGER DEFAULT 0,
    rows_inserted INTEGER DEFAULT 0,
    rows_updated INTEGER DEFAULT 0,
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    status TEXT NOT NULL DEFAULT 'running', -- running, completed, failed
    error_message TEXT
);

-- Add imdb_id column to existing series table if needed (for linking)
-- This allows linking local series to IMDB data
CREATE INDEX IF NOT EXISTS idx_series_imdb_id ON series(imdb_id);
