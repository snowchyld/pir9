-- Podcast tables (SQLite)

CREATE TABLE IF NOT EXISTS podcasts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    overview TEXT,
    author TEXT,
    feed_url TEXT NOT NULL,
    website_url TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    images TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    path TEXT NOT NULL,
    root_folder_path TEXT NOT NULL,
    quality_profile_id INTEGER NOT NULL DEFAULT 1,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_info_sync TIMESTAMP,
    title_slug TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    podcast_id INTEGER NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    overview TEXT,
    episode_number INTEGER,
    season_number INTEGER NOT NULL DEFAULT 1,
    air_date_utc TIMESTAMP,
    duration_ms INTEGER,
    download_url TEXT,
    file_size BIGINT,
    has_file BOOLEAN NOT NULL DEFAULT 0,
    podcast_file_id INTEGER,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    guid TEXT
);

CREATE TABLE IF NOT EXISTS podcast_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    podcast_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    quality TEXT NOT NULL DEFAULT '{}',
    media_info TEXT,
    date_added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_podcast_episodes_podcast ON podcast_episodes(podcast_id);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_air_date ON podcast_episodes(air_date_utc);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_guid ON podcast_episodes(guid);
CREATE INDEX IF NOT EXISTS idx_podcast_files_podcast ON podcast_files(podcast_id);
