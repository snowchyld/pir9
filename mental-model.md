# pir9 Mental Model

> Living document — updated as the codebase evolves.
> **Version**: 0.30.0 | **Last updated**: 2026-02-15

## 1. What Is pir9?

A Smart PVR (Personal Video Recorder) for TV shows, anime, and movies — a Rust rewrite of Sonarr. It automates the lifecycle: discover → grab → download → import → rename → organize.

Key differentiator: maintains **Sonarr v3 API compatibility** (frozen response shapes) while adding a modern v5 API, Rust performance, and distributed scanning via Redis.

---

## 2. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (TypeScript, Web Components, TanStack Query, Vite)    │
│  Served via nginx reverse proxy or embedded static files        │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP + WebSocket
┌────────────────────────────▼────────────────────────────────────┐
│  Web Layer — src/web/mod.rs                                     │
│  Axum router + middleware + WebSocket hub + static file serving  │
│  AppState { db, config (RwLock), event_bus, scheduler }         │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│  API Layer — src/api/                                           │
│  v3/ (frozen, Sonarr compat) │ v5/ (current, active dev)       │
│  ~10,900 lines (v3)         │ ~8,500+ lines (v5)               │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│  Core Layer — src/core/                                         │
│  Business logic, domain services, repositories                  │
│  tv/ │ movies/ │ parser/ │ profiles/ │ queue/ │ download/       │
│  indexers/ │ scanner/ │ mediafiles/ │ notifications/             │
│  naming.rs │ scheduler.rs │ messaging/ │ metadata.rs            │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│  Infrastructure                                                  │
│  PostgreSQL (SQLx) │ Redis (optional, events) │ FFmpeg (probing)│
│  External APIs: Skyhook, TVDB, TVMaze, TMDB, IMDB service      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Entry Point & Boot Sequence

**`src/main.rs`** → **`src/cli_args.rs`** (clap) → **`src/web/mod.rs`**

1. Parse CLI args (`--mode`, `--redis-url`, `--worker-path`, `--port`)
2. Load `config/config.toml` → `AppConfig` (wrapped in `parking_lot::RwLock`)
3. Connect to PostgreSQL via SQLx, run migrations
4. Initialize event bus (local broadcast or hybrid Redis)
5. Build `AppState` (shared via `Arc`)
6. Start scheduler (spawns tokio tasks for each job)
7. Start WebSocket hub
8. Build Axum router (v3 + v5 + static files + WebSocket)
9. Bind to port and serve

**Run Modes** (controlled by `--mode`):

| Mode | Web Server | Scheduler | Scanning | Redis |
|------|-----------|-----------|----------|-------|
| **All** (default) | Yes | Yes | Local | Optional |
| **Server** | Yes | Yes | Via Redis workers | Required |
| **Worker** | No | No | Local (publishes results) | Required |

---

## 4. AppState — The Shared Kernel

```rust
pub struct AppState {
    pub db: Database,                              // SQLx pool
    pub config: parking_lot::RwLock<AppConfig>,    // Runtime config
    pub event_bus: Arc<EventBus>,                  // Message broadcast
    pub scheduler: Arc<JobScheduler>,              // Background jobs
}
```

**Critical pattern**: `parking_lot::RwLockReadGuard` is NOT `Send`. Always scope guards in `{ }` blocks before any `.await`:
```rust
let port = { state.config.read().server.port };  // Guard dropped here
do_async_work().await;  // Now safe
```

---

## 5. API Layer

### v3 (Frozen — Sonarr Compatibility)
- **Path**: `src/api/v3/` (~10,900 lines across ~46 modules)
- **Rule**: NEVER change response shapes. Adding fields is OK. Removing/renaming is never OK.
- **Consumers**: Overseerr, Ombi, LunaSea, nzb360, Bazarr, Tdarr
- **Serde**: `#[serde(rename_all = "camelCase")]` on all response types

