-- Movies support

CREATE TABLE IF NOT EXISTS movies (
    id BIGSERIAL PRIMARY KEY,
    tmdb_id BIGINT NOT NULL DEFAULT 0,
    imdb_id VARCHAR(20),
    title VARCHAR(256) NOT NULL,
    clean_title VARCHAR(256) NOT NULL DEFAULT '',
    sort_title VARCHAR(256) NOT NULL DEFAULT '',
    status INTEGER NOT NULL DEFAULT 0,
    overview TEXT,
    monitored BOOLEAN NOT NULL DEFAULT false,
    quality_profile_id BIGINT NOT NULL DEFAULT 0,
    title_slug VARCHAR(256) NOT NULL DEFAULT '',
    path VARCHAR(512) NOT NULL DEFAULT '',
    root_folder_path VARCHAR(512) NOT NULL DEFAULT '',
    year INTEGER NOT NULL DEFAULT 0,
    release_date DATE,
    physical_release_date DATE,
    digital_release_date DATE,
    runtime INTEGER NOT NULL DEFAULT 0,
    studio VARCHAR(256),
    certification VARCHAR(64),
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    images TEXT NOT NULL DEFAULT '[]',
    has_file BOOLEAN NOT NULL DEFAULT false,
    movie_file_id BIGINT,
    added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ,
    imdb_rating REAL,
    imdb_votes INTEGER,
    UNIQUE(tmdb_id)
);

CREATE INDEX IF NOT EXISTS idx_movies_sort_title ON movies(sort_title);
CREATE INDEX IF NOT EXISTS idx_movies_imdb_id ON movies(imdb_id);
CREATE INDEX IF NOT EXISTS idx_movies_monitored ON movies(monitored);
CREATE INDEX IF NOT EXISTS idx_movies_year ON movies(year);

-- Movie files (one movie = one file, simpler than episodes)
CREATE TABLE IF NOT EXISTS movie_files (
    id BIGSERIAL PRIMARY KEY,
    movie_id BIGINT NOT NULL REFERENCES movies(id) ON DELETE CASCADE,
    relative_path VARCHAR(512) NOT NULL DEFAULT '',
    path VARCHAR(1024) NOT NULL DEFAULT '',
    size BIGINT NOT NULL DEFAULT 0,
    date_added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scene_name VARCHAR(512),
    release_group VARCHAR(128),
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    media_info TEXT,
    original_file_path VARCHAR(1024),
    edition VARCHAR(256)
);

CREATE INDEX IF NOT EXISTS idx_movie_files_movie_id ON movie_files(movie_id);
