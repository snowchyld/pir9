# pir9 Backlog

## High Priority

### PostgreSQL Support
**Status:** Partially implemented - data migrated, code changes needed

**What's done:**
- PostgreSQL schema created (`migrations/postgres/001_initial_schema.sql`)
- Data successfully migrated to PostgreSQL (114 series, 7365 episodes, 5847 files)
- Docker Compose profile `production-postgres` configured
- Environment file `.env.postgres` created

**What's needed:**
1. Refactor repositories to be database-agnostic
   - Current: All repositories use `self.db.sqlite()` directly
   - Target: Use sqlx's `Any` pool type or conditional compilation
   - Files to modify: `src/core/datastore/repositories.rs` (all repository implementations)

2. Update query syntax for PostgreSQL compatibility
   - SQLite uses `?` for parameters, PostgreSQL uses `$1, $2, ...`
   - SQLite uses `AUTOINCREMENT`, PostgreSQL uses `SERIAL`
   - Consider using sqlx's `query!` macro for compile-time checked queries

3. Add database abstraction layer
   - Create trait-based repository interfaces
   - Implement for both SQLite and PostgreSQL

**Why PostgreSQL:**
- Better concurrent write performance (SQLite is single-writer)
- No "database is locked" errors during bulk operations
- Better scaling for larger libraries

**Current workaround:**
- SQLite with WAL mode enabled (`PRAGMA journal_mode=WAL`)
- Improves concurrent read/write but still single-writer

---

## Medium Priority

### Multi-episode file detection improvements
- Current: Basic S01E01E02 pattern detection
- Needed: Support for more patterns (S01E01-E03, etc.)

### Quality parsing improvements
- Parse HDR/DV information from filenames
- Better source detection (AMZN, NF, etc.)

---

## Low Priority

### Code cleanup
- 333 compiler warnings to address
- Unused code removal (NormalizeApiPathLayer, etc.)

### Documentation
- API documentation
- Deployment guide updates
