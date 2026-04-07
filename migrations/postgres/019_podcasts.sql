-- Podcast tables (PostgreSQL)

CREATE TABLE IF NOT EXISTS podcasts (
    id BIGSERIAL PRIMARY KEY,
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
    quality_profile_id BIGINT NOT NULL DEFAULT 1,
    monitored BOOLEAN NOT NULL DEFAULT true,
    added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ,
    title_slug TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS podcast_episodes (
    id BIGSERIAL PRIMARY KEY,
    podcast_id BIGINT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    overview TEXT,
    episode_number INTEGER,
    season_number INTEGER NOT NULL DEFAULT 1,
    air_date_utc TIMESTAMPTZ,
    duration_ms INTEGER,
    download_url TEXT,
    file_size BIGINT,
    has_file BOOLEAN NOT NULL DEFAULT false,
    podcast_file_id BIGINT,
    monitored BOOLEAN NOT NULL DEFAULT true,
    guid TEXT
);

CREATE TABLE IF NOT EXISTS podcast_files (
    id BIGSERIAL PRIMARY KEY,
    podcast_id BIGINT NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    quality TEXT NOT NULL DEFAULT '{}',
    media_info TEXT,
    date_added TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_podcast_episodes_podcast ON podcast_episodes(podcast_id);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_air_date ON podcast_episodes(air_date_utc);
CREATE INDEX IF NOT EXISTS idx_podcast_episodes_guid ON podcast_episodes(guid);
CREATE INDEX IF NOT EXISTS idx_podcast_files_podcast ON podcast_files(podcast_id);
