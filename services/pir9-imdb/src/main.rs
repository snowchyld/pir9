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
use serde::Serialize;
use tracing::{error, info, warn};

mod db;
mod models;
mod sync;
mod tmdb;
mod tvmaze;

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
    pub tmdb_client: Option<Arc<tmdb::TmdbClient>>,
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

    info!("Starting pir9 IMDB Service v{}", env!("CARGO_PKG_VERSION"));

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
        // Movie endpoints
        .route("/api/movies/search", get(search_movies))
        .route("/api/movies/{imdb_id}", get(get_movie))
        .route("/api/movies/{imdb_id}/credits", get(get_title_credits))
        // Series credits (same handler, different route for clarity)
        .route("/api/series/{imdb_id}/credits", get(get_title_credits))
        // People
        .route("/api/people/{nconst}", get(get_person))
        // Stats
        .route("/api/stats", get(get_stats))
        // Sync endpoints
        .route("/api/sync", post(start_sync))
        .route("/api/sync/status", get(get_sync_status))
        .route("/api/sync/cancel", post(cancel_sync))
        .route("/api/backfill-air-dates", post(backfill_air_dates))
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
            if let Some(ref mut ds) = status.name_basics {
                ds.is_running = is_running && ds.status == "running";
            }
            if let Some(ref mut ds) = status.title_principals {
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

    let mut handle_guard = state.sync_handle.lock().await;

    // Check if a sync or backfill is already running
    if let Some(ref handle) = *handle_guard {
        if !handle.join_handle.is_finished() {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "A sync or backfill is already running",
                    "status": "running"
                })),
            )
                .into_response();
        }
        let _ = state.db.fail_stale_running_syncs().await;
    }

    // Create cancellation token and spawn the backfill task
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

    *handle_guard = Some(SyncHandle {
        cancel_token: token,
        join_handle,
    });

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
