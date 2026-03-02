-- People from name.basics + Credits from title.principals
-- Pre-filtered: only people known for titles in our series/movie sets

CREATE TABLE IF NOT EXISTS imdb_people (
    nconst              BIGINT PRIMARY KEY,         -- nm0000001 → 1
    primary_name        TEXT NOT NULL,
    birth_year          SMALLINT,
    death_year          SMALLINT,
    primary_profession  TEXT,                        -- comma-separated
    known_for_titles    TEXT                         -- comma-separated tt IDs (numeric)
);

CREATE TABLE IF NOT EXISTS imdb_credits (
    tconst      BIGINT NOT NULL,                    -- tt ID (numeric)
    nconst      BIGINT NOT NULL REFERENCES imdb_people(nconst),
    ordering    SMALLINT NOT NULL,                   -- billing order
    category    TEXT NOT NULL,                        -- actor, director, writer, etc.
    job         TEXT,                                 -- specific job title
    characters  TEXT,                                 -- JSON array string from IMDB
    PRIMARY KEY (tconst, nconst, ordering)
);

CREATE INDEX IF NOT EXISTS idx_credits_tconst ON imdb_credits(tconst);
CREATE INDEX IF NOT EXISTS idx_credits_nconst ON imdb_credits(nconst);
CREATE INDEX IF NOT EXISTS idx_people_name ON imdb_people(primary_name);
CREATE INDEX IF NOT EXISTS idx_people_name_lower ON imdb_people(LOWER(primary_name));
