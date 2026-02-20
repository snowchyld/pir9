-- Initial database schema for pir9 (PostgreSQL)
-- This schema matches the Rust model definitions exactly

-- Series table (matches SeriesDbModel - all IDs are i64)
CREATE TABLE IF NOT EXISTS series (
    id BIGSERIAL PRIMARY KEY,
    tvdb_id BIGINT NOT NULL UNIQUE,      -- i64 in Rust
    tv_rage_id BIGINT NOT NULL DEFAULT 0, -- i64 in Rust
    tv_maze_id BIGINT NOT NULL DEFAULT 0, -- i64 in Rust
    imdb_id TEXT,
    tmdb_id BIGINT NOT NULL DEFAULT 0,    -- i64 in Rust
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    overview TEXT,
    monitored BOOLEAN NOT NULL DEFAULT true,
    monitor_new_items INTEGER NOT NULL DEFAULT 0,
    quality_profile_id BIGINT NOT NULL DEFAULT 0,   -- i64 in Rust
    language_profile_id BIGINT,                     -- Option<i64> in Rust
    season_folder BOOLEAN NOT NULL DEFAULT true,
    series_type INTEGER NOT NULL DEFAULT 0,
    title_slug TEXT NOT NULL,
    path TEXT NOT NULL,
    root_folder_path TEXT NOT NULL,
    year INTEGER NOT NULL DEFAULT 0,
    first_aired DATE,
    last_aired DATE,
    runtime INTEGER NOT NULL DEFAULT 0,
    network TEXT,
    certification TEXT,
    use_scene_numbering BOOLEAN NOT NULL DEFAULT false,
    added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_info_sync TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_series_tvdb_id ON series(tvdb_id);
CREATE INDEX IF NOT EXISTS idx_series_sort_title ON series(sort_title);
CREATE INDEX IF NOT EXISTS idx_series_monitored ON series(monitored);

-- Episode files table (must be created before episodes due to FK)
CREATE TABLE IF NOT EXISTS episode_files (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    date_added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    scene_name TEXT,
    release_group TEXT,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    media_info TEXT,
    original_file_path TEXT
);

CREATE INDEX IF NOT EXISTS idx_episode_files_series_id ON episode_files(series_id);
CREATE INDEX IF NOT EXISTS idx_episode_files_season ON episode_files(series_id, season_number);

-- Episodes table (matches EpisodeDbModel)
CREATE TABLE IF NOT EXISTS episodes (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    tvdb_id BIGINT NOT NULL DEFAULT 0,    -- i64 in Rust
    episode_file_id BIGINT REFERENCES episode_files(id) ON DELETE SET NULL,  -- Option<i64> in Rust
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    absolute_episode_number INTEGER,
    scene_absolute_episode_number INTEGER,
    scene_episode_number INTEGER,
    scene_season_number INTEGER,
    title TEXT NOT NULL DEFAULT '',
    overview TEXT,
    air_date DATE,
    air_date_utc TIMESTAMP,
    runtime INTEGER NOT NULL DEFAULT 0,
    has_file BOOLEAN NOT NULL DEFAULT false,
    monitored BOOLEAN NOT NULL DEFAULT true,
    unverified_scene_numbering BOOLEAN NOT NULL DEFAULT false,
    added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_search_time TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_episodes_series_id ON episodes(series_id);
CREATE INDEX IF NOT EXISTS idx_episodes_season ON episodes(series_id, season_number);
CREATE INDEX IF NOT EXISTS idx_episodes_has_file ON episodes(has_file);
CREATE INDEX IF NOT EXISTS idx_episodes_monitored ON episodes(monitored);
CREATE UNIQUE INDEX IF NOT EXISTS idx_episodes_series_season_episode ON episodes(series_id, season_number, episode_number);

-- Episode to file mapping (for multi-episode files)
CREATE TABLE IF NOT EXISTS episode_file_mapping (
    episode_file_id BIGINT NOT NULL REFERENCES episode_files(id) ON DELETE CASCADE,
    episode_id BIGINT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    PRIMARY KEY (episode_file_id, episode_id)
);

-- Seasons table
CREATE TABLE IF NOT EXISTS seasons (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(series_id, season_number)
);

CREATE INDEX IF NOT EXISTS idx_seasons_series_id ON seasons(series_id);

-- Indexers table (matches IndexerDbModel)
CREATE TABLE IF NOT EXISTS indexers (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    enable_rss BOOLEAN NOT NULL DEFAULT true,
    enable_automatic_search BOOLEAN NOT NULL DEFAULT true,
    enable_interactive_search BOOLEAN NOT NULL DEFAULT true,
    protocol INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 25,
    download_client_id BIGINT NOT NULL DEFAULT 0,  -- i64 in Rust
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_indexers_name ON indexers(name);

-- Download clients table (matches DownloadClientDbModel)
CREATE TABLE IF NOT EXISTS download_clients (
    id BIGSERIAL PRIMARY KEY,
    enable BOOLEAN NOT NULL DEFAULT true,
    protocol INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_download_clients_name ON download_clients(name);

-- Notifications table (matches NotificationDbModel)
CREATE TABLE IF NOT EXISTS notifications (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    on_grab BOOLEAN NOT NULL DEFAULT false,
    on_download BOOLEAN NOT NULL DEFAULT false,
    on_upgrade BOOLEAN NOT NULL DEFAULT false,
    on_rename BOOLEAN NOT NULL DEFAULT false,
    on_series_delete BOOLEAN NOT NULL DEFAULT false,
    on_episode_file_delete BOOLEAN NOT NULL DEFAULT false,
    on_episode_file_delete_for_upgrade BOOLEAN NOT NULL DEFAULT false,
    on_health_issue BOOLEAN NOT NULL DEFAULT false,
    on_health_restored BOOLEAN NOT NULL DEFAULT false,
    on_manual_interaction_required BOOLEAN NOT NULL DEFAULT false,
    on_application_update BOOLEAN NOT NULL DEFAULT false,
    include_health_warnings BOOLEAN NOT NULL DEFAULT false,
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Quality profiles table
CREATE TABLE IF NOT EXISTS quality_profiles (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT true,
    cutoff INTEGER NOT NULL DEFAULT 0,
    items TEXT NOT NULL DEFAULT '[]',
    min_format_score INTEGER NOT NULL DEFAULT 0,
    cutoff_format_score INTEGER NOT NULL DEFAULT 0,
    format_items TEXT NOT NULL DEFAULT '[]'
);

-- Language profiles table
CREATE TABLE IF NOT EXISTS language_profiles (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT true,
    cutoff INTEGER NOT NULL DEFAULT 1,
    languages TEXT NOT NULL DEFAULT '[]'
);

-- Delay profiles table
CREATE TABLE IF NOT EXISTS delay_profiles (
    id BIGSERIAL PRIMARY KEY,
    enable_usenet BOOLEAN NOT NULL DEFAULT true,
    enable_torrent BOOLEAN NOT NULL DEFAULT true,
    preferred_protocol INTEGER NOT NULL DEFAULT 0,
    usenet_delay INTEGER NOT NULL DEFAULT 0,
    torrent_delay INTEGER NOT NULL DEFAULT 0,
    bypass_if_highest_quality BOOLEAN NOT NULL DEFAULT false,
    bypass_if_above_custom_format_score INTEGER NOT NULL DEFAULT 0,
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Custom formats table
CREATE TABLE IF NOT EXISTS custom_formats (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    include_custom_format_when_renaming BOOLEAN NOT NULL DEFAULT false,
    specifications TEXT NOT NULL DEFAULT '[]'
);

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id BIGSERIAL PRIMARY KEY,
    label TEXT NOT NULL UNIQUE
);

-- History table (matches HistoryDbModel)
CREATE TABLE IF NOT EXISTS history (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,  -- i64 in Rust
    episode_id BIGINT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE, -- i64 in Rust
    source_title TEXT NOT NULL,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    custom_formats TEXT NOT NULL DEFAULT '[]',
    custom_format_score INTEGER NOT NULL DEFAULT 0,  -- i32 in Rust
    quality_cutoff_not_met BOOLEAN NOT NULL DEFAULT false,
    date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    download_id TEXT,
    event_type INTEGER NOT NULL DEFAULT 0,  -- i32 in Rust
    data TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_history_series_id ON history(series_id);
CREATE INDEX IF NOT EXISTS idx_history_episode_id ON history(episode_id);
CREATE INDEX IF NOT EXISTS idx_history_date ON history(date);
CREATE INDEX IF NOT EXISTS idx_history_download_id ON history(download_id);

-- Blocklist table
CREATE TABLE IF NOT EXISTS blocklist (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    episode_ids TEXT NOT NULL DEFAULT '[]',
    source_title TEXT NOT NULL,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    custom_formats TEXT NOT NULL DEFAULT '[]',
    custom_format_score INTEGER NOT NULL DEFAULT 0,
    protocol INTEGER NOT NULL DEFAULT 0,
    indexer TEXT NOT NULL DEFAULT '',
    message TEXT,
    date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_blocklist_series_id ON blocklist(series_id);

-- Root folders table
CREATE TABLE IF NOT EXISTS root_folders (
    id BIGSERIAL PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    accessible BOOLEAN NOT NULL DEFAULT true,
    free_space BIGINT,
    total_space BIGINT,
    unmapped_folders TEXT
);

-- Config table
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Scheduled tasks table
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id BIGSERIAL PRIMARY KEY,
    type_name TEXT NOT NULL UNIQUE,
    interval INTEGER NOT NULL DEFAULT 0,
    last_execution TIMESTAMP,
    last_start_time TIMESTAMP
);

-- Commands table
CREATE TABLE IF NOT EXISTS commands (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    command_name TEXT NOT NULL,
    message TEXT,
    body TEXT,
    priority TEXT NOT NULL DEFAULT 'normal',
    status TEXT NOT NULL DEFAULT 'queued',
    result TEXT,
    queued TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started TIMESTAMP,
    ended TIMESTAMP,
    duration TEXT,
    exception TEXT,
    trigger TEXT NOT NULL DEFAULT 'unspecified',
    state_change_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    send_updates_to_client BOOLEAN NOT NULL DEFAULT false,
    update_scheduled_task BOOLEAN NOT NULL DEFAULT true,
    last_execution_time TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_commands_status ON commands(status);
CREATE INDEX IF NOT EXISTS idx_commands_name ON commands(name);

-- Logs table
CREATE TABLE IF NOT EXISTS logs (
    id BIGSERIAL PRIMARY KEY,
    time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    exception TEXT,
    exception_type TEXT,
    level TEXT NOT NULL,
    logger TEXT,
    message TEXT,
    method TEXT,
    url TEXT
);

CREATE INDEX IF NOT EXISTS idx_logs_time ON logs(time);
CREATE INDEX IF NOT EXISTS idx_logs_level ON logs(level);

-- IMDB tables
CREATE TABLE IF NOT EXISTS imdb_titles (
    tconst TEXT PRIMARY KEY,
    title_type TEXT NOT NULL,
    primary_title TEXT NOT NULL,
    original_title TEXT,
    is_adult BOOLEAN NOT NULL DEFAULT false,
    start_year INTEGER,
    end_year INTEGER,
    runtime_minutes INTEGER,
    genres TEXT
);

CREATE INDEX IF NOT EXISTS idx_imdb_titles_primary_title ON imdb_titles(primary_title);
CREATE INDEX IF NOT EXISTS idx_imdb_titles_title_type ON imdb_titles(title_type);

CREATE TABLE IF NOT EXISTS imdb_ratings (
    tconst TEXT PRIMARY KEY REFERENCES imdb_titles(tconst) ON DELETE CASCADE,
    average_rating REAL NOT NULL,
    num_votes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS imdb_episodes (
    tconst TEXT PRIMARY KEY,
    parent_tconst TEXT NOT NULL,
    season_number INTEGER,
    episode_number INTEGER,
    FOREIGN KEY (parent_tconst) REFERENCES imdb_titles(tconst) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_imdb_episodes_parent ON imdb_episodes(parent_tconst);
CREATE INDEX IF NOT EXISTS idx_imdb_episodes_season_episode ON imdb_episodes(parent_tconst, season_number, episode_number);

CREATE TABLE IF NOT EXISTS imdb_sync_status (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_sync TIMESTAMP,
    titles_count INTEGER DEFAULT 0,
    ratings_count INTEGER DEFAULT 0,
    episodes_count INTEGER DEFAULT 0,
    sync_duration_secs INTEGER,
    last_error TEXT,
    is_syncing BOOLEAN DEFAULT false,
    last_processed_line BIGINT DEFAULT 0,
    current_file TEXT
);

-- Download queue table
CREATE TABLE IF NOT EXISTS download_queue (
    id BIGSERIAL PRIMARY KEY,
    download_id TEXT NOT NULL,
    download_client_id BIGINT NOT NULL REFERENCES download_clients(id) ON DELETE CASCADE,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    episode_id BIGINT REFERENCES episodes(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    sizeleft BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued',
    added TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    estimated_completion_time TIMESTAMP,
    error_message TEXT,
    output_path TEXT,
    UNIQUE(download_id, download_client_id)
);

CREATE INDEX IF NOT EXISTS idx_download_queue_series ON download_queue(series_id);
CREATE INDEX IF NOT EXISTS idx_download_queue_status ON download_queue(status);
CREATE INDEX IF NOT EXISTS idx_download_queue_download_id ON download_queue(download_id);

-- Insert default quality profile
INSERT INTO quality_profiles (id, name, cutoff, items)
VALUES (1, 'Any', 0, '[{"quality": {"id": 0, "name": "Unknown"}, "items": [], "allowed": true}]')
ON CONFLICT (id) DO NOTHING;

-- Insert default language profile
INSERT INTO language_profiles (id, name, cutoff, languages)
VALUES (1, 'English', 1, '[{"language": {"id": 1, "name": "English"}, "allowed": true}]')
ON CONFLICT (id) DO NOTHING;

-- Insert default delay profile
INSERT INTO delay_profiles (id, enable_usenet, enable_torrent, preferred_protocol)
VALUES (1, true, true, 0)
ON CONFLICT (id) DO NOTHING;

-- Insert default scheduled tasks
INSERT INTO scheduled_tasks (type_name, interval) VALUES
    ('RssSync', 15),
    ('RefreshSeries', 360),
    ('DownloadedEpisodesScan', 0),
    ('Housekeeping', 1440),
    ('HealthCheck', 5),
    ('Backup', 10080)
ON CONFLICT (type_name) DO NOTHING;

-- Insert default IMDB sync status
INSERT INTO imdb_sync_status (id, titles_count, ratings_count, episodes_count)
VALUES (1, 0, 0, 0)
ON CONFLICT (id) DO NOTHING;
