#![allow(dead_code, unused_imports, unused_variables)]
//! Podcast API endpoints
//! CRUD operations for podcasts

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::PodcastDbModel;
use crate::core::datastore::repositories::{
    PodcastEpisodeRepository, PodcastFileRepository, PodcastRepository,
};
use crate::web::AppState;

// Re-use ApiError from series module
use super::series::ApiError;

/// Create podcast router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_podcasts).post(create_podcast))
        .route(
            "/{id}",
            get(get_podcast).put(update_podcast).delete(delete_podcast),
        )
        .route("/{id}/episodes", get(get_podcast_episodes))
        .route("/{id}/refresh", post(refresh_podcast))
        .route("/{id}/rescan", post(rescan_podcast))
        .route("/lookup", get(lookup_podcast))
}

/// List all podcasts
async fn list_podcasts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PodcastResponse>>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());

    let db_podcasts = repo
        .get_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcasts: {}", e)))?;

    // Bulk-load episode statistics
    let stats_map = bulk_load_podcast_stats(&state.db).await;

    let podcasts: Vec<PodcastResponse> = db_podcasts
        .into_iter()
        .map(|p| {
            let mut response = PodcastResponse::from(p);
            if let Some(stats) = stats_map.get(&response.id) {
                response.statistics = Some(stats.clone());
            }
            response
        })
        .collect();

    Ok(Json(podcasts))
}

/// Get a single podcast by ID
async fn get_podcast(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<PodcastResponse>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());

    let podcast = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = PodcastResponse::from(podcast);
    enrich_podcast_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Bulk-load podcast episode statistics in a single query
