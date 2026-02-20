-- Initial database schema for Pir9
-- Creates all core tables

-- Series table
CREATE TABLE IF NOT EXISTS series (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tvdb_id INTEGER NOT NULL UNIQUE,
    tv_rage_id INTEGER NOT NULL DEFAULT 0,
    tv_maze_id INTEGER NOT NULL DEFAULT 0,
    imdb_id TEXT,
    tmdb_id INTEGER NOT NULL DEFAULT 0,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    overview TEXT,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    monitor_new_items INTEGER NOT NULL DEFAULT 0,
    quality_profile_id INTEGER NOT NULL DEFAULT 0,
    language_profile_id INTEGER,
    season_folder BOOLEAN NOT NULL DEFAULT 1,
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
    use_scene_numbering BOOLEAN NOT NULL DEFAULT 0,
    added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_info_sync DATETIME
);

CREATE INDEX idx_series_tvdb_id ON series(tvdb_id);
CREATE INDEX idx_series_sort_title ON series(sort_title);
CREATE INDEX idx_series_monitored ON series(monitored);

-- Episodes table
CREATE TABLE IF NOT EXISTS episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL,
    tvdb_id INTEGER NOT NULL DEFAULT 0,
    episode_file_id INTEGER,
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    absolute_episode_number INTEGER,
    scene_absolute_episode_number INTEGER,
    scene_episode_number INTEGER,
    scene_season_number INTEGER,
    title TEXT NOT NULL DEFAULT '',
    overview TEXT,
    air_date DATE,
    air_date_utc DATETIME,
    runtime INTEGER NOT NULL DEFAULT 0,
    has_file BOOLEAN NOT NULL DEFAULT 0,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    unverified_scene_numbering BOOLEAN NOT NULL DEFAULT 0,
    added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_search_time DATETIME,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    FOREIGN KEY (episode_file_id) REFERENCES episode_files(id) ON DELETE SET NULL
);

CREATE INDEX idx_episodes_series_id ON episodes(series_id);
CREATE INDEX idx_episodes_season ON episodes(series_id, season_number);
CREATE INDEX idx_episodes_has_file ON episodes(has_file);
CREATE INDEX idx_episodes_monitored ON episodes(monitored);
CREATE UNIQUE INDEX idx_episodes_series_season_episode ON episodes(series_id, season_number, episode_number);

-- Episode files table
CREATE TABLE IF NOT EXISTS episode_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL,
    season_number INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    date_added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    scene_name TEXT,
    release_group TEXT,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    media_info TEXT,
    original_file_path TEXT,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE INDEX idx_episode_files_series_id ON episode_files(series_id);
CREATE INDEX idx_episode_files_season ON episode_files(series_id, season_number);

-- Episode to file mapping (for multi-episode files)
CREATE TABLE IF NOT EXISTS episode_file_mapping (
    episode_file_id INTEGER NOT NULL,
    episode_id INTEGER NOT NULL,
    PRIMARY KEY (episode_file_id, episode_id),
    FOREIGN KEY (episode_file_id) REFERENCES episode_files(id) ON DELETE CASCADE,
    FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE
);

-- Seasons table
CREATE TABLE IF NOT EXISTS seasons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL,
    season_number INTEGER NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    UNIQUE(series_id, season_number)
);

CREATE INDEX idx_seasons_series_id ON seasons(series_id);

-- Indexers table
CREATE TABLE IF NOT EXISTS indexers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    enable_rss BOOLEAN NOT NULL DEFAULT 1,
    enable_automatic_search BOOLEAN NOT NULL DEFAULT 1,
    enable_interactive_search BOOLEAN NOT NULL DEFAULT 1,
    protocol INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 25,
    download_client_id INTEGER NOT NULL DEFAULT 0,
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_indexers_name ON indexers(name);

-- Download clients table
CREATE TABLE IF NOT EXISTS download_clients (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    enable BOOLEAN NOT NULL DEFAULT 1,
    protocol INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_download_clients_name ON download_clients(name);

-- Notifications table
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    implementation TEXT NOT NULL,
    config_contract TEXT NOT NULL,
    on_grab BOOLEAN NOT NULL DEFAULT 0,
    on_download BOOLEAN NOT NULL DEFAULT 0,
    on_upgrade BOOLEAN NOT NULL DEFAULT 0,
    on_rename BOOLEAN NOT NULL DEFAULT 0,
    on_series_delete BOOLEAN NOT NULL DEFAULT 0,
    on_episode_file_delete BOOLEAN NOT NULL DEFAULT 0,
    on_episode_file_delete_for_upgrade BOOLEAN NOT NULL DEFAULT 0,
    on_health_issue BOOLEAN NOT NULL DEFAULT 0,
    on_health_restored BOOLEAN NOT NULL DEFAULT 0,
    on_manual_interaction_required BOOLEAN NOT NULL DEFAULT 0,
    on_application_update BOOLEAN NOT NULL DEFAULT 0,
    include_health_warnings BOOLEAN NOT NULL DEFAULT 0,
    settings TEXT NOT NULL DEFAULT '{}',
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Quality profiles table
CREATE TABLE IF NOT EXISTS quality_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT 1,
    cutoff INTEGER NOT NULL DEFAULT 0,
    items TEXT NOT NULL DEFAULT '[]',
    min_format_score INTEGER NOT NULL DEFAULT 0,
    cutoff_format_score INTEGER NOT NULL DEFAULT 0,
    format_items TEXT NOT NULL DEFAULT '[]'
);

