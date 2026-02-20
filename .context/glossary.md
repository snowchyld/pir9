# pir9 Glossary

Project-specific terminology for the pir9 Smart PVR. Use these terms consistently in code, comments, and documentation.

## PVR Domain

| Term | Definition | Not to be confused with |
|------|------------|------------------------|
| **PVR** | Personal Video Recorder — software that automates TV show acquisition | DVR (hardware-based recording) |
| **Series** | A TV show with seasons and episodes | Movie (standalone, no episodes) |
| **Episode** | A single installment of a series, identified by season + episode number | Episode File (the media file on disk) |
| **Season** | Ordered grouping of episodes within a series | Series (the entire show) |
| **Quality Profile** | Rules defining which video qualities are acceptable and preferred | Quality (a single resolution/source combo) |
| **Cutoff** | The quality threshold in a profile — once met, stop upgrading | Minimum quality (lowest acceptable) |
| **Root Folder** | Top-level directory where series or movie folders live (e.g., `/media/tv/`) | Series folder (e.g., `/media/tv/Breaking Bad/`) |
| **Indexer** | A search provider that returns available releases (Newznab, Torznab) | Download client (which actually downloads) |
| **Release** | A specific uploaded version of an episode/movie (e.g., `Show.S01E05.720p.HDTV-GROUP`) | Episode (the logical content, not a file) |
| **Release Group** | The team/person that encoded and uploaded a release (e.g., `GROUP` in `-GROUP`) | Source (where the video came from) |
| **Grab** | The act of sending a release to a download client for acquisition | Download (the transfer itself) |
| **Import** | Moving a completed download into the organized library with proper naming | Grab (which initiates the download) |
| **Wanted** | An episode that is monitored and doesn't yet have an acceptable file | Missing (similar, but "wanted" implies actively seeking) |
| **Monitored** | A series/episode that pir9 actively seeks releases for | Unmonitored (exists in library but not actively sought) |
| **Unmapped** | A folder on disk that doesn't correspond to any known series or movie | Missing (episode exists in DB but no file on disk) |

## Quality Tiers

Listed from lowest to highest weight:

| Quality | Source | Typical Resolution |
|---------|--------|--------------------|
| **SDTV** | TV capture | 480p or below |
| **DVD** | DVD rip | 480p |
| **WEBDL480p** | Web stream | 480p |
| **HDTV720p** | HD TV capture | 720p |
| **HDTV1080p** | HD TV capture | 1080p |
| **WEBDL720p** | Web stream | 720p |
| **WEBDL1080p** | Web stream | 1080p |
| **Bluray720p** | Blu-ray encode | 720p |
| **Bluray1080p** | Blu-ray encode | 1080p |
| **WEBDL2160p** | Web stream | 4K |
| **Bluray2160p** | Blu-ray encode | 4K |
| **Remux1080p** | Blu-ray remux | 1080p (lossless) |
| **Remux2160p** | Blu-ray remux | 4K (lossless) |

## Architecture Terms

| Term | Definition | Location |
|------|------------|----------|
| **AppState** | Shared application state passed to all Axum handlers via `State<Arc<AppState>>` | `src/web/mod.rs` |
| **Event Bus** | Pub/sub system for inter-component communication (in-memory or Redis) | `src/core/messaging/` |
| **Handler** | HTTP request handler — receives request, calls service, returns response | `src/api/v3/`, `src/api/v5/` |
| **Service** | Business logic layer — orchestrates operations, enforces domain rules | `src/core/<domain>/services.rs` |
| **Repository** | Data access abstraction — encapsulates SQL queries, returns domain models | `src/core/datastore/repositories.rs` |
| **DbModel** | Database-mapped struct (suffixed with `DbModel`) | `src/core/datastore/models.rs` |
| **Tracked Download** | A download being monitored from grab through import or failure | `src/core/queue/` |
| **Scheduler** | Background job runner using cron-like scheduling | `src/core/scheduler.rs` |
| **Scanner** | File system scanner that discovers media files in root folders | `src/core/scanner/` |
| **Worker** | Remote scan worker deployed on NAS for distributed file scanning | `src/core/worker.rs` |

## API Terms

| Term | Definition |
|------|------------|
| **v3 API** | Legacy Sonarr-compatible API — response shapes are **frozen** |
| **v5 API** | Current pir9-native API — freely evolvable |
| **Command** | An async operation triggered via API (refresh series, search, backup) |
| **Lookup** | Searching external metadata sources (TVDB, IMDB, TMDB) for series/movie info |

## Deployment Modes

| Mode | Description | Use case |
|------|-------------|----------|
| **Standalone** | Everything in one process (default) | Single-machine setup |
| **Server** | Web UI + scheduler, uses Redis for messaging | Central management node |
| **Worker** | Scan worker only, connects to server via Redis | NAS with local disk access |

## Abbreviations

| Abbreviation | Full Form |
|--------------|-----------|
| **IMDB** | Internet Movie Database |
| **TMDB** | The Movie Database |
| **TVDB** | The TV Database |
| **RSS** | Really Simple Syndication (used for new release feeds from indexers) |
| **NZB** | Newzbin file format (Usenet download descriptor) |
| **ADR** | Architecture Decision Record |