### v5 (Current — Active Development)
- **Path**: `src/api/v5/` (~34 modules)
- **Pattern**: Handler function → Repository → DB → Response transformation

**v5 modules**: `series.rs`, `episodes.rs`, `episodefile.rs`, `movies.rs`, `queue.rs`, `calendar.rs`, `command.rs`, `system.rs`, `rootfolder.rs`, `health.rs`, `release.rs`, `history.rs`, `blocklist.rs`, `wanted.rs`, `profile.rs`, `quality.rs`, `tag.rs`, `notification.rs`, `indexers.rs`, `download.rs`, `customformat.rs`, `customfilter.rs`, `config.rs`, `settings.rs`, `seasonpass.rs`, `log.rs`, `diskspace.rs`, `update.rs`, `imdb.rs`, `parse.rs`, `manualimport.rs`, `filesystem.rs`, `localization.rs`, `remotepathmapping.rs`

**v3 modules** (~44 files): Mirrors v5 plus legacy endpoints like `autotagging`, `releaseprofile`, `serieseditor`, `serieslookup`, `indexerflag`, `language`, `languageprofile`, `mediacover`, `metadata`, `qualitydefinition`, `importlist`

**Handler pattern**:
```rust
async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());
    let series = repo.get_by_id(id).await?;
    Ok(Json(SeriesResponse::from(series)))
}
```

---

## 6. Core Domain Modules

### 6.1 TV Domain (`core/tv/`)
- **Structure**: `mod.rs`, `models.rs`, `repositories.rs`, `services.rs`, `events.rs`
- **Models**: `SeriesDbModel`, `EpisodeDbModel`, `EpisodeFileDbModel`
- **Services**: CRUD, monitoring, refresh, scan orchestration, TMDB image lookup
- **Events**: `SeriesAddedEvent`, `SeriesUpdatedEvent`, `SeriesDeletedEvent` (typed structs)
- **Key fields**: `tvdb_id` (primary lookup), `imdb_id`, `tmdb_id`, `tv_maze_id`
- **Stats (v0.30.0)**: `episode_count`/`episode_file_count` only count monitored episodes — progress bar shows "what the user cares about". `total_episode_count` is the unfiltered count. Episode file JOIN uses `ef.id = e.episode_file_id` (not season-based join)
- **Year=0 fix (v0.30.0)**: `auto_refresh_series()` fixes `(0)` in series paths when metadata resolves the year. Edit handler also catches `(0)` paths and replaces with real year.

### 6.2 Movies (`core/movies/`)
- **Structure**: `mod.rs`, `models.rs`, `repositories.rs`, `services.rs`, `events.rs`
- **Added v0.9.0**: CRUD API, folder import, IMDB + TMDB integration
- **Models**: `MovieDbModel`, `MovieFileDbModel`
- **Events**: `MovieAddedEvent`, `MovieUpdatedEvent`, `MovieDeletedEvent` (typed structs)
- **TMDB**: Integration is inline in `movies/services.rs` (no separate `tmdb.rs` file)
- **Partial unique indexes**: `tmdb_id WHERE tmdb_id != 0`, `imdb_id WHERE imdb_id IS NOT NULL`
- **Slug routing (v0.30.0)**: Movie URLs use `titleSlug` (e.g., `/movies/babylon-5-thirdspace`) — detail page does slug→ID lookup via movie list
- **Duplicate prevention (v0.30.0)**: `RefreshMovies` pre-checks IMDB/TMDB conflicts before UPDATE — merges duplicates (transfers files, deletes duplicate) instead of hitting PostgreSQL unique constraint errors
- **Status derivation (v0.30.0)**: When status==TBA and year>0, derives from year: past=Released, current=InCinemas, future=Announced
- **Import dedup (v0.30.0)**: `get_by_path()` check before insert prevents duplicate movie records for same folder
- **Bulk stats (v0.30.0)**: `list_movies` uses single `bulk_load_movie_sizes()` query instead of N+1 per-movie queries