async fn bulk_load_podcast_stats(
    db: &crate::core::datastore::Database,
) -> std::collections::HashMap<i64, PodcastStatistics> {
    use sqlx::Row;

    let pool = db.pool();

    let rows = sqlx::query(
        r#"
        SELECT
            pe.podcast_id,
            COUNT(pe.id)::int as episode_count,
            COUNT(CASE WHEN pe.has_file = true THEN 1 END)::int as episode_file_count,
            COALESCE(SUM(pf.size), 0)::bigint as size_on_disk
        FROM podcast_episodes pe
        LEFT JOIN podcast_files pf ON pf.podcast_id = pe.podcast_id
        GROUP BY pe.podcast_id
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .filter_map(|row| {
            let id: i64 = row.try_get("podcast_id").ok()?;
            let episode_count: i32 = row.try_get("episode_count").ok()?;
            let episode_file_count: i32 = row.try_get("episode_file_count").ok()?;
            let size_on_disk: i64 = row.try_get("size_on_disk").ok()?;
            let percent = if episode_count > 0 {
                (episode_file_count as f64 / episode_count as f64) * 100.0
            } else {
                0.0
            };
            Some((
                id,
                PodcastStatistics {
                    episode_count,
                    episode_file_count,
                    percent_of_episodes: percent,
                    size_on_disk,
                },
            ))
        })
        .collect()
}

/// Enrich a PodcastResponse with episode statistics from the database
async fn enrich_podcast_response(
    response: &mut PodcastResponse,
    db: &crate::core::datastore::Database,
) {
    use sqlx::Row;

    let pool = db.pool();

    if let Ok(row) = sqlx::query(
        r#"
        SELECT
            COUNT(id)::int as episode_count,
            COUNT(CASE WHEN has_file = true THEN 1 END)::int as episode_file_count
        FROM podcast_episodes
        WHERE podcast_id = $1
        "#,
    )
    .bind(response.id)
    .fetch_one(pool)
    .await
    {
        let episode_count: i32 = row.try_get("episode_count").unwrap_or(0);
        let episode_file_count: i32 = row.try_get("episode_file_count").unwrap_or(0);

        let size_on_disk: i64 = sqlx::query(
            "SELECT COALESCE(SUM(size), 0)::bigint as size_on_disk FROM podcast_files WHERE podcast_id = $1",
        )
        .bind(response.id)
        .fetch_one(pool)
        .await
        .ok()
        .and_then(|r| r.try_get("size_on_disk").ok())
        .unwrap_or(0);

        let percent = if episode_count > 0 {
            (episode_file_count as f64 / episode_count as f64) * 100.0
        } else {
            0.0
        };

        response.statistics = Some(PodcastStatistics {
            episode_count,
            episode_file_count,
            percent_of_episodes: percent,
            size_on_disk,
        });
    }
}

/// Create a new podcast
async fn create_podcast(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreatePodcastRequest>,
) -> Result<Json<PodcastResponse>, ApiError> {
    options.validate()?;

    let repo = PodcastRepository::new(state.db.clone());
    let episode_repo = PodcastEpisodeRepository::new(state.db.clone());

    // Check if podcast already exists by feed URL
    if let Some(_existing) = repo
        .get_by_feed_url(&options.feed_url)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::Validation(
            "Podcast with this feed URL already exists".to_string(),
        ));
    }

    // Fetch the RSS feed to get metadata
    let feed_metadata = crate::core::podcasts::feed::fetch_feed(&options.feed_url)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch RSS feed: {}", e)))?;

    let title = options.title.clone().unwrap_or(feed_metadata.title.clone());
    let clean = clean_title(&title);
    let sort = clean.clone();
    let slug = generate_slug(&title);

    let full_path = format!(
        "{}/{}",
        options.root_folder_path.trim_end_matches('/'),
        sanitize_filename::sanitize(&title)
    );

    let genres_json = serde_json::to_string(&feed_metadata.categories)
        .unwrap_or_else(|_| "[]".to_string());
    let tags_json =
        serde_json::to_string(&options.tags).unwrap_or_else(|_| "[]".to_string());

    // Build images from feed
    let images: Vec<PodcastImage> = feed_metadata
        .image_url
        .map(|url| {
            vec![PodcastImage {
                cover_type: "poster".to_string(),
                url: url.clone(),
                remote_url: Some(url),
            }]
        })
        .unwrap_or_default();
    let images_json =
        serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());

    let db_podcast = PodcastDbModel {
        id: 0,
        title: title.clone(),
        clean_title: clean,
        sort_title: sort,
        overview: feed_metadata.description.clone(),
        author: feed_metadata.author.clone(),
        feed_url: options.feed_url.clone(),
        website_url: feed_metadata.link.clone(),
        genres: genres_json,
        images: images_json,
        tags: tags_json,
        path: full_path.clone(),
        root_folder_path: options.root_folder_path.clone(),
        quality_profile_id: options.quality_profile_id,
        monitored: options.monitored,
        added: Utc::now(),
        last_info_sync: Some(Utc::now()),
        title_slug: slug,
    };

    let id = repo
        .insert(&db_podcast)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create podcast: {}", e)))?;

    tracing::info!("Created podcast: id={}, title={}", id, title);

    // Create the podcast folder on disk
    if !full_path.is_empty() {
        let path = std::path::Path::new(&full_path);
        if !path.exists() {
            match tokio::fs::create_dir_all(path).await {
                Ok(()) => tracing::info!("Created podcast folder: {}", full_path),
                Err(e) => tracing::warn!(
                    "Failed to create podcast folder {}: {}",
                    full_path,
                    e
                ),
            }
        }
    }

    // Create episodes from feed items
    let mut episodes_added = 0;
    for fe in &feed_metadata.episodes {
        let db_episode = crate::core::datastore::models::PodcastEpisodeDbModel {
            id: 0,
            podcast_id: id,
            title: fe.title.clone(),
            overview: fe.description.clone(),
            episode_number: fe.episode_number,
            season_number: fe.season_number.unwrap_or(1),
            air_date_utc: fe.pub_date,
            duration_ms: fe.duration_ms,
            download_url: fe.download_url.clone(),
            file_size: fe.file_size,
            has_file: false,
            podcast_file_id: None,
            monitored: true,
            guid: fe.guid.clone(),
        };

        if let Err(e) = episode_repo.insert(&db_episode).await {
            tracing::warn!("Failed to insert podcast episode: {}", e);
        } else {
            episodes_added += 1;
        }
    }

    tracing::info!(
        "Added {} episodes for podcast id={}",
        episodes_added,
        id
    );

    let created = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created podcast: {}", e)))?
        .ok_or(ApiError::Internal(
            "Podcast not found after creation".to_string(),
        ))?;

    let mut response = PodcastResponse::from(created);
    enrich_podcast_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Update an existing podcast
