-- Music domain tables (artists, albums, tracks, track files)

CREATE TABLE IF NOT EXISTS artists (
    id BIGSERIAL PRIMARY KEY,
    musicbrainz_id TEXT,
    name TEXT NOT NULL,
    clean_name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    overview TEXT,
    artist_type TEXT NOT NULL DEFAULT 'group',
    status TEXT NOT NULL DEFAULT 'continuing',
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

CREATE TABLE IF NOT EXISTS albums (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    musicbrainz_id TEXT,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    album_type TEXT NOT NULL DEFAULT 'album',
    release_date DATE,
    genres TEXT NOT NULL DEFAULT '[]',
    images TEXT NOT NULL DEFAULT '[]',
    monitored BOOLEAN NOT NULL DEFAULT true,
    added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS tracks (
    id BIGSERIAL PRIMARY KEY,
    album_id BIGINT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id BIGINT NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    track_number INTEGER NOT NULL,
    disc_number INTEGER NOT NULL DEFAULT 1,
    duration_ms INTEGER,
    has_file BOOLEAN NOT NULL DEFAULT false,
    track_file_id BIGINT,
    monitored BOOLEAN NOT NULL DEFAULT true,
    air_date_utc TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS track_files (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL,
    album_id BIGINT NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    quality TEXT NOT NULL DEFAULT '{}',
    media_info TEXT,
    date_added TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_albums_artist ON albums(artist_id);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);
CREATE INDEX IF NOT EXISTS idx_track_files_artist ON track_files(artist_id);
CREATE INDEX IF NOT EXISTS idx_track_files_album ON track_files(album_id);
