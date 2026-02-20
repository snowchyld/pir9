# ADR-001: Use Axum as Web Framework

## Status
Accepted

## Context

pir9 is a ground-up rewrite of Sonarr (C#/.NET) in Rust. We needed a web framework for:
- REST API (35+ endpoints across v3 and v5)
- WebSocket connections (real-time UI updates)
- Static file serving (frontend SPA)
- Middleware (CORS, compression, tracing)

The main contenders in the Rust ecosystem were **Axum**, **Actix-web**, and **Warp**.

## Decision

Use **Axum** (from the Tokio team) as the web framework.

## Consequences

### Positive
- **First-class Tokio integration** — no impedance mismatch with our async runtime; shares the same executor, tower middleware ecosystem, and hyper HTTP layer
- **Tower middleware** — tower-http provides production-ready CORS, compression, tracing, and static file serving out of the box
- **Type-safe extractors** — `State<Arc<AppState>>`, `Path<i64>`, `Query<T>`, `Json<T>` catch mismatches at compile time
- **WebSocket support** — built-in via `axum::extract::ws`, no separate crate needed
- **Growing ecosystem** — Axum adoption has grown rapidly; good documentation and community support
- **Composable routing** — `Router::new().nest()` maps cleanly to our v3/v5 API structure

### Negative
- **Compile times** — Axum's heavy use of generics and tower traits increases compile times (mitigated by cargo-chef in Docker builds)
- **Extractors ordering matters** — `Body` extractor must be last; subtle bugs if extractors are ordered wrong
- **Less mature than Actix-web** — Actix has a longer track record in production (though Axum is backed by the Tokio team)

### Neutral
- Axum's Router uses `/{param}` syntax (not `:param` like Express/Actix) — different from Sonarr's original routes but consistent within pir9
