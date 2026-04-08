# ADR-006: Consolidate to Single External PostgreSQL Instance

**Date**: 2026-04-08
**Status**: Accepted
**Deciders**: Drew

## Context

pir9 runs three separate PostgreSQL 16-alpine containers:

| Container | Database | Size | Purpose |
|-----------|----------|------|---------|
| `pir9-postgres` (:5434) | `pir9` | 145 MB | Main application data |
| `pir9-postgres-imdb` (:5435) | `pir9_imdb` | 8.6 GB | IMDB dataset cache |
| `pir9-postgres-musicbrainz` (:5436) | `pir9_musicbrainz` | 7.6 GB | MusicBrainz dataset cache |

This wastes ~256 MB RAM (3x `shared_buffers=128MB`), triples operational overhead (monitoring, backups, upgrades), and adds three containers to docker-compose for no isolation benefit — all services deploy together as a single system.

Additionally, a database audit identified:
- 6 dead tables in the main DB (legacy IMDB tables, `download_queue`, `schema_migrations`)
- `TIMESTAMP` columns that should be `TIMESTAMPTZ` (blocklist, scheduled_tasks)
- Default Postgres tuning on all instances (suboptimal for the workload)
- Dead `UnitOfWork` abstraction in the datastore layer
- MusicBrainz syncing unused datasets (~400 MB wasted)

## Decision

Consolidate all three databases onto a single external PostgreSQL 18 instance:

- **Host**: 10.0.0.20
- **Port**: 5433
- **Instance**: PostgreSQL 18.3
- **Databases**: `pir9`, `pir9_imdb`, `pir9_musicbrainz` (separate databases, not schemas)
- **App user**: `pir9` (limited privileges)
- **Admin**: `postgres` (for provisioning only)

### Migration Strategy

| Database | Strategy | Rationale |
|----------|----------|-----------|
| `pir9` (main) | **pg_dump → pg_restore** | Small (145 MB), preserves all application state |
| `pir9_imdb` | **Fresh sync from TSV files** | Cached files exist in `tmp/imdb_data/`, service has `POST /api/sync` |
| `pir9_musicbrainz` | **Fresh sync from tar.xz files** | Cached files exist in `tmp/musicbrainz_data/`, service has `POST /api/sync` |

IMDB/MB are re-imported rather than migrated because:
1. The data is freely re-downloadable from public datasets
2. Fresh import avoids carrying forward dead tuples and bloat
3. The new Postgres 18 instance will build optimal indexes from scratch
4. A clean import is simpler than cross-version pg_dump/pg_restore for 16 GB of data

### Bundled Fixes (applied during migration)

| # | Fix | Scope |
|---|-----|-------|
| 4 | `TIMESTAMP` → `TIMESTAMPTZ` for `blocklist.date`, `scheduled_tasks.last_execution`, `scheduled_tasks.last_start_time` | Main DB migration |
| 5 | Postgres tuning: `shared_buffers=512MB`, `work_mem=16MB`, `maintenance_work_mem=256MB`, `random_page_cost=1.1`, `max_wal_size=4GB` | Instance-level |
| 7 | Autovacuum tuning for large IMDB tables (`autovacuum_vacuum_scale_factor=0.05`) | IMDB DB |
| 9 | Remove dead `UnitOfWork` abstraction from `src/core/datastore/` | Rust code |
| 10 | Audit and disable unused MusicBrainz dataset syncing | MB service config |

### What is NOT changed

- Existing indexes on IMDB and MusicBrainz databases are **kept** (will be leveraged later)
- API compatibility (v3/v5) unchanged
- Application behavior unchanged

## Consequences

### Positive
- ~256 MB RAM freed (3 Postgres instances → 0 local instances)
- Simplified docker-compose (remove 3 postgres containers + 3 volumes)
- Centralized backup strategy on 10.0.0.20
- PostgreSQL 18 features available (improved query planning, better vacuuming)
- Proper tuning from day one

### Negative
- Network dependency on 10.0.0.20 (was previously local)
- IMDB/MB re-import takes time (~hours for full sync)
- Must ensure 10.0.0.20 has adequate disk space (~20 GB for all three databases)

### Risks
- If 10.0.0.20 is unreachable, pir9 cannot start (mitigated: same LAN, UPS-backed NAS)
- Cross-network latency for queries (mitigated: 10GbE or local LAN, sub-ms latency)
