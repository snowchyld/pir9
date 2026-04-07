-- Add on_series_add and on_movie_add columns to notifications table
ALTER TABLE notifications ADD COLUMN on_series_add BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE notifications ADD COLUMN on_movie_add BOOLEAN NOT NULL DEFAULT 0;