### 6.3 Parser (`core/parser/`)
Regex-based release title parser. Priority order:
1. `S01E02` / `S01E02E03` (standard)
2. `1x02` (alternative)
3. `2024.01.15` (daily shows)
4. `S01` / `Season 01` (full season)
5. Absolute episode numbers (anime)

**Title matching**: `best_series_match()` scores candidates 0-100 based on title + year proximity. Used at all match sites (RSS sync, queue reconciliation, import).

**Quality detection**: Resolution (`2160p/1080p/720p/480p`) × Source (`BluRay/WebDL/HDTV/DVD`) + modifiers (REMUX, PROPER, REPACK).

### 6.4 Profiles (`core/profiles/`)
- **QualityProfile**: `cutoff` (weight threshold), `items` (nested JSON with groups), `allowed` list
- **LanguageProfile**: Preferred languages with cutoff
- **DelayProfile**: Usenet/torrent delay in minutes before grabbing

### 6.5 Download System (`core/download/`)
- **Structure**: `mod.rs`, `clients.rs`, `import.rs`, `history.rs`

**Clients** (`clients.rs`):

| Client | Protocol | Auth | Key Detail |
|--------|----------|------|------------|
| **qBittorrent** | Torrent | Form login, session cookie | Auto re-login on 403, extracts info_hash from magnets/torrents |
| **SABnzbd** | Usenet | API key (query param) | Dual queue+history polling |
| **NZBGet** | Usenet | HTTP Basic Auth | JSON-RPC at `/jsonrpc` |
| **Transmission** | Torrent | Optional Basic Auth | CSRF via `X-Transmission-Session-Id` (HTTP 409 retry) |
| **Deluge** | Torrent | Password-only, cookies | JSON-RPC 1.0, requires `ensure_connected()` dance |

**Common trait**:
```rust
pub trait DownloadClient: Send + Sync {
    async fn test(&self) -> Result<()>;
    async fn add_from_url(&self, url: &str, options: DownloadOptions) -> Result<String>;
    async fn add_from_magnet(&self, magnet: &str, options: DownloadOptions) -> Result<String>;
    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>>;
    async fn remove(&self, id: &str, delete_files: bool) -> Result<()>;
    // ... pause, resume, get_files
}
```

**`DownloadClientDbModel` (v0.30.0)**: Added `remove_completed_downloads: bool` and `remove_failed_downloads: bool` fields — previously hardcoded to `true`, now persisted per-client.

**qBittorrent logging (v0.30.0)**: Per-torrent status, GET/POST success paths moved to `trace` level — only login/summary logs at `debug`.

**ImportService** (`import.rs`): Processes completed downloads into the library. Handles single-file downloads AND multi-file season/multi-season packs. Per video file: parse filename → match to episode(s) → analyze media (FFmpeg) → compute file hash (BLAKE3) → rename via naming engine → move to series folder → create `EpisodeFileDbModel` + set `episode.has_file=true`. Returns `ImportResult { success, episode_file_ids, episode_ids, error_message }`.

**History** (`history.rs`): `DownloadHistory` tracking with typed `HistoryEventType` enum: `Grabbed`, `DownloadFailed`, `DownloadFolderImported`, `DownloadIgnored`, `FileImported`, `FileDeleted`, `FileRenamed`. Persisted to `history` table via `HistoryRepository`.

### 6.6 Indexers (`core/indexers/`)
- **Structure**: `mod.rs`, `clients.rs`, `definitions.rs`, `rss.rs`, `search.rs`

**Clients** (`clients.rs`):

| Protocol | Format | Auth |
|----------|--------|------|
| **Newznab** | XML RSS with `<newznab:attr>` | API key (query param) |
| **Torznab** | Same as Newznab + magnet building | API key (query param) |
| **Prowlarr** | Native REST API, JSON | `X-Api-Key` header |

