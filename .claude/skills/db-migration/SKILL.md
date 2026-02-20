---
name: db-migration
description: Create a new SQLx database migration
user-invocable: true
arguments:
  - name: description
    description: Short description of the migration (e.g., "add-notifications-table")
    required: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# Database Migration: $ARGUMENTS

You are creating a new SQLx database migration for **$ARGUMENTS**.

## Steps

### 1. Determine the next migration number

List existing migrations to find the next number:
```bash
ls migrations/
```
Naming format: `NNN_description.sql` (e.g., `008_add_notifications.sql`)

### 2. Determine which database(s) need migration

- **Main database** (`migrations/`): Series, episodes, movies, profiles, settings, etc.
- **pir9-imdb database** (`services/pir9-imdb/migrations/`): IMDB title/episode/ratings data

Most migrations only need the main database unless touching IMDB data.

### 3. Create the migration file

Create `migrations/NNN_$ARGUMENTS.sql`:

```sql
-- $ARGUMENTS
-- Created: YYYY-MM-DD

CREATE TABLE IF NOT EXISTS table_name (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- columns...
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add indexes for frequently queried columns
CREATE INDEX IF NOT EXISTS idx_table_column ON table_name(column);
```

### 4. SQLx conventions

- **Primary keys**: `INTEGER PRIMARY KEY AUTOINCREMENT` (SQLite) or `BIGSERIAL PRIMARY KEY` (PostgreSQL)
- **Timestamps**: Always include `created_at` and `updated_at` with defaults
- **Indexes**: Add for foreign keys and frequently queried columns
- **`IF NOT EXISTS`**: Always use for CREATE TABLE/INDEX (idempotent migrations)
- **Foreign keys**: Reference parent tables with `ON DELETE CASCADE` or `ON DELETE SET NULL` as appropriate
- **Naming**: snake_case for tables and columns

### 5. Update repository code

If the migration adds a new table, create corresponding repository methods in `src/core/datastore/repositories.rs` using SQLx compile-time checked queries:

```rust
pub async fn find_all(&self) -> Result<Vec<TableDbModel>> {
    let rows = sqlx::query_as!(TableDbModel, "SELECT * FROM table_name")
        .fetch_all(&*self.pool)
        .await?;
    Ok(rows)
}
```

### 6. Verify

```bash
cargo build  # SQLx compile-time checks will catch schema mismatches
cargo test
```

## PostgreSQL Considerations

If the project uses PostgreSQL (`PIR9_DB_TYPE=postgres`), ensure SQL syntax is compatible:
- Use `BIGSERIAL` instead of `INTEGER PRIMARY KEY AUTOINCREMENT`
- Use `TIMESTAMP WITH TIME ZONE` instead of `TIMESTAMP`
- Use `TEXT` instead of `VARCHAR` (PostgreSQL treats them identically)
