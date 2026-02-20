# Event Bus Catalog

All events published through pir9's event bus (`src/core/messaging/`). The bus supports in-memory mode (standalone) or Redis pub/sub (distributed server/worker mode).

## Event Categories

### Command Events

Lifecycle events for async operations triggered via API (refresh, search, backup, etc.).

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `CommandStarted` | `api/v5/command.rs`, `api/v3/command.rs` | Command name, trigger source | WebSocket (real-time UI updates) |
| `CommandUpdated` | Various command executors | Progress percentage, message | WebSocket |
| `CommandCompleted` | Various command executors | Command name, duration | WebSocket, Notification service |
| `CommandFailed` | Various command executors | Command name, error message | WebSocket, Notification service |

### Series Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `SeriesAdded` | `core/tv/services.rs` | Series model | Notification service, Scanner |
| `SeriesUpdated` | `core/tv/services.rs` | Series model | WebSocket |
| `SeriesDeleted` | `core/tv/services.rs` | Series ID | Notification service, WebSocket |
| `SeriesRefreshed` | `core/tv/services.rs` | Series ID, episode count | WebSocket |
| `SeriesScanned` | `core/scanner/consumer.rs` | Series ID, files found | WebSocket |

### Movie Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `MovieAdded` | `core/movies/services.rs` | Movie model | Notification service |
| `MovieUpdated` | `core/movies/services.rs` | Movie model | WebSocket |
| `MovieDeleted` | `core/movies/services.rs` | Movie ID | Notification service |
| `MovieRefreshed` | `core/movies/services.rs` | Movie ID | WebSocket |
| `MovieFileImported` | `core/movies/services.rs` | Movie ID, file path | Notification service |
| `MovieFileDeleted` | `core/movies/services.rs` | Movie ID, file path | Notification service |

### Episode Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `EpisodeAdded` | `core/tv/services.rs` | Episode model | WebSocket |
| `EpisodeUpdated` | `core/tv/services.rs` | Episode model | WebSocket |
| `EpisodeFileImported` | `core/scanner/consumer.rs` | Episode ID, file info, quality | Notification service, WebSocket |
| `EpisodeFileDeleted` | `core/tv/services.rs` | Episode ID | Notification service |

### Search Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `EpisodeSearchRequested` | API handlers | Episode ID | Search executor |
| `SeasonSearchRequested` | API handlers | Series ID, season number | Search executor |
| `SeriesSearchRequested` | API handlers | Series ID | Search executor |

### Download Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `ReleaseGrabbed` | `TrackedDownloadService` | Release title, indexer, quality | Notification service, History recorder |
| `DownloadStarted` | Queue monitor | Download ID, client | WebSocket |
| `DownloadCompleted` | Queue monitor | Download ID, output path | Import pipeline, Notification service |
| `DownloadFailed` | Queue monitor | Download ID, error | Notification service, History recorder |

### Queue Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `QueueUpdated` | `TrackedDownloadService` | Queue snapshot | WebSocket (real-time queue page) |

### System Events

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `HealthCheckChanged` | Health checker | Check name, status | Notification service, WebSocket |
| `ConfigUpdated` | Config API handler | Changed keys | All components that cache config |
| `NotificationSent` | Notification service | Provider, event type | Logging |

### Distributed Scanning Events (Redis only)

These events are only published when running in server/worker mode with Redis.

| Event | Published By | Payload | Subscribers |
|-------|-------------|---------|-------------|
| `ScanRequest` | Server (scanner/jobs.rs) | Root folder path, series ID | Worker nodes |
| `ScanResult` | Worker (scanner/consumer.rs) | Series ID, discovered files | Server |
| `WorkerOnline` | Worker (worker.rs) | Worker ID, path, capabilities | Server |
| `WorkerOffline` | Worker (worker.rs) | Worker ID | Server |
| `WorkerHeartbeat` | Worker (worker.rs) | Worker ID, load metrics | Server |

## Event Flow Patterns

### Release Grab Flow
```
RSS Sync → parse title → match series → check quality profile
    → TrackedDownloadService::grab_release()
        → publishes ReleaseGrabbed
        → publishes QueueUpdated
            → WebSocket pushes to frontend
            → Notification service sends Discord/webhook
            → History records the grab
```

### Episode Import Flow
```
Download completes → DownloadCompleted event
    → Import pipeline picks up
        → parse filename → match episode → move/rename file
            → publishes EpisodeFileImported
                → Notification service notifies
                → WebSocket updates UI
            → publishes QueueUpdated (removes from queue)
```

### Distributed Scan Flow
```
Server: Scheduler triggers scan
    → publishes ScanRequest (via Redis)
        → Worker receives, scans local filesystem
            → publishes ScanResult (via Redis)
                → Server processes results, updates DB
                    → publishes SeriesScanned
```

## Implementation Notes

- Event bus is defined in `src/core/messaging/mod.rs`
- In-memory implementation: `src/core/messaging/memory_bus.rs`
- Redis implementation: `src/core/messaging/redis_bus.rs` (behind `redis-events` feature flag)
- Subscribers register during `AppState` initialization in `src/web/mod.rs`
- Events are fire-and-forget — publishers don't wait for subscriber completion
