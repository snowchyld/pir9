-- Migrate all JSON-as-TEXT columns to native JSONB (ADR-006)
--
-- PostgreSQL-only: enables GIN indexes, containment queries, native JSON ops.
-- Pattern: DROP DEFAULT → ALTER TYPE → SET DEFAULT (required because TEXT defaults
-- can't be auto-cast to JSONB during ALTER COLUMN TYPE).
--
-- 60 columns across 25 tables. Idempotent: skips columns already converted.

DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN
        SELECT c.table_name, c.column_name, c.column_default
        FROM information_schema.columns c
        WHERE c.table_schema = 'public'
        AND c.data_type IN ('text', 'character varying')
        AND c.column_name IN (
            'quality', 'languages', 'media_info', 'settings', 'items', 'tags',
            'format_items', 'specifications', 'episode_ids', 'custom_formats',
            'data', 'images', 'genres', 'unmapped_folders', 'body', 'release_json',
            'required', 'ignored', 'preferred', 'secondary_types', 'filters',
            'status_messages'
        )
        ORDER BY c.table_name, c.column_name
    LOOP
        -- Step 1: Drop existing default (TEXT default blocks type change)
        IF r.column_default IS NOT NULL THEN
            EXECUTE format('ALTER TABLE %I ALTER COLUMN %I DROP DEFAULT', r.table_name, r.column_name);
        END IF;

        -- Step 2: Convert TEXT → JSONB
        EXECUTE format('ALTER TABLE %I ALTER COLUMN %I TYPE JSONB USING %I::jsonb', r.table_name, r.column_name, r.column_name);

        -- Step 3: Set JSONB default
        IF r.column_name IN ('quality', 'settings', 'data', 'body') THEN
            EXECUTE format('ALTER TABLE %I ALTER COLUMN %I SET DEFAULT ''{}''::jsonb', r.table_name, r.column_name);
        ELSIF r.column_name NOT IN ('media_info', 'unmapped_folders', 'release_json') THEN
            -- Array-type columns get [] default; nullable columns (media_info, unmapped_folders, release_json) keep NULL
            EXECUTE format('ALTER TABLE %I ALTER COLUMN %I SET DEFAULT ''[]''::jsonb', r.table_name, r.column_name);
        END IF;

        RAISE NOTICE 'Converted %.% to JSONB', r.table_name, r.column_name;
    END LOOP;
END $$;