async fn update_podcast(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdatePodcastRequest>,
) -> Result<Json<PodcastResponse>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());

    let mut podcast = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Apply updates
    if let Some(title) = update.title {
        podcast.title = title.clone();
        podcast.clean_title = clean_title(&title);
        podcast.sort_title = podcast.clean_title.clone();
        podcast.title_slug = generate_slug(&title);
    }
    if let Some(monitored) = update.monitored {
        podcast.monitored = monitored;
    }
    if let Some(quality_profile_id) = update.quality_profile_id {
        podcast.quality_profile_id = quality_profile_id;
    }
    if let Some(path) = update.path {
        podcast.path = path;
    }
    if let Some(tags) = update.tags {
        podcast.tags = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    }

    repo.update(&podcast)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update podcast: {}", e)))?;

    tracing::info!("Updated podcast: id={}", id);

    let mut response = PodcastResponse::from(podcast);
    enrich_podcast_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Delete a podcast
async fn delete_podcast(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeletePodcastQuery>,
) -> Result<(), ApiError> {
    let repo = PodcastRepository::new(state.db.clone());

    let podcast = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Delete files from disk if requested
    if params.delete_files {
        let podcast_path = std::path::Path::new(&podcast.path);
        if podcast_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(podcast_path) {
                tracing::error!(
                    "Failed to delete podcast folder {}: {}",
                    podcast.path,
                    e
                );
            } else {
                tracing::info!("Deleted podcast folder: {}", podcast.path);
            }
        }
    }

    // Delete from database (podcast_episodes cascade)
    repo.delete(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete podcast: {}", e)))?;

    tracing::info!(
        "Deleted podcast: id={}, title={}, delete_files={}",
        id,
        podcast.title,
        params.delete_files
    );

    Ok(())
}

/// Get episodes for a podcast
async fn get_podcast_episodes(
    State(state): State<Arc<AppState>>,
    Path(podcast_id): Path<i64>,
) -> Result<Json<Vec<PodcastEpisodeResponse>>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());
    let episode_repo = PodcastEpisodeRepository::new(state.db.clone());

    // Verify podcast exists
    repo.get_by_id(podcast_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let episodes = episode_repo
        .get_by_podcast_id(podcast_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch episodes: {}", e)))?;

    let responses: Vec<PodcastEpisodeResponse> =
        episodes.into_iter().map(PodcastEpisodeResponse::from).collect();

    Ok(Json(responses))
}

/// Refresh a podcast (re-fetch RSS feed and sync episodes)
async fn refresh_podcast(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<PodcastResponse>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());
    let episode_repo = PodcastEpisodeRepository::new(state.db.clone());

    let mut podcast = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!("Refreshing podcast: id={}, title={}", id, podcast.title);

    // Fetch the RSS feed
    let feed_metadata = crate::core::podcasts::feed::fetch_feed(&podcast.feed_url)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch RSS feed: {}", e)))?;

    // Update podcast metadata from feed
    if let Some(desc) = feed_metadata.description {
        podcast.overview = Some(desc);
    }
    if let Some(author) = feed_metadata.author {
        podcast.author = Some(author);
    }
    if let Some(link) = feed_metadata.link {
        podcast.website_url = Some(link);
    }
    if let Some(image_url) = &feed_metadata.image_url {
        let images = vec![PodcastImage {
            cover_type: "poster".to_string(),
            url: image_url.clone(),
            remote_url: Some(image_url.clone()),
        }];
        podcast.images =
            serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());
    }
    if !feed_metadata.categories.is_empty() {
        podcast.genres = serde_json::to_string(&feed_metadata.categories)
            .unwrap_or_else(|_| "[]".to_string());
    }

    podcast.last_info_sync = Some(Utc::now());

    repo.update(&podcast)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update podcast: {}", e)))?;

    // Sync episodes: add new ones based on guid deduplication
    let mut episodes_added = 0;
    for fe in &feed_metadata.episodes {
        // Check for existing episode by guid
        let existing = if let Some(ref guid) = fe.guid {
            episode_repo
                .get_by_guid(id, guid)
                .await
                .unwrap_or(None)
        } else {
            None
        };

        if existing.is_none() {
            let db_episode = crate::core::datastore::models::PodcastEpisodeDbModel {
                id: 0,
                podcast_id: id,
                title: fe.title.clone(),
                overview: fe.description.clone(),
                episode_number: fe.episode_number,
                season_number: fe.season_number.unwrap_or(1),
                air_date_utc: fe.pub_date,
                duration_ms: fe.duration_ms,
                download_url: fe.download_url.clone(),
                file_size: fe.file_size,
                has_file: false,
                podcast_file_id: None,
                monitored: true,
                guid: fe.guid.clone(),
            };

            if let Err(e) = episode_repo.insert(&db_episode).await {
                tracing::warn!("Failed to insert podcast episode: {}", e);
            } else {
                episodes_added += 1;
            }
        }
    }

    tracing::info!(
        "Refreshed podcast id={}: {} new episodes",
        id,
        episodes_added
    );

    let refreshed = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::Internal(
            "Podcast not found after refresh".to_string(),
        ))?;

    let mut response = PodcastResponse::from(refreshed);
    enrich_podcast_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Rescan podcast files on disk
