# Anti-Patterns

Patterns to avoid in the pir9 codebase. All examples are Rust or TypeScript specific to this project.

## Rust Anti-Patterns

### Using `unwrap()` in Non-Test Code
```rust
// BAD: Panics crash the Tokio runtime silently
let series = repo.find_by_id(id).await.unwrap();
let config: Config = serde_json::from_str(&data).unwrap();

// GOOD: Propagate with ? or provide context
let series = repo.find_by_id(id).await?;
let config: Config = serde_json::from_str(&data)
    .context("failed to parse config JSON")?;

// GOOD: expect() with reason (only for truly invariant conditions)
let pool = state.db.as_ref().expect("database pool not initialized");
```

### Blocking the Async Runtime
```rust
// BAD: Sync I/O in async context blocks the Tokio thread pool
async fn import_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)?;  // BLOCKS!
    let metadata = std::fs::metadata(path)?;        // BLOCKS!
    Ok(())
}

// GOOD: Use tokio::fs for file operations
async fn import_file(path: &Path) -> Result<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let metadata = tokio::fs::metadata(path).await?;
    Ok(())
}

// GOOD: For CPU-bound work, use spawn_blocking
let result = tokio::task::spawn_blocking(move || {
    expensive_cpu_work(&data)
}).await?;
```

### Silent Tokio::spawn Panics
```rust
// BAD: Panic in spawned task is silently swallowed
tokio::spawn(async move {
    process_release(release).await.unwrap();  // panic is lost!
});

// GOOD: Handle errors, log failures, clean up state
tokio::spawn(async move {
    if let Err(e) = process_release(release).await {
        error!(error = %e, "failed to process release");
        // Clean up any stale DB records
        cleanup_stale_state(&db).await;
    }
});
```

### Taking a Guard When You Need It Later
```rust
// BAD: take() consumes the handle — can't check status afterward
let handle = handle_guard.take();
handle.abort();
// Later: handle_guard is now None, can't check if task is done

// GOOD: Borrow the handle for status checks, take only for final cleanup
let handle = &*handle_guard;  // borrow, don't consume
handle.abort();
// Later: handle_guard still holds the JoinHandle for status checks
```

## Database Anti-Patterns

### Initializing Counters From Zero on Resume
```rust
// BAD: Progress regresses when resuming from checkpoint
let mut processed = 0;  // Always starts at 0!
let checkpoint = load_checkpoint().await?;
// If checkpoint says 50,000 processed, we reset to 0 and re-report

// GOOD: Initialize from checkpoint values
let checkpoint = load_checkpoint().await?;
let mut processed = checkpoint.total_processed;  // Resume from saved state
```

### N+1 Queries
```rust
// BAD: One query per series
let series_list = repo.find_all_series().await?;
for series in &series_list {
    let episodes = repo.find_episodes_by_series(series.id).await?;  // N queries!
}

// GOOD: Batch query
let series_list = repo.find_all_series().await?;
let series_ids: Vec<i64> = series_list.iter().map(|s| s.id).collect();
let episodes = repo.find_episodes_by_series_ids(&series_ids).await?;  // 1 query
```

### Raw String SQL Without Parameters
```rust
// BAD: SQL injection risk
let query = format!("SELECT * FROM series WHERE title = '{}'", user_input);

// GOOD: SQLx parameterized queries (compile-time checked)
let series = sqlx::query_as!(SeriesDbModel,
    "SELECT * FROM series WHERE title = $1", user_input
).fetch_optional(&*pool).await?;
```

## API Anti-Patterns

### Changing v3 Response Shapes
```rust
// BAD: Renaming, removing, or retyping v3 fields
#[derive(Serialize)]
pub struct SeriesResource {
    pub series_id: i64,      // Was `id` — breaks every Sonarr client!
    // Removed `quality_profile_id` — breaks Overseerr!
}

// GOOD: Add new fields, never remove or rename
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResource {
    pub id: i64,                        // Original field preserved
    pub quality_profile_id: i64,        // Original field preserved
    pub pir9_internal_id: Option<i64>,  // NEW field — clients ignore unknowns
}
```

### Business Logic in Handlers
```rust
// BAD: Handler does everything
async fn import_episode(State(state): State<Arc<AppState>>, ...) -> Result<...> {
    let file = find_largest_file(&path)?;
    let quality = parse_quality_from_filename(&file);
    let episode = match_episode(&series, season, episode_num);
    rename_and_move_file(&file, &episode)?;
    update_database(&state.db, &episode).await?;
    publish_event(&state.event_bus, EpisodeFileImported { .. });
    // ... 100 more lines
}

// GOOD: Handler delegates to service
async fn import_episode(State(state): State<Arc<AppState>>, ...) -> Result<...> {
    let result = MediaFileService::import_episode(&state, &request).await?;
    Ok(Json(result))
}
```

## Frontend Anti-Patterns

### Implicit Return in forEach
```typescript
// BAD: forEach callback returns a value (Biome flags this)
items.forEach(item => item.process())

// GOOD: Block body prevents implicit return
items.forEach(item => { item.process(); })
```

### Using `let` When `const` Works
```typescript
// BAD: Variable never reassigned
let url = `/api/v5/series/${id}`;
let response = await fetch(url);

// GOOD: const signals immutability
const url = `/api/v5/series/${id}`;
const response = await fetch(url);
```

### Implicit `any` Types
```typescript
// BAD: TypeScript infers `any`
function processData(data) { ... }

// GOOD: Explicit types
function processData(data: SeriesResource): ProcessResult { ... }
```

## Architectural Anti-Patterns

### Circular Module Dependencies
```
// BAD: tv depends on indexers, indexers depends on tv
core/tv/services.rs → core/indexers/search.rs → core/tv/models.rs (circular!)

// GOOD: Shared types in a common location, or use events
core/tv/services.rs → core/indexers/search.rs
core/indexers/ uses ParsedEpisodeInfo from core/parser/ (no cycle)
```

### Adding Dependencies Without Checking Existing Crates
```toml
# BAD: Adding `bytes` crate when reqwest already provides what you need
[dependencies]
bytes = "1.0"  # Only needed for reqwest::Bytes type

# GOOD: Use .to_vec() to convert to Vec<u8> and avoid the new dependency
let data: Vec<u8> = response.bytes().await?.to_vec();
```
