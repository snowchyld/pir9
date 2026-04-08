-- Add index on episode_files.path for path-based lookups
CREATE INDEX IF NOT EXISTS idx_episode_files_path ON episode_files(path);
