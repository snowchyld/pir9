# Distributed Scanning

Pir9 supports distributed file scanning where workers run on machines with direct disk access to media files. This eliminates network I/O overhead when the main server accesses files over NFS/SMB mounts.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Server (Docker Host)                                            │
│  ├── pir9 --mode server        # Web UI + API + Scheduler      │
│  ├── Redis                     # Message bus                    │
│  └── Services:                                                  │
│      ├── ScanResultConsumer    # Processes worker results      │
│      ├── JobTrackerService     # Handles timeouts/retries      │
│      └── WorkerRegistryService # Tracks online workers         │
└─────────────────────────────────────────────────────────────────┘
                             │
                             │ Redis pub/sub
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│ Worker (Synology NAS)                                           │
│  └── pir9 --mode worker                                         │
│      ├── Subscribes to scan requests                            │
│      ├── Scans local disk (fast!)                              │
│      ├── Publishes results back                                 │
│      └── Sends heartbeats every 30s                            │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Build with Redis Support

```bash
cargo build --release --features redis-events
# or
make release
```

### 2. Start Redis

```bash
docker run -d --name redis -p 6379:6379 redis:alpine
```

### 3. Start Server

```bash
./pir9 --mode server --redis-url redis://localhost:6379
# or with Docker:
docker run -d --name pir9 \
  -p 8989:8989 \
  -e PIR9_REDIS_URL=redis://redis:6379 \
  reg.pir9.org:2443/pir9:latest \
  --mode server --redis-url redis://redis:6379
```

### 4. Start Worker(s) on NAS

```bash
docker run -d --name pir9-worker \
  -v /volume1/media/tv:/media/tv:ro \
  -v /volume1/media/anime:/media/anime:ro \
  reg.pir9.org:2443/pir9:latest \
  --mode worker \
  --redis-url redis://your-server-ip:6379 \
  --worker-path /media/tv \
  --worker-path /media/anime
```

## CLI Options

### Server Mode

```
pir9 --mode server --redis-url <URL>

Options:
  --mode server         Run as server (web UI + scheduler)
  --redis-url <URL>     Redis connection URL (required)
  --port <PORT>         Override default port (8989)
```

### Worker Mode

```
pir9 --mode worker --redis-url <URL> --worker-path <PATH>...

Options:
  --mode worker         Run as scan worker only
  --redis-url <URL>     Redis connection URL (required)
  --worker-path <PATH>  Paths this worker handles (required, repeatable)
  --worker-id <ID>      Custom worker ID (auto-generated if not set)
```

### Standalone Mode (Default)

```
pir9
# or
pir9 --mode all

Runs everything in one process (no Redis required).
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PIR9_REDIS_URL` | Redis connection URL | - |
| `PIR9_WORKER_PATHS` | Comma-separated worker paths | - |
| `PIR9_WORKER_ID` | Worker identifier | Auto-generated UUID |
| `PIR9_PORT` | Server port | 8989 |

## How It Works

### Scan Flow

1. **User triggers RescanSeries** via API/UI
2. **Server publishes `ScanRequest`** to Redis with:
   - Job ID (UUID)
   - Series IDs to scan
   - Paths to scan
3. **Workers receive request**, check if paths match their `--worker-path`
4. **Matching worker scans locally** (no network I/O!)
5. **Worker publishes `ScanResult`** with discovered files
6. **Server's `ScanResultConsumer`** processes results:
   - Creates `EpisodeFile` records
   - Links episodes to files
   - Publishes `SeriesScanned` event to UI

### Health Monitoring

- Workers send **heartbeats every 30 seconds**
- Server marks workers **unhealthy after 100 seconds** (3 missed heartbeats)
- `WorkerRegistryService` tracks all online workers and their paths

### Timeout & Retry

| Setting | Value |
|---------|-------|
| Job timeout | 5 minutes |
| Max retries | 3 |
| Retry backoff | 5s, 10s, 20s, 40s (exponential) |

If all retries fail, the server **falls back to local scanning**.

## Message Types

### ScanRequest
```json
{
  "type": "scan_request",
  "job_id": "uuid",
  "scan_type": "rescan_series",
  "series_ids": [1, 2, 3],
  "paths": ["/media/tv/Show Name"]
}
```

### ScanResult
```json
{
  "type": "scan_result",
  "job_id": "uuid",
  "series_id": 1,
  "worker_id": "worker-uuid",
  "files_found": [
    {
      "path": "/media/tv/Show/S01E01.mkv",
      "size": 1234567890,
      "season_number": 1,
      "episode_numbers": [1],
      "release_group": "GROUP",
      "filename": "S01E01.mkv"
    }
  ],
  "errors": []
}
```

### WorkerHeartbeat
```json
{
  "type": "worker_heartbeat",
  "worker_id": "worker-uuid",
  "paths": ["/media/tv", "/media/anime"],
  "scans_completed": 42,
  "files_found": 1337,
  "uptime_seconds": 3600
}
```

## Docker Compose Examples

See the main repository for complete docker-compose files:
- `docker-compose.yml` - Server deployment (with profiles for SQLite or PostgreSQL)
- `docker-compose.synology-worker.yml` - Synology worker deployment

### Server (Docker Host)

```yaml
services:
  api:
    image: reg.pir9.org:2443/pir9:latest
    command:
      - "--mode"
      - "server"
      - "--redis-url"
      - "redis://redis:6379"
    environment:
      - PIR9_DB_TYPE=postgres
      - PIR9_DB_CONNECTION=postgresql://pir9:pir9@postgres:5432/pir9
    depends_on:
      - redis
      - postgres

  redis:
    image: redis:7-alpine
    ports:
      - "6311:6379"  # Exposed for Synology workers (6311 avoids conflicts)

  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: pir9
      POSTGRES_PASSWORD: pir9
      POSTGRES_DB: pir9
```

### Worker (Synology Container Manager)

```yaml
services:
  pir9-worker:
    image: reg.pir9.org:2443/pir9:latest
    container_name: pir9-worker
    restart: unless-stopped
    command:
      - "--mode"
      - "worker"
      - "--redis-url"
      - "redis://10.0.0.13:6311"
      - "--worker-path"
      - "/volume1/Shows"
    volumes:
      - /volume1/Shows:/volume1/Shows:ro
    environment:
      - TZ=America/New_York
```

## Troubleshooting

### Worker not receiving requests

1. Check Redis connectivity:
   ```bash
   redis-cli -h your-server-ip ping
   ```

2. Verify worker paths match series paths in Pir9

3. Check worker logs:
   ```bash
   docker logs pir9-worker
   ```

### Scans timing out

- Increase `DEFAULT_JOB_TIMEOUT` in `src/core/scanner/jobs.rs`
- Check if worker has disk access issues
- Verify network connectivity between server and worker

### Files not being linked to episodes

- Ensure filename follows standard naming: `Show.S01E01.720p.HDTV.mkv`
- Check that episodes exist in database (refresh series first)

## Performance Benefits

| Scenario | Without Workers | With Workers |
|----------|-----------------|--------------|
| Scanning 10,000 files over NFS | ~5 minutes | ~30 seconds |
| Network I/O during scan | High | None (local disk) |
| CPU load on server | High | Minimal |

The worker reads files directly from local disk while the server only handles database operations and coordination.
