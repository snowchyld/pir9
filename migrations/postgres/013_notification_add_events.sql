-- Add on_series_add and on_movie_add columns to notifications table
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS on_series_add BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS on_movie_add BOOLEAN NOT NULL DEFAULT false;
