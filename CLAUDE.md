# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Pir9 is a Smart PVR (Personal Video Recorder) for TV and anime, written in Rust. It's a rewrite of Sonarr that maintains API compatibility while leveraging Rust's performance and safety.

## Build & Development Commands

```bash
# Build
cargo build --release                    # Release build
cargo build --release --features "redis-events"  # With Redis event bus

# Test
cargo test                               # Run all tests
cargo test test_name                     # Run specific test
cargo test -- --nocapture                # Show test output

# Lint
cargo clippy -- -D warnings              # Rust linting
cd frontend && npm run lint           # Frontend linting

# Run locally
RUST_LOG=debug cargo run --release       # Run with debug logging
```

### Makefile shortcuts

```bash
make dev-api          # Build Rust API locally
make dev-frontend     # Build frontend (frontend/dist/)
make watch-frontend   # Frontend dev server with hot reload
make test             # Run tests
make lint             # Run all linters
make run-api          # Run API locally with debug logging
make release          # Build and push to registry
make push             # Push to reg.pir9.org:2443/pir9:latest
```

### Docker

```bash
docker compose --profile production up -d     # Production (multi-container)
docker compose -f docker-compose.simple.yml up -d  # Simple single-container
make deploy           # Quick deploy to running containers
```

### Run Modes

```bash
# Standalone (default) - everything in one process
./pir9

# Server mode - web UI + scheduler, uses Redis for distributed scanning
./pir9 --mode server --redis-url redis://localhost:6379

# Worker mode - scan worker only, runs on NAS with local disk access
./pir9 --mode worker --redis-url redis://server:6379 --worker-path /media/tv
```

## Architecture

The codebase follows a layered architecture:

```
Web Layer (src/web/)
    └── REST API + WebSocket + Static Files
API Layer (src/api/)
    └── v3 (legacy) and v5 (current) endpoints
Core Layer (src/core/)
    └── Business logic and domain models
Infrastructure (src/core/datastore/, messaging.rs, scheduler.rs)
    └── Database, Event Bus, Background Jobs
```

### Key Modules

| Module | Purpose |
|--------|---------|
| `core/tv/` | Series, Episodes, domain services and repositories |
| `core/indexers/` | RSS feeds, release parsing, search |
| `core/download/` | qBittorrent, SABnzbd, NZBGet, Transmission clients |
| `core/parser/` | Release title parsing (regex-based) |
| `core/profiles/` | Quality, Language, Delay profiles |
| `core/queue/` | Download queue management |
| `core/mediafiles/` | File import, rename, move operations |
| `core/notifications/` | Discord, Slack, Telegram, Email, Webhook |
| `core/messaging/` | Event bus (in-memory or Redis pub/sub) |
| `core/scheduler.rs` | Background job scheduling |
| `core/scanner/` | Distributed file scanning (worker support) |
| `core/worker.rs` | Scan worker for distributed deployments |

### Frontend

- **frontend/**: Modern frontend using TypeScript, Web Components, Vite, and TanStack Query

## Tech Stack

- **Runtime**: Tokio (async)
- **Web**: Axum
- **Database**: SQLx with SQLite (default) or PostgreSQL
- **Features**: `sqlite` (default), `postgres`, `redis-events`, `torrent`

## Code Patterns

### Error handling
- Use `anyhow` for application errors, `thiserror` for library/module errors
- Structured logging via `tracing` crate

### Async patterns
- Tokio-based async/await throughout
- Lock-free data structures (DashMap) where appropriate
- Event bus for inter-component communication

### Database
- SQLx with compile-time checked SQL queries
- Migrations in `migrations/` directory
- Repository pattern in `core/datastore/`

## Configuration

Priority order:
1. Environment variables (`PIR9_*`)
2. `config/config.toml`
3. Default values

Key environment variables:
- `PIR9_PORT` - Server port (default 8989)
- `PIR9_DB_TYPE` - Database type (sqlite/postgres)
- `PIR9_DB_CONNECTION` - Database connection string
- `PIR9_REDIS_URL` - Redis URL for distributed deployments
- `RUST_LOG` - Logging level (debug, info, warn, error)

## API Compatibility

The API has two versions:
- **v3** (`src/api/v3/`): Legacy compatibility with original Sonarr
- **v5** (`src/api/v5/`): Current API version

When modifying endpoints, maintain backward compatibility in v3.

## Active Work Items

See `TODO-LINT.md` for current lint issues and feature completion status. This tracks:
- Unused imports in API v3 modules (WIP endpoints)
- Dead code analysis by feature area (download queue, history, scheduler jobs, etc.)
- Approach options for addressing warnings

## Code Style Preferences

- **Acronyms in enums**: Keep uppercase (SDTV, DVD, HDTV) - use `#[allow(clippy::upper_case_acronyms)]`
- **Serde casing**: Use `#[serde(rename_all = "camelCase")]` for API types (Sonarr compatibility)
- **Derive order**: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default`

## Adding New Code

### New API endpoint
1. Add route in `src/api/v3/<resource>.rs` or `src/api/v5/<resource>.rs`
2. Response types go in same file or `src/api/v3/models.rs`
3. Business logic goes in `src/core/<domain>/services.rs`
4. Database access goes in `src/core/datastore/repositories.rs`

### New download client
1. Implement trait in `src/core/download/clients.rs`
2. Add variant to `DownloadClientType` enum
3. Register in client factory

### New notification provider
1. Add provider in `src/core/notifications/providers/`
2. Implement `NotificationProvider` trait

## Testing

- Unit tests: Same file in `#[cfg(test)]` module
- Integration tests: `tests/` directory
- Test naming: `test_<function>_<scenario>`

## Versioning & Commits

This project uses semver. **Every commit MUST bump the version.**

### Rules
1. **Read current version** from `Cargo.toml` before committing
2. **Bump the version** based on the change type:
   - `fix:` → patch bump (0.8.0 → 0.8.1)
   - `feat:` → minor bump (0.8.0 → 0.9.0)
   - Breaking changes → major bump (0.8.0 → 1.0.0)
3. **Update both Cargo.toml files** if the change touches pir9-imdb:
   - `Cargo.toml` (main project)
   - `services/pir9-imdb/Cargo.toml` (IMDB service — keep in sync)
4. **Stage the version bump** alongside the code changes in the same commit
5. **Write detailed commit messages** using conventional commits format:
   - First line: `type: short description` (under 72 chars)
   - Blank line, then body explaining what changed and why
   - End with `Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>`

### Current version: 0.3.1

### Commit types
- `feat:` — new feature or capability
- `fix:` — bug fix
- `refactor:` — code restructuring without behavior change
- `docs:` — documentation only
- `chore:` — build, CI, tooling, or maintenance
- `perf:` — performance improvement
- `test:` — adding or fixing tests

## Do NOT

- Add new dependencies without checking if existing crates cover the use case
- Modify v3 API response shapes (breaks Sonarr client compatibility)
- Use `unwrap()` in non-test code - use `?` or `expect("reason")`
- Block async runtime with sync I/O

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `AppState` | `src/web/mod.rs` | Shared state passed to handlers |
| `Series` | `core/tv/models.rs` | Primary domain model |
| `ParsedEpisodeInfo` | `core/parser/mod.rs` | Release parsing result |
| `QualityProfile` | `core/profiles/` | Quality selection rules |
