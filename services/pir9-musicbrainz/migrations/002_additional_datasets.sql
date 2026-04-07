-- Labels (record labels)
CREATE TABLE IF NOT EXISTS mb_labels (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT,
    label_type TEXT,
    area TEXT,
    label_code INTEGER,
    begin_date TEXT,
    end_date TEXT,
    disambiguation TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Recordings (individual tracks/songs)
CREATE TABLE IF NOT EXISTS mb_recordings (
    mbid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    artist_mbid TEXT,
    artist_name TEXT,
    length_ms INTEGER,
    first_release_date TEXT,
    isrcs TEXT NOT NULL DEFAULT '[]',
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Works (compositions)
CREATE TABLE IF NOT EXISTS mb_works (
    mbid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    work_type TEXT,
    languages TEXT NOT NULL DEFAULT '[]',
    iswcs TEXT NOT NULL DEFAULT '[]',
    disambiguation TEXT,
    genres TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Areas (countries/regions)
CREATE TABLE IF NOT EXISTS mb_areas (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT,
    area_type TEXT,
    iso_3166_1 TEXT,
    iso_3166_2 TEXT,
    disambiguation TEXT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Series (compilation/album series)
CREATE TABLE IF NOT EXISTS mb_series (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    series_type TEXT,
    disambiguation TEXT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Events (concerts/festivals)
CREATE TABLE IF NOT EXISTS mb_events (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    event_type TEXT,
    begin_date TEXT,
    end_date TEXT,
    time TEXT,
    cancelled BOOLEAN NOT NULL DEFAULT false,
    disambiguation TEXT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Instruments
CREATE TABLE IF NOT EXISTS mb_instruments (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    instrument_type TEXT,
    description TEXT,
    disambiguation TEXT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Places (venues/studios)
CREATE TABLE IF NOT EXISTS mb_places (
    mbid TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    place_type TEXT,
    area TEXT,
    coordinates TEXT,
    disambiguation TEXT,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_mb_labels_name ON mb_labels(name);
CREATE INDEX IF NOT EXISTS idx_mb_recordings_artist ON mb_recordings(artist_mbid);
CREATE INDEX IF NOT EXISTS idx_mb_recordings_title ON mb_recordings(title);
CREATE INDEX IF NOT EXISTS idx_mb_works_title ON mb_works(title);
CREATE INDEX IF NOT EXISTS idx_mb_areas_name ON mb_areas(name);
CREATE INDEX IF NOT EXISTS idx_mb_areas_iso ON mb_areas(iso_3166_1);
