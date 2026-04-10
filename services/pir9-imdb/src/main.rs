//! pir9 IMDB Service
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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use models::DownloadProgress;

mod db;
mod models;
mod sync;
mod tmdb;
mod tvmaze;

use db::DbRepository;

/// Well-known task slot names
const TASK_SYNC: &str = "sync";
const TASK_BACKFILL: &str = "backfill";

/// Handle to a running background task
struct TaskHandle {
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

/// Registry of named background tasks — allows independent operations to run concurrently
type TaskMap = HashMap<String, TaskHandle>;

/// Check whether a task slot is available (no task, or previous task finished).
/// Automatically removes finished handles so they don't block future requests.
fn try_claim_slot(tasks: &mut TaskMap, slot: &str) -> Result<(), &'static str> {
    if let Some(handle) = tasks.get(slot) {
        if !handle.join_handle.is_finished() {
            return Err(match slot {
                TASK_SYNC => "A sync/process is already running",
                TASK_BACKFILL => "An air-date backfill is already running",
                _ => "A task is already running in this slot",
            });
        }
        // Finished (success, error, or panic) — evict stale handle
        tasks.remove(slot);
    }
    Ok(())
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db: DbRepository,
    pub tmdb_client: Option<Arc<tmdb::TmdbClient>>,
    tasks: Arc<Mutex<TaskMap>>,
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
                .unwrap_or_else(|_| "info,pir9_imdb=debug".into()),
        )
        .init();

    info!("Starting pir9 IMDB Service v{}", env!("CARGO_PKG_VERSION"));

    // Database connection
    let db_url = std::env::var("PIR9_IMDB_DB_URL")
        .unwrap_or_else(|_| "postgres://pir9:pir9@localhost:5433/pir9_imdb".to_string());

    info!("Connecting to database...");
    let connect_options: PgConnectOptions = db_url.parse()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // Skip WAL flush wait — this is re-syncable cache data
                sqlx::query("SET synchronous_commit = off")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(connect_options)
        .await?;

    // Run migrations
    info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    let db = DbRepository::new(pool);

    // Startup cleanup: mark any stale 'running' syncs as failed (crash recovery)
    db.fail_stale_running_syncs().await?;

    // Initialize TMDB client (optional — enriches movie lookups with TMDB IDs + images)
    let tmdb_client = tmdb::TmdbClient::from_env().map(Arc::new);
    if tmdb_client.is_some() {
        info!("TMDB client initialized — movie lookups will be enriched with TMDB data");
    } else {
        info!("No PIR9_TMDB_API_KEY configured — movie lookups will return IMDB data only");
    }

    let state = AppState {
        db,
        tmdb_client,
        tasks: Arc::new(Mutex::new(HashMap::new())),
        download_progress: Arc::new(RwLock::new(DownloadProgress::default())),
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        // Series endpoints
        .route("/api/series/search", get(search_series))
        .route("/api/series/{imdb_id}", get(get_series))
        .route("/api/series/{imdb_id}/episodes", get(get_episodes))
        // Movie endpoints
        .route("/api/movies/search", get(search_movies))
        .route("/api/movies/{imdb_id}", get(get_movie))
        .route("/api/movies/{imdb_id}/credits", get(get_title_credits))
        .route("/api/movies/{imdb_id}/crew", get(get_title_crew))
        .route("/api/movies/{imdb_id}/akas", get(get_title_akas))
        // Series credits/crew/akas (same handlers, different route for clarity)
        .route("/api/series/{imdb_id}/credits", get(get_title_credits))
        .route("/api/series/{imdb_id}/crew", get(get_title_crew))
        .route("/api/series/{imdb_id}/akas", get(get_title_akas))
        // People
        .route("/api/people/{nconst}", get(get_person))
        // Stats
        .route("/api/stats", get(get_stats))
        // Sync endpoints
        .route("/api/sync", post(start_sync))
        .route("/api/sync/status", get(get_sync_status))
        .route("/api/sync/cancel", post(cancel_sync))
        .route("/api/backfill-air-dates", post(backfill_air_dates))
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

#[derive(Debug, Deserialize)]
struct EpisodeQuery {
    season: Option<i32>,
}

async fn get_episodes(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
    Query(query): Query<EpisodeQuery>,
) -> impl IntoResponse {
    let result = if let Some(season) = query.season {
        state.db.get_episodes_by_season(&imdb_id, season).await
    } else {
        state.db.get_episodes(&imdb_id).await
    };

    match result {
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

async fn search_movies(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.db.search_movies(&query.q, query.limit).await {
        Ok(results) => (StatusCode::OK, Json(results)).into_response(),
        Err(e) => {
            error!("Movie search error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// TMDB cache staleness threshold: re-fetch after 30 days
const TMDB_CACHE_DAYS: i64 = 30;

async fn get_movie(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    let mut db_movie = match state.db.get_movie(&imdb_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Movie not found" })),
            )
                .into_response()
        }
        Err(e) => {
            error!("Get movie error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // On-demand TMDB enrichment: fetch if never fetched or cache is stale
    if let Some(ref tmdb) = state.tmdb_client {
        let needs_fetch = match db_movie.tmdb_fetched_at {
            None => true,
            Some(fetched_at) => {
                let age = chrono::Utc::now() - fetched_at;
                age.num_days() > TMDB_CACHE_DAYS
            }
        };

        if needs_fetch {
            let imdb_str = format!("tt{:07}", db_movie.imdb_id);
            match tmdb.find_movie_by_imdb_id(&imdb_str).await {
                Some(data) => {
                    info!("TMDB enriched {} → tmdb_id={}", imdb_str, data.tmdb_id);
                    // Update DB cache
                    if let Err(e) = state
                        .db
                        .update_movie_tmdb_data(
                            db_movie.imdb_id,
                            Some(data.tmdb_id),
                            data.poster_url.as_deref(),
                            data.fanart_url.as_deref(),
                        )
                        .await
                    {
                        warn!("Failed to cache TMDB data for {}: {}", imdb_str, e);
                    }
                    // Update in-memory for this response
                    db_movie.tmdb_id = Some(data.tmdb_id);
                    db_movie.poster_url = data.poster_url;
                    db_movie.fanart_url = data.fanart_url;
                }
                None => {
                    // TMDB has no mapping — record that we checked
                    if let Err(e) = state
                        .db
                        .update_movie_tmdb_data(db_movie.imdb_id, None, None, None)
                        .await
                    {
                        warn!("Failed to mark TMDB check for {}: {}", imdb_id, e);
                    }
                }
            }
        }
    }

    (StatusCode::OK, Json(db_movie.to_api())).into_response()
}

async fn get_title_credits(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_credits_for_title(&imdb_id).await {
        Ok(credits) => {
            let result = models::TitleCredits {
                imdb_id,
                credits,
            };
            (StatusCode::OK, Json(result)).into_response()
        }
        Err(e) => {
            error!("Get credits error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_title_akas(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_akas_for_title(&imdb_id).await {
        Ok(akas) => (StatusCode::OK, Json(akas)).into_response(),
        Err(e) => {
            error!("Get AKAs error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_title_crew(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_crew_for_title(&imdb_id).await {
        Ok(Some(crew)) => (StatusCode::OK, Json(crew)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Crew not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get crew error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn get_person(
    State(state): State<Arc<AppState>>,
    Path(nconst): Path<String>,
) -> impl IntoResponse {
    match state.db.get_person(&nconst).await {
        Ok(Some(person)) => (StatusCode::OK, Json(person)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Person not found" })),
        )
            .into_response(),
        Err(e) => {
            error!("Get person error: {}", e);
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
    let req = body.map(|b| b.0).unwrap_or_default();
    let datasets = req.datasets;
    let force = req.force;

    let mut tasks = state.tasks.lock().await;

    if let Err(msg) = try_claim_slot(&mut tasks, TASK_SYNC) {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg, "status": "running" })),
        )
            .into_response();
    }

    // Previous task finished — clean up any stale DB records (e.g. panic before cleanup)
    let _ = state.db.fail_stale_running_syncs().await;

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
        // Force mode: truncate all data tables before syncing
        if force {
            info!("Force sync requested — truncating all IMDB data tables...");
            if let Err(e) = db.truncate_all_data().await {
                error!("Failed to truncate tables: {}", e);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                return;
            }
        }

        info!("Starting IMDB sync (datasets: {:?}, force: {})...", datasets, force);
        match sync::run_full_sync_selective(&db, &datasets, task_token, progress.clone()).await {
            Ok(report) => {
                info!("IMDB sync completed: {:?}", report);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
                p.current_file.clear();
                p.percentage = 0.0;
                p.bytes_done = 0;
                p.total_bytes = 0;
            }
            Err(e) => {
                error!("IMDB sync failed: {}", e);
                let mut p = progress.write().await;
                p.phase = "idle".to_string();
            }
        }
    });

    tasks.insert(
        TASK_SYNC.to_string(),
        TaskHandle {
            cancel_token: token,
            join_handle,
        },
    );

    let message = if force { "Full sync started (tables truncated)" } else { "Sync started" };
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": message,
            "status": "running",
            "force": force
        })),
    )
        .into_response()
}

async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.get_sync_status().await {
        Ok(mut status) => {
            // Enrich with live is_running from the in-memory task registry
            let tasks = state.tasks.lock().await;
            let is_running = tasks
                .get(TASK_SYNC)
                .map_or(false, |h| !h.join_handle.is_finished());

            // Set top-level is_running
            status.is_running = is_running;

            // Read live download progress
            let dl_progress = state.download_progress.read().await;
            let dl_file = dl_progress.current_file.clone();
            let dl_phase = dl_progress.phase.clone();
            let dl_pct = dl_progress.percentage;
            let dl_bytes_done = dl_progress.bytes_done;
            let dl_total = dl_progress.total_bytes;
            let est_total = dl_progress.estimated_total_rows;
            let cur_rows = dl_progress.current_rows_processed;
            drop(dl_progress);

            // Set per-dataset is_running + download/parsing progress on whichever has status="running"
            for ds_opt in [
                &mut status.title_basics,
                &mut status.title_episodes,
                &mut status.title_ratings,
                &mut status.title_akas,
                &mut status.title_crew,
                &mut status.name_basics,
                &mut status.title_principals,
            ] {
                if let Some(ref mut ds) = ds_opt {
                    ds.is_running = is_running && ds.status == "running";
                    // Attach download progress to the currently-active dataset
                    if ds.is_running && ds.dataset_name == dl_file {
                        ds.current_phase = Some(dl_phase.clone());
                        if dl_phase == "downloading" {
                            ds.download_progress = Some(dl_pct);
                            ds.download_size_bytes = if dl_total > 0 { Some(dl_total) } else { None };
                            ds.download_bytes_done = if dl_bytes_done > 0 { Some(dl_bytes_done) } else { None };
                        } else if dl_phase == "parsing" {
                            // Attach parsing progress
                            ds.estimated_total_rows = if est_total > 0 { Some(est_total) } else { None };
                            ds.parsing_progress = if est_total > 0 {
                                Some((cur_rows as f64 / est_total as f64 * 100.0).min(100.0))
                            } else {
                                None
                            };
                            // Use live rows_processed from progress state (more current than DB)
                            ds.rows_processed = cur_rows;
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
    let tasks = state.tasks.lock().await;
    let mut cancelled = Vec::new();

    for (name, handle) in tasks.iter() {
        if !handle.join_handle.is_finished() {
            info!("Cancelling running task: {}", name);
            handle.cancel_token.cancel();
            cancelled.push(name.clone());
        }
    }

    if cancelled.is_empty() {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "No tasks are currently running",
                "cancelled": 0
            })),
        )
            .into_response()
    } else {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": format!("Cancellation requested for: {}", cancelled.join(", ")),
                "status": "cancelling",
                "cancelled": cancelled.len()
            })),
        )
            .into_response()
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

    drop(join_handle); // Fire and forget

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

    let mut tasks = state.tasks.lock().await;

    if let Err(msg) = try_claim_slot(&mut tasks, TASK_SYNC) {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg, "status": "running" })),
        )
            .into_response();
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

    tasks.insert(
        TASK_SYNC.to_string(),
        TaskHandle {
            cancel_token: token,
            join_handle,
        },
    );
    drop(tasks);

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Process started",
            "status": "running"
        })),
    )
        .into_response()
}

