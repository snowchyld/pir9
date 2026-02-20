//! Media Cover API endpoints
//! Fetches and serves series cover images from TVDB via Skyhook

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::core::datastore::repositories::SeriesRepository;
use crate::web::AppState;

/// GET /api/v3/mediacover - Base endpoint returns empty (covers require series_id/filename)
pub async fn list_media_covers() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookImage {
    cover_type: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookShowResponse {
    images: Option<Vec<SkyhookImage>>,
}

/// GET /api/v3/mediacover/:seriesId/:filename
/// Fetches cover images from TVDB via Skyhook and proxies them
pub async fn get_media_cover(
    State(state): State<Arc<AppState>>,
    Path((series_id, filename)): Path<(i32, String)>,
) -> Response {
    // Parse the cover type from filename (poster.jpg, fanart.jpg, banner.jpg)
    let cover_type = filename
        .strip_suffix(".jpg")
        .or_else(|| filename.strip_suffix(".png"))
        .unwrap_or(&filename);

    // Look up the series to get TVDB ID
    let series_repo = SeriesRepository::new(state.db.clone());
    let series = match series_repo.get_by_id(series_id as i64).await {
        Ok(Some(s)) => s,
        _ => {
            return (StatusCode::NOT_FOUND, "Series not found").into_response();
        }
    };

    // Fetch show data from Skyhook to get image URLs
    let client = reqwest::Client::new();
    let url = format!(
        "http://skyhook.sonarr.tv/v1/tvdb/shows/en/{}",
        series.tvdb_id
    );

    let skyhook_response = match client
        .get(&url)
        .header("User-Agent", "pir9/0.1.0")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return (StatusCode::NOT_FOUND, "Failed to fetch from Skyhook").into_response();
        }
    };

    let show_data: SkyhookShowResponse = match skyhook_response.json().await {
        Ok(d) => d,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Failed to parse Skyhook response").into_response();
        }
    };

    // Find the matching image URL
    let image_url = show_data
        .images
        .as_ref()
        .and_then(|images| {
            images
                .iter()
                .find(|img| img.cover_type.to_lowercase() == cover_type.to_lowercase())
        })
        .map(|img| img.url.clone());

    let image_url = match image_url {
        Some(url) => url,
        None => {
            tracing::debug!(
                "Image type '{}' not found for series {}",
                cover_type,
                series_id
            );
            return (StatusCode::NOT_FOUND, "Image not found").into_response();
        }
    };

    // Fetch the actual image
    let image_response = match client
        .get(&image_url)
        .header("User-Agent", "pir9/0.1.0")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => {
            return (StatusCode::NOT_FOUND, "Failed to fetch image").into_response();
        }
    };

    // Determine content type
    let content_type = image_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    // Stream the image body
    let bytes = match image_response.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read image").into_response();
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400") // Cache for 24 hours
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build response",
            )
                .into_response()
        })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_media_covers))
        .route("/{series_id}/{filename}", get(get_media_cover))
}
