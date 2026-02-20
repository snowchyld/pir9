# Pir9 Implementation TODO

## Overview

This document tracks the implementation status of Pir9 compared to the original Sonarr.
Current completion: ~80% (Phase 1-5 complete: infrastructure, download clients, indexers, queue/import, wanted/calendar)

---

## Phase 1: Core Infrastructure (Foundation) ✅

### 1.1 Logging System ✅
- [x] Create `logs` database table (migration: 002_logs.sql)
- [x] Implement `LogRepository` for CRUD operations
- [x] Create logging service (`AppLogger` in core/logging.rs)
- [x] Update `/api/v5/log` endpoint to query database
- [ ] Add log rotation/cleanup scheduled task
- [x] Wire up logging calls throughout codebase (commands, file ops)

### 1.2 Health Checks ✅
- [x] Implement actual health check logic in `/api/v5/health`
- [x] Check download client configuration
- [x] Check indexer configuration
- [x] Check root folder accessibility
- [x] Check disk space warnings (using libc::statvfs on Unix)
- [x] Check for series with no root folder

### 1.3 File Operations Completion ✅
- [x] Implement actual file deletion in `DELETE /episodefile`
- [x] Implement bulk file deletion
- [x] Calculate `size_on_disk` for series/episodes
- [x] Implement rename preview (`/api/v3/rename`)
- [ ] Implement actual file renaming (requires command support)

---

## Phase 2: Download Client Integration ✅

### 2.1 Download Client Framework ✅
- [x] Implement `DownloadClientService` with real HTTP calls (core/download/clients.rs)
- [x] Add connection testing with actual API validation
- [x] Implement download status tracking
- [x] Store download client configs in database (DownloadClientRepository)
- [x] API CRUD operations with persistence (/api/v3/downloadclient)
- [x] Schema endpoint for available client types

### 2.2 qBittorrent Integration ✅
- [x] Implement qBittorrent Web API v2 client
- [x] Add torrent submission (URL, magnet, file upload)
- [x] Get download status/progress
- [x] Session management with cookie auth
- [x] Category/label management
- [x] Pause/resume/remove operations

### 2.3 SABnzbd Integration ✅
- [x] Implement SABnzbd API client
- [x] Add NZB submission (URL, file upload)
- [x] Get download status/progress (queue + history)
- [x] API key authentication
- [x] Category management
- [x] Pause/resume/remove operations

### 2.4 Queue API ✅
- [x] Queue shows real downloads from all clients
- [x] Queue status with error/warning counts
- [x] Remove downloads from queue/client
- [ ] Match downloads to series/episodes (pending)

### 2.5 Other Clients (Lower Priority)
- [ ] Deluge
- [ ] Transmission
- [ ] NZBGet
- [ ] rTorrent

---

## Phase 3: Indexer/Search Integration ✅

### 3.1 Newznab Protocol ✅
- [x] Implement Newznab API client (core/indexers/clients.rs)
- [x] Caps endpoint parsing (capabilities, categories, limits)
- [x] Search endpoint (t=tvsearch, t=search)
- [x] Parse XML results into ReleaseInfo
- [x] Quality parsing from release titles
- [x] TVDB/IMDB ID support

### 3.2 Torznab Protocol ✅
- [x] Implement Torznab API client (extends Newznab)
- [x] Handle magnet links (build from info_hash)
- [x] Handle .torrent files (download URLs)
- [x] Seeder/leecher tracking

### 3.3 Search Service ✅
- [x] Implement `IndexerSearchService.search_indexer()` with real queries
- [x] Implement interactive search (/api/v3/release)
- [x] Quality parsing from titles
- [x] Sort by quality weight
- [ ] Custom format scoring (pending)
- [ ] Result deduplication (pending)

### 3.4 RSS Sync ✅
- [x] Implement RSS feed fetching (RssSyncService)
- [x] Parse Newznab XML feeds
- [x] Extract releases from feed items
- [ ] Automatic grab logic (pending)

### 3.5 Indexer API ✅
- [x] CRUD operations with database persistence
- [x] Connection testing (/api/v3/indexer/test)
- [x] Schema endpoint for Newznab/Torznab types

---

## Phase 4: Queue Management ✅

### 4.1 Queue Database
- [x] Real-time query from download clients (no local cache)
- [x] Track pending downloads
- [x] Track download progress
- [x] Link to download clients

### 4.2 Queue API ✅
- [x] Implement `/api/v3/queue` with real data from clients
- [x] Implement `/api/v3/queue/status` with counts
- [x] Implement `/api/v3/queue/details`
- [x] Implement queue item removal
- [x] Match downloads to series/episodes using parser
- [ ] Implement grab/force download

### 4.3 Import Pipeline ✅
- [x] Detect completed downloads (ImportService.check_for_completed_downloads)
- [x] Parse release information (parser module with regex patterns)
- [x] Match to series/episodes (title_matches_series, match_episodes)
- [x] Move/copy files to library (ImportService.import)
- [x] Update database records (episode_file_id, has_file)
- [x] Clean up download client (ImportService.cleanup_download)
- [x] Record history (HistoryRepository)