-- Language profiles table
CREATE TABLE IF NOT EXISTS language_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT 1,
    cutoff INTEGER NOT NULL DEFAULT 1,
    languages TEXT NOT NULL DEFAULT '[]'
);

-- Delay profiles table
CREATE TABLE IF NOT EXISTS delay_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    enable_usenet BOOLEAN NOT NULL DEFAULT 1,
    enable_torrent BOOLEAN NOT NULL DEFAULT 1,
    preferred_protocol INTEGER NOT NULL DEFAULT 0,
    usenet_delay INTEGER NOT NULL DEFAULT 0,
    torrent_delay INTEGER NOT NULL DEFAULT 0,
    bypass_if_highest_quality BOOLEAN NOT NULL DEFAULT 0,
    bypass_if_above_custom_format_score INTEGER NOT NULL DEFAULT 0,
    tags TEXT NOT NULL DEFAULT '[]'
);

-- Custom formats table
CREATE TABLE IF NOT EXISTS custom_formats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    include_custom_format_when_renaming BOOLEAN NOT NULL DEFAULT 0,
    specifications TEXT NOT NULL DEFAULT '[]'
);

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL UNIQUE
);

-- History table
CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL,
    episode_id INTEGER NOT NULL,
    source_title TEXT NOT NULL,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    custom_formats TEXT NOT NULL DEFAULT '[]',
    custom_format_score INTEGER NOT NULL DEFAULT 0,
    quality_cutoff_not_met BOOLEAN NOT NULL DEFAULT 0,
    date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    download_id TEXT,
    event_type INTEGER NOT NULL DEFAULT 0,
    data TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE
);

CREATE INDEX idx_history_series_id ON history(series_id);
CREATE INDEX idx_history_episode_id ON history(episode_id);
CREATE INDEX idx_history_date ON history(date);
CREATE INDEX idx_history_download_id ON history(download_id);

-- Blocklist table
CREATE TABLE IF NOT EXISTS blocklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL,
    episode_ids TEXT NOT NULL DEFAULT '[]',
    source_title TEXT NOT NULL,
    quality TEXT NOT NULL DEFAULT '{}',
    languages TEXT NOT NULL DEFAULT '[]',
    custom_formats TEXT NOT NULL DEFAULT '[]',
    custom_format_score INTEGER NOT NULL DEFAULT 0,
    protocol INTEGER NOT NULL DEFAULT 0,
    indexer TEXT NOT NULL DEFAULT '',
    message TEXT,
    date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE INDEX idx_blocklist_series_id ON blocklist(series_id);

-- Root folders table
CREATE TABLE IF NOT EXISTS root_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    accessible BOOLEAN NOT NULL DEFAULT 1,
    free_space INTEGER,
    total_space INTEGER,
    unmapped_folders TEXT
);

-- Config table
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Scheduled tasks table
CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type_name TEXT NOT NULL UNIQUE,
    interval INTEGER NOT NULL DEFAULT 0,
    last_execution DATETIME,
    last_start_time DATETIME
);

-- Commands table
CREATE TABLE IF NOT EXISTS commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    command_name TEXT NOT NULL,
    message TEXT,
    body TEXT,
    priority TEXT NOT NULL DEFAULT 'normal',
    status TEXT NOT NULL DEFAULT 'queued',
    result TEXT,
    queued DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started DATETIME,
    ended DATETIME,
    duration TEXT,
    exception TEXT,
    trigger TEXT NOT NULL DEFAULT 'unspecified',
    state_change_time DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    send_updates_to_client BOOLEAN NOT NULL DEFAULT 0,
    update_scheduled_task BOOLEAN NOT NULL DEFAULT 1,
    last_execution_time DATETIME
);

CREATE INDEX idx_commands_status ON commands(status);

-- Insert default quality profile
INSERT OR IGNORE INTO quality_profiles (id, name, cutoff, items) VALUES (
    1,
    'Any',
    0,
    '[{"quality": {"id": 0, "name": "Unknown"}, "items": [], "allowed": true}]'
);

-- Insert default language profile
INSERT OR IGNORE INTO language_profiles (id, name, cutoff, languages) VALUES (
    1,
    'English',
    1,
    '[{"language": {"id": 1, "name": "English"}, "allowed": true}]'
);

-- Insert default delay profile
INSERT OR IGNORE INTO delay_profiles (id, enable_usenet, enable_torrent, preferred_protocol) VALUES (
    1,
    1,
    1,
    0
);

-- Insert default scheduled tasks
INSERT OR IGNORE INTO scheduled_tasks (type_name, interval) VALUES
    ('RssSync', 15),
    ('RefreshSeries', 360),
    ('DownloadedEpisodesScan', 0),
    ('Housekeeping', 1440),
    ('HealthCheck', 5),
    ('Backup', 10080);
