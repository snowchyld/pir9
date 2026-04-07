-- Track listings per release (links recordings to releases with position/disc info)
CREATE TABLE IF NOT EXISTS mb_release_tracks (
    release_mbid TEXT NOT NULL,
    disc_number INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL,
    title TEXT NOT NULL,
    recording_mbid TEXT,
    length_ms INTEGER,
    PRIMARY KEY (release_mbid, disc_number, position)
);

CREATE INDEX IF NOT EXISTS idx_mb_release_tracks_release ON mb_release_tracks(release_mbid);
CREATE INDEX IF NOT EXISTS idx_mb_release_tracks_recording ON mb_release_tracks(recording_mbid);
