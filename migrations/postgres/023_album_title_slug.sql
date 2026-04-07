-- Add title_slug column to albums for human-readable URLs
ALTER TABLE albums ADD COLUMN IF NOT EXISTS title_slug TEXT NOT NULL DEFAULT '';

-- Backfill: generate slugs from clean_title
UPDATE albums SET title_slug = LOWER(REPLACE(REPLACE(REPLACE(TRIM(clean_title), ' ', '-'), '--', '-'), '''', ''))
WHERE title_slug = '';
