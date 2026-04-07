-- Add secondary_types to albums for proper grouping (Compilation, Soundtrack, Live, etc.)
ALTER TABLE albums ADD COLUMN IF NOT EXISTS secondary_types TEXT NOT NULL DEFAULT '[]';
