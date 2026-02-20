# ADR-003: Use SQLx with Compile-Time Checked Queries

## Status
Accepted

## Context

pir9 needs a database layer for series, episodes, movies, quality profiles, download history, and configuration. Options considered:

1. **SQLx** — async, compile-time checked SQL queries, no ORM abstraction
2. **SeaORM** — async ORM built on SQLx, generates Rust types from schema
3. **Diesel** — sync ORM with compile-time checked queries (requires connection at build time)

## Decision

Use **SQLx** with the `query!` and `query_as!` macros for compile-time verified SQL, wrapped in a **Repository pattern** (`src/core/datastore/repositories.rs`).

## Consequences

### Positive
- **Compile-time SQL checking** — typos, wrong column names, and type mismatches are caught before runtime
- **Zero abstraction cost** — raw SQL executes directly; no query builder overhead or N+1 risk from lazy loading
- **Async-native** — built for Tokio; no `spawn_blocking` wrappers needed
- **Multi-database support** — same crate supports both SQLite and PostgreSQL via feature flags
- **Migration system** — built-in `sqlx migrate` with numbered SQL files

### Negative
- **Requires database at compile time** — `sqlx::query!` needs a live DB (or `sqlx-data.json` offline mode) for compile-time checking, complicating CI
- **Manual SQL** — no query builder; developers write raw SQL (mitigated by the Repository pattern centralizing queries)
- **Schema changes require migration** — no auto-migration from model changes; must write SQL migrations manually
- **No lazy loading** — all data must be explicitly queried (prevents N+1 but requires thinking about joins upfront)

### Neutral
- The Repository pattern adds a thin abstraction layer that isolates SQL from business logic — services never see raw queries
- `DbModel` suffix convention distinguishes database structs from API response types
