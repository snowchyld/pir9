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

use crate::core::imdb::ImdbClient;
use crate::web::AppState;

/// Create a shared ImdbClient (reads config from env vars on first call)
fn imdb_client() -> ImdbClient {
    ImdbClient::from_env()
}

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
        .route("/stats", get(get_stats))
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
    State(_state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let client = imdb_client();
    match client.search_series(&query.term, query.limit).await {
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
    State(_state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    let client = imdb_client();
    match client.get_series(&imdb_id).await {
        Ok(Some(series)) => (StatusCode::OK, Json(serde_json::json!(series))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Series not found" }))).into_response(),
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
    State(_state): State<Arc<AppState>>,
    Path(imdb_id): Path<String>,
) -> impl IntoResponse {
    let client = imdb_client();
    match client.get_episodes(&imdb_id).await {
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

/// Start an IMDB sync - proxied to pir9-imdb service's POST /api/sync
async fn start_sync(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = imdb_client();
    match client.start_sync().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
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
async fn get_sync_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = imdb_client();
    match client.get_sync_status().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
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
async fn cancel_stale_syncs(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = imdb_client();
    match client.cancel_sync().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
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

/// Get IMDB stats - proxied to pir9-imdb service
async fn get_stats(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = imdb_client();
    match client.get_stats().await {
        Ok(Some(stats)) => {
            // Transform to match what the frontend expects
            Json(serde_json::json!({
                "seriesCount": stats.series_count,
                "episodeCount": stats.episode_count,
                "lastBasicsSync": stats.last_sync,
                "lastEpisodesSync": stats.last_sync,
                "lastRatingsSync": stats.last_sync,
            }))
            .into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({
                "seriesCount": 0,
                "episodeCount": 0,
                "lastBasicsSync": null,
                "lastEpisodesSync": null,
                "lastRatingsSync": null,
            }))
            .into_response()
        }
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
        Ok(Some(row)) => {
            Json(serde_json::json!({
                "found": true,
                "id": row.get::<i64, _>("id"),
                "title": row.get::<String, _>("title"),
                "tvdbId": row.get::<i64, _>("tvdb_id"),
                "imdbId": row.get::<Option<String>, _>("imdb_id"),
                "path": row.get::<String, _>("path"),
                "monitored": row.get::<bool, _>("monitored"),
            }))
            .into_response()
        }
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
