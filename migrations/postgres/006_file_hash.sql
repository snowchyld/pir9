-- Add file_hash column for content-based identification (BLAKE3)
ALTER TABLE episode_files ADD COLUMN IF NOT EXISTS file_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_episode_files_hash ON episode_files(file_hash);

ALTER TABLE movie_files ADD COLUMN IF NOT EXISTS file_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_movie_files_hash ON movie_files(file_hash);
