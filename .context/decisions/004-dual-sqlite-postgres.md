# ADR-004: Support Both SQLite and PostgreSQL

## Status
Accepted

## Context

Sonarr uses SQLite exclusively. This is great for single-user deployments (no separate database server) but limits scalability and concurrent access. pir9 targets a wider range of deployments:

- **Home user on a NAS** — wants zero-config, no extra services
- **Power user with Docker Compose** — already runs PostgreSQL for other apps
- **Distributed mode** — server + workers need a shared database

## Decision

Support **both SQLite (default) and PostgreSQL** via Cargo feature flags:
- `sqlite` feature (default) — zero-config, embedded database
- `postgres` feature — full PostgreSQL with connection pooling

Database type is selected at runtime via `PIR9_DB_TYPE` environment variable or `config.toml`.

## Consequences

### Positive
- **Zero friction for new users** — SQLite just works, no database server needed
- **Production-ready scaling** — PostgreSQL for users who need concurrent access and distributed deployments
- **Choice** — users can start with SQLite and migrate to PostgreSQL when they outgrow it

### Negative
- **SQL dialect differences** — must maintain compatible SQL across both databases (e.g., `AUTOINCREMENT` vs `SERIAL`, `CURRENT_TIMESTAMP` differences)
- **Migration duplication risk** — migrations must work for both databases (currently focused on PostgreSQL syntax with SQLite compatibility)
- **Testing matrix** — ideally test against both databases (currently focused on PostgreSQL for CI)

### Neutral
- SQLx's `Any` pool type abstracts the connection, but compile-time query checking only works against one database at a time
- The pir9-imdb service is PostgreSQL-only (IMDB dataset is too large for SQLite — 10M+ titles)
