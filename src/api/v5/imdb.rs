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
use tracing::warn;

use crate::web::AppState;

// ========== IMDB Service Client (inlined from former core::imdb module) ==========

/// Configuration for the pir9-imdb microservice
#[derive(Debug, Clone)]
struct ImdbServiceConfig {
    base_url: String,
    enabled: bool,
}

impl Default for ImdbServiceConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("PIR9_IMDB_SERVICE_URL")
                .unwrap_or_else(|_| "http://pir9-imdb:8990".to_string()),
            enabled: std::env::var("PIR9_IMDB_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
}

/// HTTP client for the pir9-imdb microservice
#[derive(Clone)]
struct ImdbClient {
    config: ImdbServiceConfig,
    client: reqwest::Client,
}

impl ImdbClient {
    fn from_env() -> Self {
        Self {
            config: ImdbServiceConfig::default(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    async fn get_series(&self, imdb_id: &str) -> anyhow::Result<Option<ImdbSeries>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/series/{}", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("IMDB service returned error: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to IMDB service: {}", e);
                Ok(None)
            }
        }
    }

    async fn search_series(&self, query: &str, limit: u32) -> anyhow::Result<Vec<ImdbSeries>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        let url = format!(
            "{}/api/series/search?q={}&limit={}",
            self.config.base_url,
            urlencoding::encode(query),
            limit
        );
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    async fn get_episodes(&self, imdb_id: &str) -> anyhow::Result<Vec<ImdbEpisode>> {
        if !self.config.enabled {
            return Ok(vec![]);
        }
        let url = format!("{}/api/series/{}/episodes", self.config.base_url, imdb_id);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    async fn get_stats(&self) -> anyhow::Result<Option<ImdbStats>> {
        if !self.config.enabled {
            return Ok(None);
        }
        let url = format!("{}/api/stats", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(_) | Err(_) => Ok(None),
        }
    }

    async fn start_sync(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync", self.config.base_url);
        match self.client.post(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    async fn get_sync_status(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync/status", self.config.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }

    async fn cancel_sync(&self) -> anyhow::Result<ImdbProxyResponse> {
        if !self.config.enabled {
            return Ok(ImdbProxyResponse {
                status: 503,
                body: serde_json::json!({ "error": "IMDB service is not enabled" }),
            });
        }
        let url = format!("{}/api/sync/cancel", self.config.base_url);
        match self.client.post(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response.json().await.unwrap_or_else(|_| {
                    serde_json::json!({ "error": "Failed to parse response" })
                });
                Ok(ImdbProxyResponse { status, body })
            }
            Err(e) => Ok(ImdbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("IMDB service unavailable: {}", e) }),
            }),
        }
    }
}

struct ImdbProxyResponse {
    status: u16,
    body: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImdbSeries {
    imdb_id: String,
    title: String,
    original_title: Option<String>,
    start_year: Option<i32>,
    end_year: Option<i32>,
    runtime_minutes: Option<i32>,
    genres: Vec<String>,
    is_adult: bool,
    rating: Option<f64>,
    votes: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImdbEpisode {
    imdb_id: String,
    parent_imdb_id: String,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    title: Option<String>,
    runtime_minutes: Option<i32>,
    rating: Option<f64>,
    votes: Option<i64>,
    air_date: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImdbStats {
    series_count: i64,
    episode_count: i64,
    last_sync: Option<String>,
    db_size_bytes: Option<i64>,
}

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
