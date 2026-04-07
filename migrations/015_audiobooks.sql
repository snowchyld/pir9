-- Audiobook tables
CREATE TABLE IF NOT EXISTS audiobooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    author TEXT,
    narrator TEXT,
    overview TEXT,
    publisher TEXT,
    isbn TEXT,
    asin TEXT,
    duration_ms BIGINT,
    release_date DATE,
    genres TEXT NOT NULL DEFAULT '[]',
    images TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    path TEXT NOT NULL,
    root_folder_path TEXT NOT NULL,
    quality_profile_id BIGINT NOT NULL DEFAULT 1,
    monitored BOOLEAN NOT NULL DEFAULT 1,
    has_file BOOLEAN NOT NULL DEFAULT 0,
    audiobook_file_id BIGINT,
    added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_info_sync DATETIME,
    title_slug TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS audiobook_chapters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audiobook_id BIGINT NOT NULL REFERENCES audiobooks(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    chapter_number INTEGER NOT NULL,
    duration_ms BIGINT,
    has_file BOOLEAN NOT NULL DEFAULT 0,
    monitored BOOLEAN NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS audiobook_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audiobook_id BIGINT NOT NULL,
    relative_path TEXT NOT NULL,
    path TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    quality TEXT NOT NULL DEFAULT '{}',
    media_info TEXT,
    date_added DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_audiobooks_title ON audiobooks(title);
CREATE INDEX IF NOT EXISTS idx_audiobooks_author ON audiobooks(author);
CREATE INDEX IF NOT EXISTS idx_audiobooks_isbn ON audiobooks(isbn);
CREATE INDEX IF NOT EXISTS idx_audiobooks_asin ON audiobooks(asin);
CREATE INDEX IF NOT EXISTS idx_audiobook_chapters_book ON audiobook_chapters(audiobook_id);
CREATE INDEX IF NOT EXISTS idx_audiobook_files_book ON audiobook_files(audiobook_id);