**Search** (`search.rs`): `IndexerSearchService` — interactive/automatic search across all enabled indexers (separate from RSS sync). Iterates indexers with `enable_automatic_search`, aggregates results, continues on per-indexer failure.

**Definitions** (`definitions.rs`): `IndexerDefinition` + `IndexerFieldDefinition` — provider schemas for the settings UI (field types: Text, Number, Boolean, Select, Password, Url, Path).

**RSS** (`rss.rs`): `RssSyncService` — RSS feed fetching (used by scheduler's `execute_rss_sync`).

### 6.7 Queue & Tracked Downloads (`core/queue/`)

**State machine**:
```
Downloading → ImportBlocked → ImportPending → Importing → Imported
           ↘ FailedPending → Failed
```

**`TrackedDownloadService.grab_release()`**:
1. Select best download client matching protocol
2. Prefer magnet links (avoid indexer dependency)
3. Fallback: magnet from info_hash → torrent file download → redirect-following
4. Convert .torrent → magnet via bencoding parser
5. Send to download client, create tracking record

### 6.8 Naming Engine (`core/naming.rs`, ~672 lines)

Character-by-character template scanner (no regex). O(n) single-pass.

```
{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Release Group}]
```

- Selects format by `series.series_type`: 0=standard, 1=daily, 2=anime
- Multi-episode: 0=Extend (`S01E01-E02-E03`), 4=Range (`S01E01-E03`)
- Empty `{Release Group}` inside `[{Release Group}]` auto-strips brackets
- Colon replacement: 0=delete, 1=space, 4=dash

### 6.9 Media Analysis (`core/mediafiles/`)

- **With FFmpeg** (`media-probe` feature): Real stream analysis via `unbundle` crate
- **Without**: Filename pattern matching fallback
- **File hashing**: BLAKE3, 1MB buffer, `spawn_blocking`
- **HDR detection**: `color_transfer` (smpte2084=HDR10, arib-std-b67=HLG) + `color_primaries` (bt2020)
- **Codec normalization**: `h264`→`x264`, `hevc`→`x265`

### 6.10 Scanner (`core/scanner/`)

- `mod.rs` — Core scanning logic (directory walk, video extension filter)
- `jobs.rs` — JobTrackerService (timeout, retries)
- `consumer.rs` — ScanResultConsumer (imports results from workers)
- `registry.rs` — WorkerRegistryService (tracks online workers, heartbeats)

**Video extensions**: `mkv, mp4, avi, wmv, m4v, ts, webm, mov`

### 6.11 Notifications (`core/notifications/`)
- **Structure**: `mod.rs`, `providers.rs`, `service.rs`

**Providers** (`providers.rs`, v0.18.0): Webhook + Slack only (Discord/Telegram/Pushover/Email removed).

**Service** (`service.rs`): `NotificationService` subscribes to event bus, filters by notification settings, dispatches to enabled providers.

**Events**: Grab, Download, Upgrade, Rename, SeriesAdd/Delete, HealthIssue, etc.

### 6.12 Other Core Files
- **`logging.rs`** — App-level log persistence to `logs` table (separate from tracing/stdout)
- **`metadata.rs`** — `MetadataService` — unified metadata lookup orchestrating IMDB + TVMaze enrichment
- **`tvmaze.rs`** — `TvMazeClient` — fallback metadata when Skyhook returns null for network/status
- **`imdb.rs`** — `ImdbClient` — HTTP client for pir9-imdb microservice (search, ratings, genres)

---

---

## 6B. CLI Tool (`src/cli.rs`)

Standalone HTTP API client for managing a running pir9 server from the command line.

```
pir9-cli --url http://localhost:8989 --api-key KEY <command>
```

**Commands**:
- `series list [--format table|json]`, `series add --tvdb-id N`, `series delete`, `series refresh`, `series search`
- `system status`, `system health`, `system diskspace`, `system backup`, `system restore`, `system clear-logs`
- `config show`, `config set KEY VALUE`, `config validate` (connectivity test)

**Internals**: `ApiClient` struct with `get()/post()/put()/delete()`, `X-Api-Key` header auth. Env vars: `PIR9_URL` (default `http://localhost:8989`), `PIR9_API_KEY`.

---

## 6C. Shared API Models (`src/api/models.rs`)

- `ApiResponse<T>` — standard wrapper: `{ success, data, error }`
- `ApiError` — `{ code, message, details }`
- `PaginationParams` — `{ page, page_size, sort_key, sort_direction }` (defaults: page=1, page_size=20)

---

## 6D. Configuration (`src/core/configuration.rs`)

`AppConfig` with `validator::Validate` support:

| Section | Struct | Key Fields |
|---------|--------|------------|
| `[server]` | `ServerConfig` | `port`, `bind_address`, `enable_ssl`, `request_timeout_secs`, `max_body_size_mb` |
| `[database]` | `DatabaseConfig` | `database_type`, `connection_string`, `max_connections`, `connection_timeout_secs` |
| `[security]` | `SecurityConfig` | `secret_key`, `enable_authentication`, `authentication_method`, `session_timeout_hours` |
| `[download]` | `DownloadConfig` | `check_interval_secs`, `enable_completed_download_handling`, `remove_completed_downloads` |
| `[media]` | `MediaConfig` | `episode_naming_pattern`, `season_folder_format`, `colon_replacement_format`, `rename_episodes` |
| `[notifications]` | `NotificationConfig` | `enabled`, `providers` |
| `[paths]` | `PathConfig` | `config_dir`, `data_dir`, `log_dir`, `backup_dir` |
| `[redis]` | `RedisConfig` (optional) | `url`, `use_for_events`, `database`, `connection_timeout_secs` |

`MediaConfig` has a `Default` impl — used as fallback in scheduler/command paths via `.unwrap_or_default()`.

---

## 7. Event System (`core/messaging/`)
- **Structure**: `mod.rs` (local EventBus + Message enum), `redis_bus.rs` (HybridEventBus)

**Local mode** (`mod.rs`): Tokio `broadcast::Sender<Message>` (capacity 1000)

**Distributed mode** (`redis_bus.rs`, `redis-events` feature): `HybridEventBus` publishes to both local broadcast AND Redis `pir9:events` channel. Instance ID prevents echo.

**Key message types**:
- Command lifecycle: `CommandStarted/Updated/Completed/Failed`
- Series: `SeriesAdded/Updated/Deleted/Refreshed/Scanned`
- Episodes: `EpisodeAdded/Updated/FileImported/FileDeleted`
- Downloads: `ReleaseGrabbed`, `DownloadStarted/Completed/Failed`
- Movies: `MovieAdded/Updated/Deleted/Refreshed/FileImported`
- System: `QueueUpdated`, `HealthCheckChanged`, `ConfigUpdated`
- Distributed: `ScanRequest/ScanResult`, `WorkerOnline/Offline/Heartbeat`

---

## 8. Scheduler (`core/scheduler.rs`, ~782 lines)

| Job | Interval | Purpose |
|-----|----------|---------|
| RssSync | 15 min | Fetch RSS → parse → match → quality check → grab |
| ProcessDownloadQueue | 1 min | Poll clients, update statuses, trigger imports |
| RefreshSeries | 6 hrs | Update metadata from TVDB/IMDB/TVMaze |
| HealthCheck | 5 min | Test all clients/indexers/disk space |
| Housekeeping | 24 hrs | Cleanup old commands, VACUUM ANALYZE |
| Backup | Weekly | `pg_dump` → `/config/Backups/` (keep last 7) |
| DownloadedEpisodesScan | On-demand | Import completed files |

### RSS Sync Pipeline (the core automation)
```
Fetch RSS from all enabled indexers (100 per indexer)
  ↓
Pre-load: monitored series, quality profiles (HashMap), active downloads (HashSet)
  ↓
For each release:
  parse_title() → best_series_match() → quality profile check
  → find wanted episodes (monitored, missing, aired, not downloading)
  → TrackedDownloadService::grab_release()
```

---

## 9. Database

### PostgreSQL (production) — SQLx with compile-time checked queries

**Key tables**: `series`, `episodes`, `episode_files`, `episode_file_mapping`, `movies`, `movie_files`, `indexers`, `download_clients`, `notifications`, `quality_profiles`, `language_profiles`, `delay_profiles`, `tags`, `root_folders`, `tracked_downloads`, `commands`, `logs`

**Two migration directories** (root `migrations/` has legacy names, `migrations/postgres/` is authoritative):

`migrations/postgres/` (7 files — these are what SQLx runs):
1. `001_initial_schema.sql` — Core tables (series, episodes, files, profiles, clients, indexers, download_clients)
2. `002_schema_sync.sql` — Schema alignment and sync fixes
3. `003_additional_tables.sql` — Supplementary tables (IMDB cache, etc.)
4. `004_imdb_enrichment.sql` — IMDB data enrichment columns
5. `005_movies.sql` — Movie domain tables (movies, movie_files)
6. `006_file_hash.sql` — `file_hash` column on episode_files and movie_files (BLAKE3)
7. `007_fix_timestamp_types.sql` — Fix `TIMESTAMP` → `TIMESTAMPTZ` for DateTime<Utc> columns

**Critical**: Always use `TIMESTAMPTZ` for `DateTime<Utc>` columns. Migration 001 used `TIMESTAMP` (causes runtime decode errors with non-empty results).

### Repository Pattern
```rust
pub struct SeriesRepository { db: Database }
impl SeriesRepository {
    pub async fn get_by_id(&self, id: i64) -> Result<SeriesDbModel>;
    pub async fn get_all(&self) -> Result<Vec<SeriesDbModel>>;
    pub async fn insert(&self, model: &SeriesDbModel) -> Result<SeriesDbModel>;
    pub async fn update(&self, model: &SeriesDbModel) -> Result<()>;
    pub async fn delete(&self, id: i64) -> Result<()>;
}
```

---

## 10. Frontend

### Stack
- **Components**: Web Components (custom elements, light DOM, no framework)
- **State**: Minimal signal system (~1KB) + TanStack Query v5
- **Router**: Navigo v8 (hash-less SPA routing)
- **Build**: Vite 6, TypeScript 5.7, Tailwind CSS 4.0
- **Lint**: Biome 2.x

### Architecture
```
frontend/src/
├── main.ts                    # Boot: styles, WebSocket, router, component registration
├── app.ts                     # <app-root> — sidebar + header + outlet + toasts
├── router.ts                  # Navigo routes → <router-outlet>
├── core/
│   ├── component.ts           # BaseComponent class, @customElement, @reactive decorators
│   ├── reactive.ts            # signal(), computed(), effect(), batch(), persistedSignal()
│   ├── http.ts                # Fetch wrapper (v5 + v3), type definitions
│   ├── query.ts               # TanStack Query: useSeriesQuery(), useMoviesQuery(), etc.
│   └── websocket.ts           # Auto-reconnect WebSocket → query invalidation
├── stores/
│   ├── app.store.ts           # UI state: sidebar, modals, toasts, view modes, sort/filter
│   └── theme.store.ts         # Dark/light theme (CSS variables, localStorage)
├── styles/
│   ├── base.css               # Tailwind + animations + component layer
│   └── themes.css             # CSS custom properties for dark/light
├── components/
│   ├── primitives/            # 16 UI components (button, input, dialog, table, badge, etc.)
│   ├── layout/                # 5 layout components (sidebar, header, outlet, toast, modal)
│   └── release-search-modal.ts
└── features/                  # ~45 page components
    ├── series/                # index, detail, edit dialog, match dialog
    ├── movies/                # index, detail
    ├── add-series/            # add, import
    ├── add-movie/             # add, import
    ├── activity/              # queue, history, blocklist
    ├── wanted/                # missing, cutoff unmet
    ├── calendar/              # calendar page
    ├── settings/              # 14 settings pages + provider dialogs
    └── system/                # status, tasks, backup, updates, events, logs
```

### Data Flow
```
Page component → useSeriesQuery() → TanStack Query cache
  ↓ (cache miss)
http.get('/api/v5/series') → Backend API
  ↓ (response)
Signal updates → BaseComponent.requestUpdate() → re-render

WebSocket message → wsManager.on('series_refreshed')
  → invalidateQueries(['/series'])
  → Query refetch → Signal update → re-render
```

### Design System
- **Aesthetic**: Glassmorphism (backdrop-filter blur, semi-transparent backgrounds)
- **Theming**: CSS custom properties, dark theme default
- **Colors**: Primary blue (#5d9cec), protocol colors (torrent=#00853d, usenet=#17b1d9)

### Frontend Patterns (v0.30.0)
- **Slug routing**: Movie and series detail pages use `titleSlug` in URLs, do slug→ID lookup on mount via list query
- **Column sorting**: Movies table headers are clickable — `handleColumnSort()` toggles direction or changes sort key, uses `safeHtml()` for SVG sort icons
- **Rating display**: `formatRating()` prefers `ratings.value` over `imdbRating` field

---

## 11. External Services

| Service | Purpose | Config |
|---------|---------|--------|
| **Skyhook** | Primary series metadata (TVDB proxy) | Built-in URL |
| **TVMaze** | Network fallback when Skyhook returns null | `core/tvmaze.rs` |
| **IMDB** | Ratings, votes, genres (via pir9-imdb microservice) | `PIR9_IMDB_SERVICE_URL` |
| **TMDB** | Movie metadata + poster/backdrop images | `PIR9_TMDB_API_KEY` |
| **GitHub** | Update checks (`/repos/pir9/pir9/releases/latest`) | Built-in |

### pir9-imdb Microservice (`services/pir9-imdb/`)
- Standalone Rust service (v0.5.2), own PostgreSQL DB (port 5433)
- Syncs IMDB non-commercial datasets (~3.5GB, TSV.gz files)
- Resumable via `last_processed_id` (IMDB IDs are monotonically increasing)
- Batch upserts (UNNEST, `BATCH_SIZE=1000`), cancellable via `CancellationToken`
- API: search series/movies, get episodes, trigger/cancel sync, stats
- **Fuzzy search (v0.5.2)**: `regexp_replace(title, '[^a-zA-Z0-9 ]', ' ', 'g')` fallback handles punctuation differences (e.g., "Mortal Instruments: City" matches "Mortal Instruments City"). Query normalized on Rust side too.

---

## 12. Deployment & Infrastructure

### Docker (4-stage cargo-chef build)
1. **Chef** — install cargo-chef
2. **Planner** — analyze deps → `recipe.json`
3. **Builder** — cook deps (cached) → build binary (FFmpeg dev libs needed)
4. **Runtime** — debian:bookworm-slim + FFmpeg shared libs + gosu

### Docker Compose Profiles

| Profile | File | Services | DB | Redis |
|---------|------|----------|----|-------|
| **Simple** | `docker-compose.simple.yml` | 1 (pir9) | SQLite | No |
| **Production** | `docker-compose.yml` | 6 (frontend, api, postgres, postgres-imdb, redis, imdb) | PostgreSQL | Yes |
| **Worker** | `docker-compose.synology-worker.yml` | 1 (pir9-worker) | None | Client |

### Makefile Key Targets

| Target | What It Does |
|--------|-------------|
| `make` (default) | `release-restart` = build frontend + docker build + push to registry + restart |
| `make dev-api` | `cargo build --release` |
| `make dev-frontend` | `npm install && npm run build` |
| `make watch-frontend` | Vite dev server with hot reload |
| `make release` | Docker build + push to `reg.pir9.org:2443/pir9:latest` |
| `make deploy` | Copy binary/frontend to running containers (no rebuild) |
| `make test` | `cargo test` |
| `make lint` | `cargo clippy -- -D warnings` + `npm run lint` |

### Distributed Architecture
```
┌──────────────┐          ┌──────────────┐
│ Server       │          │ Synology NAS │
│ (--mode      │  Redis   │ (--mode      │
│  server)     │◄────────►│  worker)     │
│ Web + Sched  │  pub/sub │ Scans local  │
│ Port 8989    │          │ /volume1/    │
└──────┬───────┘          └──────────────┘
       │
  ┌────▼─────┐
  │ PostgreSQL│
  │ Port 5434 │
  └──────────┘
```

---

## 13. Configuration

**Priority**: CLI args > env vars (`PIR9_*`) > `config/config.toml` > defaults

**Implementation**: `src/core/configuration.rs` → `AppConfig` struct (see section 6D for full detail).

**Key env vars**: `PIR9_PORT`, `PIR9_DB_TYPE`, `PIR9_DB_CONNECTION`, `PIR9_REDIS_URL`, `PIR9_IMDB_SERVICE_URL`, `PIR9_TMDB_API_KEY`, `RUST_LOG`

**Config updates**: Must update BOTH disk (`config.save()`) AND in-memory (`state.config.write()`) — otherwise settings revert on next GET.

---

## 14. Quality & Safety Pipeline

### Pre-commit Hooks (10 total)
**Fast**: gitleaks, cargo fmt, cargo clippy, biome check, ruff
**Security**: cargo audit, semgrep, grype (via SBOM)
**Advisory**: ubs (Rust + JS bug scanner)
**SBOM**: syft → `sbom.cdx.json`

### Claude Code PostToolUse Hooks
- `check-biome.sh` — Runs on `frontend/src/*.ts` edits
- `check-security.sh` — Runs semgrep + cargo audit on relevant files

---

## 15. Key Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Repository pattern | `core/datastore/` | Testable, compile-time SQL checking |
| Event-driven | `core/messaging/` | Loose coupling, distributed support |
| Feature flags | `Cargo.toml` | Optional Redis, FFmpeg, torrent support |
| `anyhow` + `thiserror` | Everywhere | Application vs library errors |
| `parking_lot::RwLock` | AppState.config | Non-deadlocking, faster than std |
| `spawn_blocking` | FFmpeg, file hashing | Keep async runtime free |
| `camelCase` serde | All API types | Sonarr compatibility |
| Partial unique indexes | movies table | Allow IMDB-only or TMDB-only imports |

---

## 16. Known Gotchas

1. **TIMESTAMP vs TIMESTAMPTZ**: Migration 001 used `TIMESTAMP`. SQLx `DateTime<Utc>` requires `TIMESTAMPTZ`. Empty results pass silently — errors only appear with actual data.
2. **parking_lot guards + async**: Must drop `RwLockReadGuard` before any `.await` or Axum rejects the handler.
3. **v3 API shapes**: Adding fields OK. Changing/removing NEVER OK. External clients break silently.
4. **Image cache-busting**: Append `?t={timestamp}` to MediaCover URLs after rematch/refresh.
5. **MediaCover route order**: `/MediaCover/Movies/{id}/{file}` must be defined BEFORE `/MediaCover/{series_id}/{file}` (literal "Movies" vs i64 capture).
6. **pre-commit `pass_filenames: false`**: Checks ENTIRE project, not just staged files. Pre-existing debt blocks ALL commits.
7. **Wanted API**: Always include series data with `titleSlug` — frontend crashes silently on `null.titleSlug`.
8. **Docker `read_only: true`**: Incompatible with runtime `useradd` — bake user at build time.
