# pir9 Feature Completion & Technical Debt

Updated: 2026-02-07

## Lint Status

**Clippy: 0 errors** — all warnings resolved as of v0.10.2.

```bash
cargo clippy -- -D warnings    # Clean ✓
cd frontend && npm run lint    # Clean (Biome 2.x) ✓
```

---

## Remaining Feature Gaps

### HIGH PRIORITY

| Feature | Location | Current State | Work Required |
|---------|----------|---------------|---------------|
| **RSS Sync auto-grab** | `core/scheduler.rs:248` | Fetches RSS feeds but doesn't process releases | Match releases against wanted episodes, check quality profiles, add to download queue |
| **Episode Cutoff Query** | `core/datastore/repositories.rs` | Returns episodes with files but doesn't compare quality | Join with quality_profiles, compare against cutoff |

### MEDIUM PRIORITY

| Feature | Location | Current State | Work Required |
|---------|----------|---------------|---------------|
| **Movie Refresh metadata** | `core/movies/services.rs:75` | Refresh exists but doesn't fetch updated info | Call pir9-imdb or TMDB for updated metadata |
| **Config Persistence** | `api/v5/config.rs:76` | Config changes aren't saved | Persist to config.toml and reload |
| **Queue Re-grab** | `api/v5/queue.rs:699` | Can't re-grab a previously removed release | Implement re-grab logic |

### LOW PRIORITY

| Feature | Location | Current State | Work Required |
|---------|----------|---------------|---------------|
| **CLI Subcommands** | `cli.rs:224-299` | All 14 commands are logging stubs | Implement series list/add/delete, system backup/restore, config show/set |
| **Search Category** | `core/indexers/clients.rs` | Parsed but discarded | Wire category through to indexer requests |
| **Media Analysis** | `core/mediafiles/mod.rs` | Filename-based only | Optionally integrate mediainfo binary for deeper metadata |

---

## Completed (Previously Tracked)

These items from the original TODO were resolved or found to be already implemented:

- [x] **All clippy warnings** — 355 → 0 across v0.8.x–v0.10.2
- [x] **Download History** — fully implemented (`record_grab`, `record_download_failed`, `record_import`)
- [x] **Download Queue** — fully implemented via `TrackedDownloadService`
- [x] **Scheduler Jobs** — 5 of 6 implemented (RefreshSeries, DownloadedEpisodesScan, Housekeeping, HealthCheck, Backup)
- [x] **Notifications** — event listener and provider dispatch working
- [x] **Download Clients API** — handlers wired and functional
- [x] **Frontend linting** — migrated to Biome 2.x (v0.10.1)
- [x] **Unused imports** — resolved via implementation or suppression

---

## Notes

- The codebase follows a **framework-first approach** — infrastructure is solid, implementations fill in over time
- API v3 maintains Sonarr compatibility — don't remove endpoints even if unimplemented
- The single highest-impact gap is **RSS Sync auto-grab** — this is what makes it a fully autonomous PVR
