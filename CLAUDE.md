# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

pir9 is a Smart PVR (Personal Video Recorder) for TV and anime, written in Rust. It's a rewrite of Sonarr that maintains API compatibility while leveraging Rust's performance and safety.

## Project Documentation

Structured documentation lives in `.context/` (tool-agnostic) and `.claude/skills/` (Claude Code workflows):

| File | Purpose |
|------|---------|
| `.context/glossary.md` | Domain terminology (PVR, quality profiles, cutoff, indexer, etc.) |
| `.context/boundaries.md` | What's frozen, what's freely modifiable, ownership map |
| `.context/anti-patterns.md` | Rust + pir9-specific patterns to avoid |
| `.context/debt.md` | Technical debt registry with priorities and proper solutions |
| `.context/events.md` | Event bus catalog — all published events and subscribers |
| `.context/decisions/` | Architecture Decision Records (ADRs) with rationale |

Claude Code skills in `.claude/skills/` provide guided workflows: `/rust-api`, `/download-client`, `/parser-dev`, `/frontend-component`, `/frontend-store`, `/db-migration`, `/docker-infra`, `/security-audit`, `/trace`, `/release`.

## Build & Development Commands

```bash
# Build
cargo build --release                    # Release build (includes postgres + redis-events)

# Test
cargo test                               # Run all tests
cargo test test_name                     # Run specific test
cargo test -- --nocapture                # Show test output

# Lint
cargo clippy -- -D warnings              # Rust linting
cd frontend && npm run lint              # Frontend linting (Biome)
cd frontend && npm run lint:fix          # Auto-fix frontend lint issues
cd frontend && npm run typecheck         # TypeScript type checking

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
make push             # Push to nas.drew.red:2443/pir9:latest
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
- **Linting**: Biome (linter + formatter) — config in `frontend/biome.json`

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

## Code Style Preferences

- **Acronyms in enums**: Keep uppercase (SDTV, DVD, HDTV) - use `#[allow(clippy::upper_case_acronyms)]`
- **Serde casing**: Use `#[serde(rename_all = "camelCase")]` for API types (Sonarr compatibility)
- **Derive order**: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default`

See `.context/anti-patterns.md` for patterns to avoid.

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

### Current version: 0.91.0

### Commit types
- `feat:` — new feature or capability
- `fix:` — bug fix
- `refactor:` — code restructuring without behavior change
- `docs:` — documentation only
- `chore:` — build, CI, tooling, or maintenance
- `perf:` — performance improvement
- `test:` — adding or fixing tests

## Frontend Linting

After modifying any frontend TypeScript files, **always run linting**:

```bash
cd frontend && npm run lint              # Check for issues
cd frontend && npm run lint:fix          # Auto-fix issues
cd frontend && npm run typecheck         # Verify types
```

Biome enforces:
- Import sorting (alphabetical)
- No unused imports or variables
- `const` over `let` when variable is never reassigned
- No implicit `any` types (warning)
- No implicit return values in `forEach` callbacks — use block body `{ }`
- Consistent formatting (2-space indent, single quotes, trailing commas)

## Pre-commit Hooks

Pre-commit runs 10 hooks on every commit (`.pre-commit-config.yaml`):

| Tier | Hook | Trigger | Purpose |
|------|------|---------|---------|
| Fast | gitleaks | all files | Secret detection |
| Fast | cargo fmt | `*.rs` | Rust formatting |
| Fast | cargo clippy | `*.rs` | Rust linting (`-D warnings`) |
| Fast | biome check | `frontend/src/**` | TS lint + format |
| Fast | ruff + ruff-format | `*.py` | Python lint (if Python files exist) |
| Security | cargo audit | `Cargo.{toml,lock}` | CVEs in Rust dependencies |
| Security | semgrep | all files | SAST (auto registry rules) |
| Security | grype | lockfiles | CVE scan across all ecosystems |
| Security | ubs | all files | Rust + JS deep bug scanning |
| SBOM | syft | lockfiles | CycloneDX SBOM → `sbom.cdx.json` |

```bash
pre-commit install                       # Install hooks (one-time)
pre-commit run --all-files               # Run all hooks manually
pre-commit run <hook-id> --all-files     # Run a specific hook
```

## Claude Code Hooks

PostToolUse hooks in `.claude/hooks/` run after every Edit/Write and feed context back:

| Hook | Scope | Output |
|------|-------|--------|
| `check-biome.sh` | `frontend/src/*.ts` | Biome lint findings |
| `check-security.sh` | all files | Semgrep findings + cargo audit (on `Cargo.toml`) |

Hooks produce **zero output on clean files** — findings only, capped at 20 lines.

## Do NOT

- Add new dependencies without checking if existing crates cover the use case
- Modify v3 API response shapes (breaks Sonarr client compatibility)
- Use `unwrap()` in non-test code - use `?` or `expect("reason")`
- Block async runtime with sync I/O

See `.context/boundaries.md` for full ownership map and modification guidelines.

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `AppState` | `src/web/mod.rs` | Shared state passed to handlers |
| `Series` | `core/tv/models.rs` | Primary domain model |
| `ParsedEpisodeInfo` | `core/parser/mod.rs` | Release parsing result |
| `QualityProfile` | `core/profiles/` | Quality selection rules |

## Task Completion

When a task is complete, **always do both** before moving on:

1. **Document**: Update `mental-model.md` with any new patterns, behaviors, or architectural changes discovered during the task
2. **Commit**: Stage all changes and create a commit following the versioning rules above

Never leave deployed-but-uncommitted work. If `make` succeeded, the code should be committed.