async fn rescan_podcast(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<PodcastResponse>, ApiError> {
    let repo = PodcastRepository::new(state.db.clone());

    let podcast = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch podcast: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!(
        "Rescan requested for podcast: id={}, title={} (not yet implemented)",
        id,
        podcast.title
    );

    // Rescan is a placeholder — the worker infrastructure (ScanType::RescanPodcast)
    // handles the actual implementation. For now, just return the podcast.

    let mut response = PodcastResponse::from(podcast);
    enrich_podcast_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Lookup podcasts by feed URL (validate and fetch metadata)
async fn lookup_podcast(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Vec<PodcastLookupResult>>, ApiError> {
    let term = query.term.trim();

    // If the term looks like a URL, fetch and parse the feed directly
    if term.starts_with("http://") || term.starts_with("https://") {
        let feed_metadata = crate::core::podcasts::feed::fetch_feed(term)
            .await
            .map_err(|e| {
                ApiError::Internal(format!("Failed to fetch RSS feed: {}", e))
            })?;

        let result = PodcastLookupResult {
            title: feed_metadata.title,
            overview: feed_metadata.description,
            author: feed_metadata.author,
            feed_url: term.to_string(),
            website_url: feed_metadata.link,
            image_url: feed_metadata.image_url,
            genres: feed_metadata.categories,
            episode_count: feed_metadata.episodes.len() as i32,
        };

        return Ok(Json(vec![result]));
    }

    // For non-URL terms, search iTunes Search API
    let results = search_itunes(term).await.unwrap_or_default();

    Ok(Json(results))
}

/// Search iTunes Search API for podcasts
async fn search_itunes(term: &str) -> Result<Vec<PodcastLookupResult>, anyhow::Error> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://itunes.apple.com/search?term={}&media=podcast&limit=25",
        urlencoding::encode(term)
    );

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            format!("pir9/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("iTunes API returned status: {}", response.status());
    }

    let body: ITunesSearchResponse = response.json().await?;

    let results: Vec<PodcastLookupResult> = body
        .results
        .into_iter()
        .map(|r| PodcastLookupResult {
            title: r.collection_name.unwrap_or_default(),
            overview: None,
            author: r.artist_name,
            feed_url: r.feed_url.unwrap_or_default(),
            website_url: r.collection_view_url,
            image_url: r.artwork_url600.or(r.artwork_url100),
            genres: r.genres.unwrap_or_default(),
            episode_count: r.track_count.unwrap_or(0),
        })
        .filter(|r| !r.feed_url.is_empty())
        .collect();

    Ok(results)
}

// Helper functions

fn clean_title(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_slug(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

// iTunes API types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesSearchResponse {
    result_count: i32,
    results: Vec<ITunesResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ITunesResult {
    collection_name: Option<String>,
    artist_name: Option<String>,
    feed_url: Option<String>,
    collection_view_url: Option<String>,
    artwork_url100: Option<String>,
    artwork_url600: Option<String>,
    genres: Option<Vec<String>>,
    track_count: Option<i32>,
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePodcastRequest {
    pub feed_url: String,
    pub title: Option<String>,
    pub quality_profile_id: i64,
    pub root_folder_path: String,
    #[serde(default = "default_true")]
    pub monitored: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
}

fn default_true() -> bool {
    true
}

impl CreatePodcastRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.feed_url.is_empty() {
            return Err(ApiError::Validation(
                "feed_url is required".to_string(),
            ));
        }
        if self.root_folder_path.is_empty() {
            return Err(ApiError::Validation(
                "root_folder_path is required".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePodcastRequest {
    pub title: Option<String>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i64>,
    pub path: Option<String>,
    pub tags: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeletePodcastQuery {
    #[serde(default)]
    pub delete_files: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub term: String,
}

/// Full podcast response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodcastResponse {
    pub id: i64,
    pub title: String,
    pub sort_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub feed_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    pub images: Vec<PodcastImage>,
    pub genres: Vec<String>,
    pub tags: Vec<i64>,
    pub path: String,
    pub root_folder_path: String,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub clean_title: String,
    pub title_slug: String,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_info_sync: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<PodcastStatistics>,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodcastStatistics {
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub percent_of_episodes: f64,
    pub size_on_disk: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodcastImage {
    pub cover_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

/// Podcast episode response
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PodcastEpisodeResponse {
    pub id: i64,
    pub podcast_id: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_number: Option<i32>,
    pub season_number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_date_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    pub has_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub podcast_file_id: Option<i64>,
    pub monitored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
}

/// Podcast lookup result
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastLookupResult {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub feed_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub genres: Vec<String>,
    pub episode_count: i32,
}

// From impls

impl From<PodcastDbModel> for PodcastResponse {
    fn from(p: PodcastDbModel) -> Self {
        let genres: Vec<String> = serde_json::from_str(&p.genres).unwrap_or_default();
        let tags: Vec<i64> = serde_json::from_str(&p.tags).unwrap_or_default();
        let images: Vec<PodcastImage> =
            serde_json::from_str(&p.images).unwrap_or_default();

        Self {
            id: p.id,
            title: p.title,
            sort_title: p.sort_title,
            overview: p.overview,
            author: p.author,
            feed_url: p.feed_url,
            website_url: p.website_url,
            images,
            genres,
            tags,
            path: p.path,
            root_folder_path: p.root_folder_path,
            quality_profile_id: p.quality_profile_id,
            monitored: p.monitored,
            clean_title: p.clean_title,
            title_slug: p.title_slug,
            added: p.added.to_rfc3339(),
            last_info_sync: p.last_info_sync.map(|d| d.to_rfc3339()),
            statistics: None,
        }
    }
}

impl From<crate::core::datastore::models::PodcastEpisodeDbModel> for PodcastEpisodeResponse {
    fn from(e: crate::core::datastore::models::PodcastEpisodeDbModel) -> Self {
        Self {
            id: e.id,
            podcast_id: e.podcast_id,
            title: e.title,
            overview: e.overview,
            episode_number: e.episode_number,
            season_number: e.season_number,
            air_date_utc: e.air_date_utc.map(|d| d.to_rfc3339()),
            duration_ms: e.duration_ms,
            download_url: e.download_url,
            file_size: e.file_size,
            has_file: e.has_file,
            podcast_file_id: e.podcast_file_id,
            monitored: e.monitored,
            guid: e.guid,
        }
    }
}
