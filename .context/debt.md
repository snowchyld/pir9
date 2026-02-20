# Technical Debt Registry

Known technical debt, feature gaps, and intentional shortcuts. Helps AI understand what exists, what's missing, and what the proper solution would be.

## How to Use This File

- **Before generating code**: Check if your work area has documented debt — avoid compounding it
- **When finding debt**: Add an entry with a unique ID and reference it in code: `// DEBT-XXX: brief description`
- **When fixing debt**: Move the entry to "Resolved Debt" with the resolution date

## Lint Status

**Clippy: 0 errors** — all warnings resolved as of v0.10.2.
```bash
cargo clippy -- -D warnings    # Clean
cd frontend && npm run lint    # Clean (Biome 2.x)
```

---

## Active Debt

### HIGH Priority

#### DEBT-001: Episode Cutoff Quality Comparison
- **Location**: `src/core/datastore/repositories.rs`
- **Description**: Episode queries return episodes with files but don't compare current quality against the profile cutoff
- **Why it exists**: Quality profile comparison requires joining quality_profiles and computing weight thresholds — deferred for simpler initial implementation
- **Risk**: Episodes below cutoff aren't flagged for upgrade, undermining the quality-based acquisition pipeline
- **Proper solution**: Join episodes with quality_profiles, compare file quality weight against profile cutoff weight, expose via `/api/v5/wanted/cutoff` endpoint
- **Effort**: Medium (2-3 days)

### MEDIUM Priority

#### DEBT-002: Movie Refresh Missing Metadata Fetch
- **Location**: `src/core/movies/services.rs:75`
- **Description**: Movie refresh endpoint exists but doesn't fetch updated info from IMDB/TMDB
- **Why it exists**: Import pipeline was prioritized over refresh; movies change metadata less frequently than series
- **Risk**: Stale ratings, plot summaries, and poster images over time
- **Proper solution**: Call pir9-imdb or TMDB API during refresh, update changed fields, re-cache images
- **Effort**: Medium (1-2 days)

#### DEBT-003: Config Changes Not Persisted
- **Location**: `src/api/v5/config.rs:76`
- **Description**: Configuration changes made via API aren't written back to `config.toml`
- **Why it exists**: Read path was implemented first; write path requires TOML serialization and file locking
- **Risk**: Config changes lost on restart; user confusion
- **Proper solution**: Persist to `config.toml` with file locking, emit `ConfigUpdated` event for hot-reload
- **Effort**: Low (1 day)

#### DEBT-004: Queue Re-grab Not Implemented
- **Location**: `src/api/v5/queue.rs:699`
- **Description**: Can't re-grab a previously removed or failed release from the queue
- **Why it exists**: Initial queue focused on forward flow (grab → download → import), not retry
- **Risk**: User must manually re-search and re-grab failed downloads
- **Proper solution**: Store original release info in tracked_downloads, implement re-grab endpoint that replays the grab
- **Effort**: Low-Medium (1-2 days)

### LOW Priority

#### DEBT-005: CLI Subcommands Are Stubs
- **Location**: `src/cli.rs:224-299`
- **Description**: All 14 CLI commands (series list/add/delete, system backup/restore, config show/set) are logging stubs that print a message and exit
- **Why it exists**: CLI was scaffolded for future use; web UI is the primary interface
- **Risk**: None for typical users; power users and scripting are blocked
- **Proper solution**: Wire CLI commands to the same service layer the API uses
- **Effort**: Medium (3-5 days for all 14 commands)

#### DEBT-006: Search Category Parsed But Discarded
- **Location**: `src/core/indexers/clients.rs`
- **Description**: Indexer search category is parsed from responses but not wired through to search requests
- **Why it exists**: Category filtering was lower priority than basic search functionality
- **Risk**: Slightly less precise search results from indexers
- **Proper solution**: Thread category parameter through to indexer API requests (Newznab `cat=` parameter)
- **Effort**: Low (half day)

#### DEBT-007: Filename-Only Media Analysis
- **Location**: `src/core/mediafiles/mod.rs`
- **Description**: Media file quality detection is based entirely on filename parsing — no actual file inspection
- **Why it exists**: Filename parsing covers 95%+ of cases; `mediainfo` binary adds a system dependency
- **Risk**: Misidentified quality for files with non-standard or ambiguous filenames
- **Proper solution**: Optionally integrate `mediainfo` binary for deeper metadata extraction (resolution, codec, bitrate)
- **Effort**: Medium (2-3 days, plus optional dependency management)

---

## Resolved Debt

| ID | Description | Resolution | Date |
|----|-------------|------------|------|
| — | All clippy warnings (355 total) | Resolved across v0.8.x–v0.10.2 | 2026-02 |
| — | Download History | Fully implemented (`record_grab`, `record_download_failed`, `record_import`) | 2026-01 |
| — | Download Queue | Fully implemented via `TrackedDownloadService` | 2026-01 |
| — | Scheduler Jobs | 5 of 6 implemented (RefreshSeries, DownloadedEpisodesScan, Housekeeping, HealthCheck, Backup) | 2026-01 |
| — | Notifications dispatch | Event listener and provider dispatch working | 2026-01 |
| — | Download Clients API | Handlers wired and functional | 2026-01 |
| — | Frontend linting | Migrated to Biome 2.x (v0.10.1) | 2026-02 |
| — | RSS Sync auto-grab | Full pipeline: RSS → parse → match → quality check → grab (v0.11.0) | 2026-02 |

---

## Notes

- The codebase follows a **framework-first approach** — infrastructure is solid, implementations fill in over time
- API v3 maintains Sonarr compatibility — don't remove endpoints even if unimplemented
- The highest-impact remaining gap is **episode cutoff comparison** (DEBT-001) — this determines which episodes need quality upgrades
