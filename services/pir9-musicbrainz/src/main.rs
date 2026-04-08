//! pir9 MusicBrainz Service
//!
//! Standalone service for managing MusicBrainz data:
//! - Syncs data from MusicBrainz JSON dumps
//! - Provides REST API for artist/album/release lookups
//! - Manages its own PostgreSQL database
//!
//! Environment variables:
//! - PIR9_MB_DB_URL: PostgreSQL connection string
//! - PIR9_MB_PORT: Service port (default 8991)
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
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use models::DownloadProgress;

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
    pub download_progress: Arc<RwLock<DownloadProgress>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pir9_musicbrainz=debug".into()),
        )
        .init();

    info!(
        "Starting pir9 MusicBrainz Service v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Database connection
    let db_url = std::env::var("PIR9_MB_DB_URL")
        .unwrap_or_else(|_| "postgres://pir9:pir9@localhost:5434/pir9_musicbrainz".to_string());

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
        download_progress: Arc::new(RwLock::new(DownloadProgress::default())),
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Artist endpoints
        .route("/api/artists/search", get(search_artists))
        .route("/api/artists/{mbid}", get(get_artist))
        .route("/api/artists/{mbid}/albums", get(get_artist_albums))
        // Album (release group) endpoints
        .route("/api/albums/{mbid}", get(get_album))
        .route("/api/albums/{mbid}/releases", get(get_album_releases))
        // Release track listings
        .route("/api/releases/{mbid}/tracks", get(get_release_tracks))
        // Label endpoints
        .route("/api/labels/search", get(search_labels))
        .route("/api/labels/{mbid}", get(get_label))
        // Recording endpoints
        .route("/api/recordings/search", get(search_recordings))
        .route("/api/recordings/{mbid}", get(get_recording))
        // Stats
        .route("/api/stats", get(get_stats))
        // Sync endpoints
        .route("/api/sync", post(start_sync))
        .route("/api/sync/status", get(get_sync_status))
        .route("/api/sync/cancel", post(cancel_sync))
        // Granular dataset endpoints
        .route("/api/datasets", get(get_datasets))
        .route("/api/download", post(start_download))
        .route("/api/process", post(start_process))
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
    let port: u16 = std::env::var("PIR9_MB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8991);

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
        "service": "pir9-musicbrainz",
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

async fn search_artists(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.db.search_artists(&query.q, query.limit).await {
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

async fn get_artist(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_artist(&mbid).await {
        Ok(Some(artist)) => (StatusCode::OK, Json(artist)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Artist not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get artist error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_artist_albums(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_artist_albums(&mbid).await {
        Ok(albums) => (StatusCode::OK, Json(albums)).into_response(),
        Err(e) => {
            error!("Get artist albums error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_album(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_release_group(&mbid).await {
        Ok(Some(album)) => (StatusCode::OK, Json(album)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Album not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get album error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_album_releases(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_releases_for_group(&mbid).await {
        Ok(releases) => (StatusCode::OK, Json(releases)).into_response(),
        Err(e) => {
            error!("Get album releases error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_release_tracks(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_release_tracks(&mbid).await {
        Ok(tracks) => (StatusCode::OK, Json(tracks)).into_response(),
        Err(e) => {
            error!("Get release tracks error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn search_labels(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.db.search_labels(&query.q, query.limit).await {
        Ok(results) => (StatusCode::OK, Json(results)).into_response(),
        Err(e) => {
            error!("Search labels error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_label(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_label(&mbid).await {
        Ok(Some(label)) => (StatusCode::OK, Json(label)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Label not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get label error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn search_recordings(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.db.search_recordings(&query.q, query.limit).await {
        Ok(results) => (StatusCode::OK, Json(results)).into_response(),
        Err(e) => {
            error!("Search recordings error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_recording(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    match state.db.get_recording(&mbid).await {
        Ok(Some(recording)) => (StatusCode::OK, Json(recording)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Recording not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get recording error: {}", e);
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

async fn start_sync(
    State(state): State<Arc<AppState>>,
    body: Option<Json<models::SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

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
        // Previous sync task finished — clean up stale DB records
        let _ = state.db.fail_stale_running_syncs().await;
    }

    // Create cancellation token and spawn the sync task
    let token = CancellationToken::new();
    let db = state.db.clone();
    let task_token = token.clone();
    let progress = state.download_progress.clone();

    // Reset download progress
    {
        let mut p = progress.write().await;
        *p = DownloadProgress::default();
    }

    let join_handle = tokio::spawn(async move {
        info!("Starting MusicBrainz sync (datasets: {:?})...", datasets);
        match sync::run_full_sync_selective(&db, &datasets, task_token, progress.clone()).await {
            Ok(report) => {
                info!("MusicBrainz sync completed: {:?}", report);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                p.current_file.clear();
                p.percentage = 0.0;
                p.bytes_done = 0;
                p.total_bytes = 0;
            }
            Err(e) => {
                error!("MusicBrainz sync failed: {}", e);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
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

            // Read live download progress
            let dl_progress = state.download_progress.read().await;
            let dl_file = dl_progress.current_file.clone();
            let dl_phase = dl_progress.phase.clone();
            let dl_pct = dl_progress.percentage;
            let dl_bytes_done = dl_progress.bytes_done;
            let dl_total = dl_progress.total_bytes;
            drop(dl_progress);

            // Set per-dataset is_running + download progress on whichever has status="running"
            for ds_opt in [
                &mut status.artists,
                &mut status.release_groups,
                &mut status.releases,
                &mut status.labels,
                &mut status.recordings,
                &mut status.works,
                &mut status.areas,
                &mut status.series,
                &mut status.events,
                &mut status.instruments,
                &mut status.places,
            ] {
                if let Some(ref mut ds) = ds_opt {
                    ds.is_running = is_running && ds.status == "running";
                    if ds.is_running && ds.dataset_name == dl_file {
                        ds.current_phase = Some(dl_phase.clone());
                        if dl_phase == "downloading" {
                            ds.download_progress = Some(dl_pct);
                            ds.download_size_bytes = if dl_total > 0 { Some(dl_total) } else { None };
                            ds.download_bytes_done = if dl_bytes_done > 0 { Some(dl_bytes_done) } else { None };
                        }
                    } else if ds.is_running {
                        ds.current_phase = Some("parsing".to_string());
                    }
                }
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
            // Mark any "running" DB records as failed so status shows correctly
            let _ = state.db.fail_stale_running_syncs().await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "Sync cancellation requested",
                    "status": "cancelling"
                })),
            )
                .into_response()
        }
        _ => {
            // No active task, but clean up any stale DB records
            let _ = state.db.fail_stale_running_syncs().await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "No sync is currently running",
                    "cancelled": 0
                })),
            )
                .into_response()
        }
    }
}

// ============================================================================
// Granular Dataset Endpoints
// ============================================================================

/// GET /api/datasets — return metadata (sizes, cache status) for each dataset
async fn get_datasets(_state: State<Arc<AppState>>) -> impl IntoResponse {
    match sync::get_dataset_infos().await {
        Ok(infos) => (StatusCode::OK, Json(infos)).into_response(),
        Err(e) => {
            error!("Failed to get dataset infos: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/download — download datasets to cache without processing
async fn start_download(
    State(state): State<Arc<AppState>>,
    body: Option<Json<models::SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

    // Downloads run independently — no sync_handle lock
    let token = CancellationToken::new();
    let task_token = token.clone();
    let progress = state.download_progress.clone();

    {
        let mut p = progress.write().await;
        p.phase = "downloading".to_string();
        p.process_only = false;
    }

    let join_handle = tokio::spawn(async move {
        info!("Starting download-only (datasets: {:?})...", datasets);
        match sync::run_download_only(&datasets, &task_token, &progress).await {
            Ok(downloaded) => {
                info!("Download complete: {:?}", downloaded);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                p.current_file.clear();
            }
            Err(e) => {
                error!("Download failed: {}", e);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
            }
        }
    });

    // Store handle so cancel can reach it
    let mut handle_guard = state.sync_handle.lock().await;
    *handle_guard = Some(SyncHandle {
        cancel_token: token,
        join_handle,
    });
    drop(handle_guard);

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Download started",
            "status": "running"
        })),
    )
        .into_response()
}

/// POST /api/process — process already-cached datasets without re-downloading
async fn start_process(
    State(state): State<Arc<AppState>>,
    body: Option<Json<models::SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

    // Check if a sync is already running, and store handle for cancel support
    let mut handle_guard = state.sync_handle.lock().await;
    if let Some(ref handle) = *handle_guard {
        if !handle.join_handle.is_finished() {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "A sync or process is already running",
                    "status": "running"
                })),
            )
                .into_response();
        }
        let _ = state.db.fail_stale_running_syncs().await;
    }

    let token = CancellationToken::new();
    let db = state.db.clone();
    let task_token = token.clone();
    let progress = state.download_progress.clone();

    {
        let mut p = progress.write().await;
        p.process_only = true;
        p.phase = "parsing".to_string();
    }

    let join_handle = tokio::spawn(async move {
        info!("Starting process-only (datasets: {:?})...", datasets);
        match sync::run_process_only(&db, &datasets, task_token, progress.clone()).await {
            Ok(report) => {
                info!("Process complete: {:?}", report);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                p.current_file.clear();
                p.process_only = false;
            }
            Err(e) => {
                error!("Process failed: {}", e);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                p.process_only = false;
            }
        }
    });

    // Store handle so cancel can reach it
    *handle_guard = Some(SyncHandle {
        cancel_token: token,
        join_handle,
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Process started",
            "status": "running"
        })),
    )
        .into_response()
}
