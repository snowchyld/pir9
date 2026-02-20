//! Pir9 IMDB Service
//!
//! Standalone service for managing IMDB data:
//! - Syncs data from IMDB non-commercial datasets
//! - Provides REST API for series/episode lookups
//! - Manages its own PostgreSQL database
//!
//! Environment variables:
//! - PIR9_IMDB_DB_URL: PostgreSQL connection string
//! - PIR9_IMDB_PORT: Service port (default 8990)
//! - RUST_LOG: Log level (default info)

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

mod db;
mod models;
mod sync;

use db::DbRepository;

/// Handle to a running sync task — single source of truth for whether a sync is active
struct SyncHandle {
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db: DbRepository,
    sync_handle: Arc<Mutex<Option<SyncHandle>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pir9_imdb=debug".into()),
        )
        .init();

    info!("Starting Pir9 IMDB Service v{}", env!("CARGO_PKG_VERSION"));

    // Database connection
    let db_url = std::env::var("PIR9_IMDB_DB_URL")
        .unwrap_or_else(|_| "postgres://pir9:pir9@localhost:5433/pir9_imdb".to_string());

    info!("Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&db_url)
        .await?;

    // Run migrations
    info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    let db = DbRepository::new(pool);

    // Startup cleanup: mark any stale 'running' syncs as failed (crash recovery)
    db.fail_stale_running_syncs().await?;

    let state = AppState {
        db,
        sync_handle: Arc::new(Mutex::new(None)),
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Series endpoints
        .route("/api/series/search", get(search_series))
        .route("/api/series/{imdb_id}", get(get_series))
        .route("/api/series/{imdb_id}/episodes", get(get_episodes))
        // Stats
        .route("/api/stats", get(get_stats))
        // Sync endpoints
        .route("/api/sync", post(start_sync))
        .route("/api/sync/status", get(get_sync_status))
        .route("/api/sync/cancel", post(cancel_sync))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(Arc::new(state));

    // Start server
    let port: u16 = std::env::var("PIR9_IMDB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8990);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ============================================================================
// Handlers
// ============================================================================

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "pir9-imdb",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    25
}

async fn search_series(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.db.search_series(&query.q, query.limit).await {
        Ok(results) => (StatusCode::OK, Json(results)).into_response(),
        Err(e) => {
            error!("Search error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_series(&imdb_id).await {
        Ok(Some(series)) => (StatusCode::OK, Json(series)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Series not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get series error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_episodes(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_episodes(&imdb_id).await {
        Ok(episodes) => (StatusCode::OK, Json(episodes)).into_response(),
        Err(e) => {
            error!("Get episodes error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_stats().await {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => {
            error!("Get stats error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn start_sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut handle_guard = state.sync_handle.lock().await;

    // Check if a sync is actually running (in-memory truth, not DB)
    if let Some(ref handle) = *handle_guard {
        if !handle.join_handle.is_finished() {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "A sync is already running",
                    "status": "running"
                })),
            )
                .into_response();
        }
        // Previous sync task finished (normally, via error, or via panic).
        // Clean up any stale 'running' DB records that the task didn't get to mark
        // (e.g., if it panicked before reaching its cleanup code).
        let _ = state.db.fail_stale_running_syncs().await;
    }

    // Create cancellation token and spawn the sync task
    let token = CancellationToken::new();
    let db = state.db.clone();
    let task_token = token.clone();

    let join_handle = tokio::spawn(async move {
        info!("Starting IMDB sync...");
        match sync::run_full_sync(&db, task_token).await {
            Ok(report) => {
                info!("IMDB sync completed: {:?}", report);
            }
            Err(e) => {
                error!("IMDB sync failed: {}", e);
            }
        }
    });

    *handle_guard = Some(SyncHandle {
        cancel_token: token,
        join_handle,
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Sync started",
            "status": "running"
        })),
    )
        .into_response()
}

async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_sync_status().await {
        Ok(mut status) => {
            // Enrich with live is_running from the in-memory handle
            let handle_guard = state.sync_handle.lock().await;
            let is_running = match &*handle_guard {
                Some(handle) => !handle.join_handle.is_finished(),
                None => false,
            };

            // Set top-level is_running
            status.is_running = is_running;

            // Set per-dataset is_running on whichever has status="running"
            if let Some(ref mut ds) = status.title_basics {
                ds.is_running = is_running && ds.status == "running";
            }
            if let Some(ref mut ds) = status.title_episodes {
                ds.is_running = is_running && ds.status == "running";
            }
            if let Some(ref mut ds) = status.title_ratings {
                ds.is_running = is_running && ds.status == "running";
            }

            (StatusCode::OK, Json(status)).into_response()
        }
        Err(e) => {
            error!("Get sync status error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn cancel_sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let handle_guard = state.sync_handle.lock().await;

    match &*handle_guard {
        Some(handle) if !handle.join_handle.is_finished() => {
            info!("Cancelling running sync...");
            handle.cancel_token.cancel();
            // Don't take the handle — keep it so get_sync_status can still report
            // is_running: true until the task finishes its wind-down
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "Sync cancellation requested",
                    "status": "cancelling"
                })),
            )
                .into_response()
        }
        _ => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "No sync is currently running",
                "cancelled": 0
            })),
        )
            .into_response(),
    }
}
