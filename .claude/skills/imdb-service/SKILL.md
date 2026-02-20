---
name: imdb-service
description: Context for the standalone pir9-imdb service
user-invocable: false
---

# pir9-imdb Service Context

This skill loads automatically when editing files in `services/pir9-imdb/`. It provides critical context about this standalone service.

## Architecture

- **Standalone Rust service** in `services/pir9-imdb/` with its own `Cargo.toml`
- **Own PostgreSQL database** on port 5433 (separate from main pir9 DB)
- **Purpose**: Ingest and serve IMDB dataset files (titles, episodes, ratings)
- **API**: HTTP endpoints for searching/looking up IMDB titles

## Key Design Patterns

### Cancellation
- Uses `tokio_util::CancellationToken` for cooperative sync cancellation
- `SyncHandle` in `AppState` is the **source of truth** for running syncs — not the DB status column
- `download_dataset()` uses `tokio::select!` to race downloads against cancellation
- When cancelling: use `&*handle_guard` for status checks, not `handle_guard.take()` (which consumes the handle)

### Resumable Sync
- IMDB IDs are **monotonically increasing** in TSV files (tt0000001, tt0000002, ...)
- `last_processed_id` serves as a resume checkpoint
- Resume logic: when restarting from DB, **initialize local counters from saved values** to avoid progress regression
- `PROGRESS_INTERVAL = 10_000` — log progress every 10k records

### Batch Upserts
- Uses PostgreSQL `UNNEST`-based batch inserts for performance
- `BATCH_SIZE = 1000` — flush batch before every checkpoint save
- SQLx binds `Vec<&str>` → `text[]` and `Vec<Option<T>>` → nullable arrays natively

### Stale Record Cleanup
- `tokio::spawn` panics are **silent** — cleanup stale DB records at the next interaction point
- Use `ORDER BY CASE WHEN status = 'running' THEN 0 ELSE 1 END` to prioritize active records

## File Structure

```
services/pir9-imdb/
├── Cargo.toml          # Independent versioning (keep in sync with main)
├── src/
│   ├── main.rs         # Server setup, routes
│   ├── sync.rs         # Dataset download and parsing
│   ├── models.rs       # Database models
│   └── handlers.rs     # HTTP handlers
└── migrations/         # PostgreSQL migrations
```

## Important Notes

- `reqwest::Bytes` requires the `bytes` crate — use `.to_vec()` to return `Vec<u8>` and avoid the dependency
- IMDB client returns `Option<f64>` for ratings, but `MovieDbModel` uses `Option<f32>` — cast needed
- **Version bumps**: When changing this service, bump BOTH `Cargo.toml` files (main + imdb service)
