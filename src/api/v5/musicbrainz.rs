//! MusicBrainz API endpoints
//!
//! Proxies requests to the separate pir9-musicbrainz microservice.
//! The MusicBrainz service manages its own PostgreSQL database and handles
//! dataset syncing, artist/album search, and lookups.

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

/// Create MusicBrainz API routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Search and lookup
        .route("/artists/search", get(search_artists))
        .route("/artists/{mbid}", get(get_artist))
        .route("/artists/{mbid}/albums", get(get_artist_albums))
        .route("/albums/{mbid}", get(get_album))
        .route("/albums/{mbid}/releases", get(get_album_releases))
        .route("/releases/{mbid}/tracks", get(get_release_tracks_proxy))
        // Sync controls (proxied to pir9-musicbrainz service)
        .route("/sync", post(trigger_sync))
        .route("/sync/status", get(get_sync_status))
        .route("/sync/cancel", post(cancel_sync))
        .route("/stats", get(get_stats))
        // Granular dataset controls
        .route("/datasets", get(get_datasets))
        .route("/download", post(start_download))
        .route("/process", post(start_process))
}

// ========== Search and Lookup ==========

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    25
}

/// Search MusicBrainz artists by name (proxied to pir9-musicbrainz service)
async fn search_artists(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.search_artists(&query.q, query.limit).await {
        Ok(results) => (StatusCode::OK, Json(serde_json::json!(results))).into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz search error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get a single artist by MusicBrainz ID
async fn get_artist(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_artist(&mbid).await {
        Ok(Some(artist)) => (StatusCode::OK, Json(serde_json::json!(artist))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Artist not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz get artist error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get albums for an artist
async fn get_artist_albums(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_artist_albums(&mbid).await {
        Ok(albums) => (StatusCode::OK, Json(serde_json::json!(albums))).into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz get artist albums error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get a single album by MusicBrainz ID
async fn get_album(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_album(&mbid).await {
        Ok(Some(album)) => (StatusCode::OK, Json(serde_json::json!(album))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Album not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz get album error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get releases (editions) for an album by release group MBID
async fn get_album_releases(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_album_releases(&mbid).await {
        Ok(releases) => (StatusCode::OK, Json(serde_json::json!(releases))).into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz get album releases error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get track listing for a specific release MBID
async fn get_release_tracks_proxy(
    State(state): State<Arc<AppState>>,
    Path(mbid): Path<String>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_release_tracks(&mbid).await {
        Ok(tracks) => (StatusCode::OK, Json(serde_json::json!(tracks))).into_response(),
        Err(e) => {
            tracing::error!("MusicBrainz get release tracks error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

// ========== Sync Controls (proxied to pir9-musicbrainz service) ==========

/// Request body for selective sync/download/process
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MbSyncRequest {
    #[serde(default)]
    datasets: Vec<String>,
}

/// Trigger a MusicBrainz sync
async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    body: Option<Json<MbSyncRequest>>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();
    match client.trigger_sync_selective(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to trigger MusicBrainz sync: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get sync status
async fn get_sync_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_sync_status().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get MusicBrainz sync status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Cancel a running sync
async fn cancel_sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.cancel_sync().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to cancel MusicBrainz sync: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get dataset metadata - proxied to pir9-musicbrainz service's GET /api/datasets
async fn get_datasets(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_datasets().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get MusicBrainz datasets: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Download datasets to cache - proxied to pir9-musicbrainz service's POST /api/download
async fn start_download(
    State(state): State<Arc<AppState>>,
    body: Option<Json<MbSyncRequest>>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();
    match client.start_download(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start MusicBrainz download: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Process cached datasets - proxied to pir9-musicbrainz service's POST /api/process
async fn start_process(
    State(state): State<Arc<AppState>>,
    body: Option<Json<MbSyncRequest>>,
) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    let datasets = body.map(|b| b.datasets.clone()).unwrap_or_default();
    match client.start_process(&datasets).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(resp.body)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to start MusicBrainz process: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

/// Get MusicBrainz stats
async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let client = match &state.musicbrainz_client {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "MusicBrainz service is not configured" })),
            )
                .into_response()
        }
    };
    match client.get_stats().await {
        Ok(Some(stats)) => (StatusCode::OK, Json(serde_json::json!(stats))).into_response(),
        Ok(None) => Json(serde_json::json!({
            "artistCount": 0,
            "releaseGroupCount": 0,
            "releaseCount": 0,
            "coverArtCount": 0,
            "lastSync": null,
            "dbSizeBytes": 0,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