// ============================================================================
// Air Date Backfill
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackfillRequest {
    #[serde(default = "default_backfill_limit")]
    limit: i64,
}

fn default_backfill_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackfillProgress {
    series_processed: i64,
    series_total: i64,
    episodes_updated: u64,
    series_not_found: i64,
}

async fn backfill_air_dates(
    State(state): State<Arc<AppState>>,
    body: Option<Json<BackfillRequest>>,
) -> impl IntoResponse {
    let limit = body.map(|b| b.limit).unwrap_or(100);

    let mut tasks = state.tasks.lock().await;

    if let Err(msg) = try_claim_slot(&mut tasks, TASK_BACKFILL) {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg, "status": "running" })),
        )
            .into_response();
    }

    let token = CancellationToken::new();
    let db = state.db.clone();
    let task_token = token.clone();

    let join_handle = tokio::spawn(async move {
        info!("Starting air date backfill (limit: {})...", limit);
        match run_backfill(&db, limit, task_token).await {
            Ok(progress) => {
                info!(
                    "Air date backfill completed: {}/{} series, {} episodes updated, {} not found on TVMaze",
                    progress.series_processed,
                    progress.series_total,
                    progress.episodes_updated,
                    progress.series_not_found
                );
            }
            Err(e) => {
                error!("Air date backfill failed: {}", e);
            }
        }
    });

    tasks.insert(
        TASK_BACKFILL.to_string(),
        TaskHandle {
            cancel_token: token,
            join_handle,
        },
    );

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": format!("Air date backfill started for up to {} series", limit),
            "status": "running"
        })),
    )
        .into_response()
}