### 4.4 Title Parser ✅
- [x] S01E02 pattern support (multi-episode: S01E02E03E04)
- [x] Alternative 1x02 format
- [x] Daily show format (YYYY.MM.DD)
- [x] Full season detection (S01.Complete)
- [x] Absolute episode numbering (anime)
- [x] Quality/source detection (HDTV, WEB-DL, BluRay + resolution)
- [x] Release group extraction
- [x] PROPER/REPACK detection

---

## Phase 5: Wanted & Calendar ✅

### 5.1 Wanted Episodes ✅
- [x] Query episodes without files that are monitored (get_missing repository method)
- [x] Implement `/api/v3/wanted/missing` and `/api/v5/wanted/missing`
- [x] Filter by date, series, monitored status
- [x] Pagination with page/pageSize
- [x] Sorting by airDateUtc, seriesTitle, episodeTitle
- [x] Include series data option
- [x] Implement `/api/v3/wanted/cutoff` and `/api/v5/wanted/cutoff` (stub for quality comparison)

### 5.2 Calendar ✅
- [x] Implement date range filtering (start/end parameters)
- [x] Query episodes by air date (get_by_air_date_range)
- [x] Include unmonitored option
- [x] Include specials option (season 0)
- [x] Calculate episode end time from runtime
- [x] Include series data option

---

## Phase 6: Notifications

### 6.1 Notification Framework
- [ ] Implement `NotificationService.send()` with real delivery
- [ ] Load notification configs from database
- [ ] Event-based triggering (on grab, download, etc.)

### 6.2 Notification Providers
- [ ] Discord webhook
- [ ] Email (SMTP)
- [ ] Webhook (generic)
- [ ] Slack
- [ ] Telegram
- [ ] Pushover
- [ ] Plex (library update)
- [ ] Emby/Jellyfin

### 6.3 Notification Events
- [ ] On Grab
- [ ] On Download/Import
- [ ] On Upgrade
- [ ] On Rename
- [ ] On Series Delete
- [ ] On Health Issue
- [ ] On Application Update

---

## Phase 7: History & Blocklist

### 7.1 History
- [ ] Record events in history table (already exists)
- [ ] Log grabs, imports, upgrades, deletions
- [ ] Implement `/api/v5/history` with real data
- [ ] Implement history filtering/pagination

### 7.2 Blocklist
- [ ] Record failed imports/bad releases
- [ ] Implement `/api/v5/blocklist` with real data
- [ ] Prevent re-downloading blocklisted items

---

## Phase 8: Additional Features

### 8.1 Manual Import
- [ ] Implement file browser for import path
- [ ] Scan for video files
- [ ] Parse filenames for episode info
- [ ] Preview import mapping
- [ ] Execute import

### 8.2 Parse API
- [ ] Implement release name parsing
- [ ] Extract series, season, episode, quality
- [ ] Scene numbering support

### 8.3 Series Metadata Enrichment
- [ ] Populate genres from database/Skyhook
- [ ] Populate tags
- [ ] Populate images with full URLs
- [ ] Calculate statistics (episode counts, etc.)

### 8.4 Remote Path Mapping
- [ ] Store mappings in database
- [ ] Apply mappings during import
- [ ] API CRUD operations

### 8.5 Config Persistence
- [ ] Save config changes to YAML/database
- [ ] Reload config on change

---

## Implementation Priority

### High Priority (Core Functionality)
1. Logging System (debugging/visibility)
2. Download Client Integration (qBittorrent first)
3. Indexer/Search (Newznab first)
4. Queue Management
5. Wanted Episodes

### Medium Priority (User Experience)
6. Notifications (Discord/Webhook)
7. Calendar
8. History tracking
9. Health Checks
10. File Operations (delete, rename)

### Lower Priority (Polish)
11. Additional download clients
12. Additional notification providers
13. Manual Import
14. Blocklist
15. Remote Path Mapping

---

## Architecture Notes

### Patterns to Follow
- **RefreshSeries/RescanSeries commands** - excellent examples of full implementation
- **Event bus** - use for decoupled notifications
- **Repository pattern** - already established for DB access

### Key Files to Reference
- `src/api/v5/command.rs` - well-implemented command execution
- `src/core/datastore/repositories.rs` - database patterns
- `src/core/messaging/mod.rs` - event bus usage

### Testing Strategy
- Unit tests for parsing logic
- Integration tests for API endpoints
- Mock external services (download clients, indexers)

---

## Progress Tracking

| Phase | Status | Completion |
|-------|--------|------------|
| Phase 1: Infrastructure | Complete | 90% |
| Phase 2: Download Clients | Complete | 85% |
| Phase 3: Indexers | Complete | 85% |
| Phase 4: Queue & Import | Complete | 90% |
| Phase 5: Wanted/Calendar | Complete | 90% |
| Phase 6: Notifications | Not Started | 0% |
| Phase 7: History/Blocklist | Not Started | 0% |
| Phase 8: Additional | Not Started | 0% |

---

## Quick Wins (Easy Implementations)

These can be done quickly to show progress:

1. **Logs table + API** - straightforward DB work
2. **Wanted/Missing** - just a query for episodes without files
3. **Calendar** - date-filtered episode query
4. **Health checks** - simple connectivity tests
5. **File deletion** - add `std::fs::remove_file` calls
6. **Size calculations** - sum episode file sizes

---

*Last Updated: 2026-01-29 (Phase 5 Complete)*
