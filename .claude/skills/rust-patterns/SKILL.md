---
name: rust-patterns
description: Rust coding conventions and patterns for the pir9 codebase
user-invocable: false
---

# pir9 Rust Conventions

This skill provides background context when editing Rust files in the pir9 codebase.

## Error Handling

- **Application errors**: Use `anyhow::Result` and `anyhow::bail!`/`anyhow::Context`
- **Module/library errors**: Define typed errors with `thiserror::Error`
- **Never use `unwrap()`** in non-test code — use `?` or `expect("descriptive reason")`
- **API errors**: Return `Result<Json<T>, ApiError>` from handlers

## Async Patterns

- Runtime: **Tokio** — all I/O is async
- Shared state: `Arc<AppState>` passed via Axum's `State` extractor
- Concurrent maps: `DashMap` for lock-free concurrent access
- Inter-component communication: Event bus (`core/messaging.rs`) — in-memory or Redis pub/sub
- Background jobs: `core/scheduler.rs` with Tokio tasks
- Cancellation: `tokio_util::CancellationToken` for cooperative shutdown

## Type Conventions

- **Derive order**: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default`
  (omit traits that don't apply — e.g., skip `Copy` for types with `String` fields)
- **Serde casing**: `#[serde(rename_all = "camelCase")]` on all API-facing types
- **Acronyms in enums**: Keep uppercase — `SDTV`, `DVD`, `HDTV`, `WEBDL`
  Use `#[allow(clippy::upper_case_acronyms)]` on the enum
- **Database models**: Suffix with `DbModel` (e.g., `SeriesDbModel`, `EpisodeDbModel`)
- **API responses**: Match Sonarr JSON shapes for v3, free to evolve in v5

## Database (SQLx)

- **Compile-time checked queries**: Use `sqlx::query!` and `sqlx::query_as!` macros
- **Repository pattern**: `XxxRepository::new(pool: DbPool)` in `core/datastore/`
- **Migrations**: Numbered SQL files in `migrations/` — format `NNN_description.sql`
- **SQLite + PostgreSQL**: Feature-gated with `sqlite` (default) and `postgres` features

## Project Structure

- `src/api/v3/` — Legacy Sonarr-compatible endpoints (DO NOT change response shapes)
- `src/api/v5/` — Current API endpoints (freely evolvable)
- `src/core/` — Business logic, domain models, services
- `src/core/datastore/` — Database repositories
- `src/web/` — Axum server setup, middleware, `AppState`

## Logging

Use `tracing` crate macros with structured fields:
```rust
use tracing::{info, warn, debug, error};
info!(series_id = %id, "importing series");
warn!(error = %e, "failed to fetch metadata");
```

## Testing

- Unit tests in `#[cfg(test)]` module within the same file
- Test naming: `test_<function>_<scenario>`
- Use `#[tokio::test]` for async tests
- Integration tests in `tests/` directory
