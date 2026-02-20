# pir9 Lint & Feature Completion TODO

Generated: 2026-01-31

## Overview

After running `make lint`, there are ~355 clippy errors remaining. This document tracks what needs to be done.

---

## 1. Quick Wins (COMPLETED)

- [x] `collapsible_str_replace` - `src/core/parser/mod.rs`
- [x] `upper_case_acronyms` - Suppressed with `#[allow(...)]`
- [x] `derivable_impls` - 4 enums now use `#[derive(Default)]`
- [x] `useless_format` - `src/core/tv/services.rs`
- [x] `new_ret_no_self` - Suppressed with `#[allow(...)]`

---

## 2. Unused Imports (~50+ occurrences)

Most are in API v3 modules where routing functions are imported but endpoints not yet implemented.

### API v3 Modules (WIP endpoints)

| File | Unused Imports |
|------|----------------|
| `src/api/v3/autotagging.rs` | `delete`, `post`, `put` |
| `src/api/v3/command.rs` | `delete`, `post` |
| `src/api/v3/config.rs` | `put` |
| `src/api/v3/customfilter.rs` | `delete`, `post`, `put` |
| `src/api/v3/customformat.rs` | `delete`, `post`, `put` |
| `src/api/v3/delayprofile.rs` | `delete`, `post` |
| `src/api/v3/downloadclient.rs` | `delete`, `put` |
| `src/api/v3/importlist.rs` | `delete`, `put` |
| `src/api/v3/indexer.rs` | `delete`, `put` |
| `src/api/v3/metadata.rs` | `delete`, `put` |
| `src/api/v3/notification.rs` | `delete`, `put` |
| `src/api/v3/qualityprofile.rs` | `delete`, `post`, `put` |
| `src/api/v3/releaseprofile.rs` | `delete`, `post`, `put` |
| `src/api/v3/remotepathmapping.rs` | `delete`, `post`, `put` |
| `src/api/v3/serieseditor.rs` | `delete` |

### Other Modules

| File | Unused Imports |
|------|----------------|
| `src/cli.rs` | `Context`, `error` |

### Recommended Action

Either:
- Implement the missing endpoints, OR
- Add `#[allow(unused_imports)]` at module level for WIP code

---

## 3. Unused Variables (`src/cli.rs`)

CLI subcommands have parameters that aren't being used:

```rust
// src/cli.rs:229
SeriesCommands::Add { tvdb_id, title, quality_profile, root_folder }
//                                    ^^^^^^^^^^^^^^^ ^^^^^^^^^^^^ unused

// src/cli.rs:241
SeriesCommands::Search { series_id, season, episode }
//                       ^^^^^^^^^ ^^^^^^ ^^^^^^^ all unused

// src/cli.rs:266
SystemCommands::Backup { path }
//                       ^^^^ unused
```

### Recommended Action

Either implement the CLI handlers or prefix with `_` (e.g., `_quality_profile`).

---

## 4. Dead Code (~200+ occurrences)

Categorized by feature area:

### HIGH PRIORITY - Core Features Stubbed

| Feature | File(s) | Current State | Work Required |
|---------|---------|---------------|---------------|
| **Download History** | `core/download/history.rs` | All methods return `Ok(())` | DB schema + insert logic |
| **Download Queue** | `core/download/queue.rs`, `core/queue/mod.rs` | `enqueue()`, `remove()`, `grab()` are stubs | Storage backend wiring |
| **Media Analysis** | `core/mediafiles/mod.rs` | `analyze(path)` ignores path | mediainfo library integration |
| **Episode Cutoff Query** | `core/datastore/repositories.rs:880-915` | Returns `(vec![], 0)` | Quality comparison logic |

### MEDIUM PRIORITY - Frameworks Complete, Bodies Empty

| Feature | File(s) | Current State | Work Required |
|---------|---------|---------------|---------------|
| **Scheduler Jobs** | `core/scheduler.rs` | Loop works, jobs just log | Implement `RssSync`, `RefreshSeries`, `DownloadedEpisodesScan`, `Housekeeping`, `Backup` |
| **Notifications** | `core/notifications/service.rs` | Event listener done | Provider dispatch completion |
| **Download Clients API** | `core/download/clients.rs` | Routes defined | Handlers need to use request bodies |

### LOW PRIORITY - Minor Gaps

| Feature | File(s) | Current State |
|---------|---------|---------------|
| **Search Category** | `core/indexers/clients.rs:157` | Parsed but discarded |
| **TV Services** | `core/tv/services.rs` | Mostly complete, some TODOs |

---

## 5. Approach Options

### Option A: Suppress Dead Code Warnings (Quick)

Add to `src/lib.rs` or `src/main.rs`:
```rust
#![allow(dead_code)]  // Temporary: WIP codebase
```

Or per-module:
```rust
#[allow(dead_code)]
mod download;
```

### Option B: Implement Features (Thorough)

Priority order:
1. CLI handlers (quick wins)
2. Download history/queue (core functionality)
3. Scheduler job bodies
4. Media analysis integration

### Option C: Remove Unused Code (If Abandoned)

Only if features are confirmed abandoned - not recommended for WIP code.

---

## 6. Running Lint

```bash
# Full lint check
make lint

# Just Rust clippy
cargo clippy -- -D warnings

# Count remaining errors
cargo clippy -- -D warnings 2>&1 | grep "^error" | wc -l
```

---

## Notes

- The codebase follows a **framework-first approach** - infrastructure is solid, implementations are stubs
- Most "dead code" is **intentional WIP**, not abandoned features
- API v3 maintains Sonarr compatibility - don't remove endpoints even if unimplemented