/// Run the air date backfill task
async fn run_backfill(
    db: &db::DbRepository,
    limit: i64,
    token: CancellationToken,
) -> anyhow::Result<BackfillProgress> {
    let client = reqwest::Client::builder()
        .user_agent("pir9-IMDB/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let series_ids = db.get_series_needing_air_dates(limit).await?;
    let total = series_ids.len() as i64;
    info!("Found {} series needing air dates", total);

    let mut progress = BackfillProgress {
        series_processed: 0,
        series_total: total,
        episodes_updated: 0,
        series_not_found: 0,
    };

    for imdb_id in &series_ids {
        if token.is_cancelled() {
            info!("Air date backfill cancelled at {}/{}", progress.series_processed, total);
            break;
        }

        match backfill_one_series(&client, db, *imdb_id).await {
            Ok(Some(updated)) => {
                progress.episodes_updated += updated;
            }
            Ok(None) => {
                progress.series_not_found += 1;
            }
            Err(e) => {
                warn!("Failed to backfill air dates for tt{:07}: {}", imdb_id, e);
            }
        }

        progress.series_processed += 1;

        if progress.series_processed % 10 == 0 {
            info!(
                "Backfill progress: {}/{} series, {} episodes updated",
                progress.series_processed, total, progress.episodes_updated
            );
        }

        // Rate limit: ~20 requests per 10 seconds
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    Ok(progress)
}

/// Backfill air dates for a single series. Returns Some(episodes_updated) or None if not found.
async fn backfill_one_series(
    client: &reqwest::Client,
    db: &db::DbRepository,
    imdb_id: i64,
) -> anyhow::Result<Option<u64>> {
    // Step 1: Look up TVMaze show ID
    let show_id = match tvmaze::lookup_show(client, imdb_id).await? {
        Some(id) => id,
        None => {
            // Mark as attempted so we don't re-query this series for 7 days
            db.mark_episodes_attempted(imdb_id).await?;
            return Ok(None);
        }
    };

    // Step 2: Get all episodes from TVMaze
    let episodes = tvmaze::get_episodes(client, show_id).await?;

    if episodes.is_empty() {
        return Ok(Some(0));
    }

    // Step 3: Build arrays for batch update
    let mut seasons = Vec::with_capacity(episodes.len());
    let mut episode_nums = Vec::with_capacity(episodes.len());
    let mut air_dates = Vec::with_capacity(episodes.len());
    let mut titles: Vec<Option<String>> = Vec::with_capacity(episodes.len());

    for ep in &episodes {
        let episode_number = match ep.number {
            Some(n) => n,
            None => continue, // Skip specials without episode numbers
        };

        let air_date = match &ep.airdate {
            Some(d) if !d.is_empty() => match chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                Ok(date) => date,
                Err(_) => continue,
            },
            _ => continue,
        };

        seasons.push(ep.season);
        episode_nums.push(episode_number);
        air_dates.push(air_date);
        titles.push(ep.name.clone());
    }

    if seasons.is_empty() {
        return Ok(Some(0));
    }

    // Step 4: Batch update
    let title_refs: Vec<Option<&str>> = titles.iter().map(|t| t.as_deref()).collect();
    let updated = db
        .update_episode_air_dates(imdb_id, &seasons, &episode_nums, &air_dates, &title_refs)
        .await?;

    Ok(Some(updated))
}
