-- MusicBrainz Artists
CREATE TABLE IF NOT EXISTS mb_artists (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT NOT NULL,
    artist_type TEXT,
    gender TEXT,
    area TEXT,
    begin_date TEXT,
    end_date TEXT,
    disambiguation TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    rating DOUBLE PRECISION,
    rating_count INTEGER,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- MusicBrainz Release Groups (≈ Albums)
CREATE TABLE IF NOT EXISTS mb_release_groups (
    mbid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    artist_mbid TEXT NOT NULL,
    artist_name TEXT NOT NULL,
    primary_type TEXT,
    secondary_types TEXT NOT NULL DEFAULT '[]',
    first_release_date TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    rating DOUBLE PRECISION,
    rating_count INTEGER,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- MusicBrainz Releases (specific editions of albums)
CREATE TABLE IF NOT EXISTS mb_releases (
    mbid TEXT PRIMARY KEY,
    release_group_mbid TEXT NOT NULL,
    title TEXT NOT NULL,
    artist_mbid TEXT,
    artist_name TEXT,
    date TEXT,
    country TEXT,
    status TEXT,
    barcode TEXT,
    packaging TEXT,
    track_count INTEGER NOT NULL DEFAULT 0,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Cover Art Archive mapping
CREATE TABLE IF NOT EXISTS mb_cover_art (
    release_mbid TEXT PRIMARY KEY,
    has_front BOOLEAN NOT NULL DEFAULT false,
    front_url TEXT,
    thumbnail_250 TEXT,
    thumbnail_500 TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sync tracking
CREATE TABLE IF NOT EXISTS mb_sync_status (
    id BIGSERIAL PRIMARY KEY,
    dataset_name TEXT NOT NULL,
    rows_processed BIGINT NOT NULL DEFAULT 0,
    rows_inserted BIGINT NOT NULL DEFAULT 0,
    rows_updated BIGINT NOT NULL DEFAULT 0,
    last_processed_id TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'running',
    error_message TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_mb_artists_name ON mb_artists(name);
CREATE INDEX IF NOT EXISTS idx_mb_artists_sort_name ON mb_artists(sort_name);
CREATE INDEX IF NOT EXISTS idx_mb_artists_name_lower ON mb_artists(LOWER(name));
CREATE INDEX IF NOT EXISTS idx_mb_release_groups_artist ON mb_release_groups(artist_mbid);
CREATE INDEX IF NOT EXISTS idx_mb_release_groups_title ON mb_release_groups(title);
CREATE INDEX IF NOT EXISTS idx_mb_releases_group ON mb_releases(release_group_mbid);
CREATE INDEX IF NOT EXISTS idx_mb_releases_artist ON mb_releases(artist_mbid);
CREATE INDEX IF NOT EXISTS idx_mb_sync_status ON mb_sync_status(dataset_name);
