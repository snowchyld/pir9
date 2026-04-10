//! IMDB API endpoints
//!
//! Proxies requests to the separate pir9-imdb microservice.
//! The IMDB service manages its own PostgreSQL database and handles
//! dataset syncing, search, and episode lookups.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::web::AppState;

/// Create IMDB API routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Search and lookup
        .route("/search", get(search_series))
        .route("/series/{imdb_id}", get(get_series))
        .route("/series/{imdb_id}/episodes", get(get_episodes))
        // Sync controls (proxied to pir9-imdb service)
        .route("/sync", post(start_sync))
        .route("/sync/status", get(get_sync_status))
        .route("/sync/cancel-stale", post(cancel_stale_syncs))
        .route("/backfill-air-dates", post(backfill_air_dates))
        .route("/stats", get(get_stats))
        // Granular dataset controls
        .route("/datasets", get(get_datasets))
        .route("/download", post(start_download))
        .route("/process", post(start_process))
        // Link to local series
        .route("/link/{imdb_id}", post(link_to_series))
        .route("/lookup-local/{imdb_id}", get(lookup_local_series))
}

// ========== Search and Lookup ==========

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub term: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    25
}

/// Search IMDB series by title (proxied to pir9-imdb service)
async fn search_series(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    match state
        .imdb_client
        .search_series(&query.term, query.limit)
        .await
    {
        Ok(results) => (StatusCode::OK, Json(serde_json::json!(results))).into_response(),
        Err(e) => {
            tracing::error!("IMDB search error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get a single series by IMDB ID
async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.imdb_client.get_series(&imdb_id).await {
        Ok(Some(series)) => (StatusCode::OK, Json(serde_json::json!(series))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Series not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("IMDB get series error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get episodes for a series
async fn get_episodes(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    match state.imdb_client.get_episodes(&imdb_id).await {
        Ok(episodes) => (StatusCode::OK, Json(serde_json::json!(episodes))).into_response(),
        Err(e) => {
            tracing::error!("IMDB get episodes error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

// ========== Sync Controls (proxied to pir9-imdb service) ==========

/// Request body for selective sync/download/process
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncRequest {
    #[serde(default)]
    datasets: Vec<String>,
}

/// Start an IMDB sync - proxied to pir9-imdb service's POST /api/sync
async fn start_sync(
    State(state): State<Arc<AppState>>,
    body: Option<Json<SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

    match state.imdb_client.start_sync_selective(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start IMDB sync: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get sync status - proxied to pir9-imdb service's GET /api/sync/status
async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.imdb_client.get_sync_status().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get IMDB sync status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Cancel stale syncs - proxied to pir9-imdb service's POST /api/sync/cancel
async fn cancel_stale_syncs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.imdb_client.cancel_sync().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to cancel IMDB sync: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Backfill episode air dates via TVMaze - proxied to pir9-imdb service
async fn backfill_air_dates(
    State(state): State<Arc<AppState>>,
    body: Option<Json<BackfillAirDatesRequest>>,
) -> impl IntoResponse {
    let limit = body.map(|b| b.limit).unwrap_or(100);
    match state.imdb_client.backfill_air_dates(limit).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start air date backfill: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackfillAirDatesRequest {
    #[serde(default = "default_backfill_limit")]
    limit: u32,
}

fn default_backfill_limit() -> u32 {
    100
}

/// Get dataset metadata - proxied to pir9-imdb service's GET /api/datasets
async fn get_datasets(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.imdb_client.get_datasets().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get IMDB datasets: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Download datasets to cache - proxied to pir9-imdb service's POST /api/download
async fn start_download(
    State(state): State<Arc<AppState>>,
    body: Option<Json<SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

    match state.imdb_client.start_download(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start IMDB download: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Process cached datasets - proxied to pir9-imdb service's POST /api/process
async fn start_process(
    State(state): State<Arc<AppState>>,
    body: Option<Json<SyncRequest>>,
) -> impl IntoResponse {
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();

    match state.imdb_client.start_process(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start IMDB process: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get IMDB stats - proxied to pir9-imdb service
async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.imdb_client.get_stats().await {
        Ok(Some(stats)) => Json(serde_json::json!({
            "seriesCount": stats.series_count,
            "episodeCount": stats.episode_count,
            "movieCount": stats.movie_count,
            "peopleCount": stats.people_count,
            "creditsCount": stats.credits_count,
            "akasCount": stats.akas_count,
            "crewCount": stats.crew_count,
            "dbSizeBytes": stats.db_size_bytes,
            "lastSync": stats.last_sync,
            "lastBasicsSync": stats.last_sync,
            "lastEpisodesSync": stats.last_sync,
            "lastRatingsSync": stats.last_sync,
        }))
        .into_response(),
        Ok(None) => Json(serde_json::json!({
            "seriesCount": 0,
            "episodeCount": 0,
            "movieCount": 0,
            "peopleCount": 0,
            "creditsCount": 0,
            "dbSizeBytes": 0,
            "lastSync": null,
            "lastBasicsSync": null,
            "lastEpisodesSync": null,
            "lastRatingsSync": null,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ========== Link to Local Series ==========

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRequest {
    pub series_id: i64,
}

async fn link_to_series(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
    Json(body): Json<LinkRequest>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    match sqlx::query(
        r#"
        UPDATE series
        SET imdb_id = $1
        WHERE id = $2
        "#,
    )
    .bind(&imdb_id)
    .bind(body.series_id)
    .execute(pool)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(serde_json::json!({
            "linked": true,
            "seriesId": body.series_id,
            "imdbId": imdb_id
        }))
        .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Series not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn lookup_local_series(
    State(state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    use sqlx::Row;

    let pool = state.db.pool();

    match sqlx::query(
        r#"
        SELECT id, title, tvdb_id, imdb_id, path, monitored, status
        FROM series
        WHERE imdb_id = $1
        "#,
    )
    .bind(&imdb_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => Json(serde_json::json!({
            "found": true,
            "id": row.get::<i64, _>("id"),
            "title": row.get::<String, _>("title"),
            "tvdbId": row.get::<i64, _>("tvdb_id"),
            "imdbId": row.get::<Option<String>, _>("imdb_id"),
            "path": row.get::<String, _>("path"),
            "monitored": row.get::<bool, _>("monitored"),
        }))
        .into_response(),
        Ok(None) => Json(serde_json::json!({
            "found": false,
            "imdbId": imdb_id
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
