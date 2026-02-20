---
name: rust-api
description: Scaffold a new v5 API endpoint following pir9 patterns
user-invocable: true
arguments:
  - name: resource
    description: Name of the API resource (e.g., "notifications", "tags")
    required: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# Rust API Endpoint Development

You are scaffolding a new v5 API endpoint for **$ARGUMENTS** in the pir9 PVR application.

## Workflow

### 1. Understand the resource
- Ask what CRUD operations are needed (list, get, create, update, delete)
- Determine if this maps to an existing DB table or needs a migration
- Check if a v3 compatibility endpoint is also needed

### 2. Create the API route file
Create `src/api/v5/$ARGUMENTS.rs` following the canonical pattern from `src/api/v5/series.rs`:

```rust
#![allow(dead_code, unused_imports, unused_variables)]

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_by_id).put(update).delete(remove))
}
```

### 3. Define response types
- Use `#[derive(Debug, Clone, Serialize, Deserialize)]` (follow project derive order)
- Use `#[serde(rename_all = "camelCase")]` for all API types (Sonarr client compatibility)
- Keep response types in the same file unless shared across endpoints

### 4. Create repository
Add database access methods in `src/core/datastore/repositories.rs` or a new domain-specific repository:
- Use SQLx compile-time checked queries
- Follow the Repository pattern: `XxxRepository::new(pool)`
- Return `Result<T>` using `anyhow`

### 5. Wire the route
Register the new routes in the v5 router at `src/api/v5/mod.rs`:
```rust
.nest("/api/v5/$ARGUMENTS", $ARGUMENTS::routes())
```

### 6. Add tests
Create unit tests in a `#[cfg(test)]` module within the route file.

## Conventions to enforce
- **No `unwrap()`** — use `?` or `expect("reason")`
- **`anyhow`** for application errors, **`thiserror`** for typed module errors
- **Acronyms stay uppercase**: SDTV, DVD, HDTV with `#[allow(clippy::upper_case_acronyms)]`
- **Handler signatures**: `State<Arc<AppState>>`, `Path<i64>`, `Query<T>`, return `Result<Json<T>, ApiError>`
- **Structured logging**: use `tracing::{info, warn, debug, error}`

## Before finishing
- Remind the user to bump the version in `Cargo.toml` for the commit
- Run `cargo clippy -- -D warnings` to verify no lint issues
- Run `cargo test` to verify tests pass
