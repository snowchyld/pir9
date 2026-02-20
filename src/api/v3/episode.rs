//! Episode API endpoints (v3)

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json},
    http::StatusCode,
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::EpisodeRepository;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct EpisodeQuery {
    pub series_id: Option<i32>,
    pub season_number: Option<i32>,
    pub episode_ids: Option<String>, // comma-separated or array
    pub episode_file_id: Option<i32>,
    pub include_images: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeResource {
    pub id: i32,
    pub series_id: i32,
    pub tvdb_id: i32,
    pub episode_file_id: i32,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: String,
    pub air_date: Option<String>,
    pub air_date_utc: Option<String>,
    pub runtime: i32,
    pub overview: Option<String>,
    pub has_file: bool,
    pub monitored: bool,
    pub absolute_episode_number: Option<i32>,
    pub scene_absolute_episode_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub unverified_scene_numbering: bool,
    pub end_time: Option<String>,
    pub grab_date: Option<String>,
    pub series_title: Option<String>,
    pub images: Vec<ImageResource>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResource {
    pub cover_type: String,
    pub url: String,
    pub remote_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorEpisodesRequest {
    pub episode_ids: Vec<i64>,
    pub monitored: bool,
}

impl From<crate::core::datastore::models::EpisodeDbModel> for EpisodeResource {
    fn from(ep: crate::core::datastore::models::EpisodeDbModel) -> Self {
        Self {
            id: ep.id as i32,
            series_id: ep.series_id as i32,
            tvdb_id: ep.tvdb_id as i32,
            episode_file_id: ep.episode_file_id.unwrap_or(0) as i32,
            season_number: ep.season_number,
            episode_number: ep.episode_number,
            title: ep.title,
            air_date: ep.air_date.map(|d| d.to_string()),
            air_date_utc: ep.air_date_utc.map(|d| d.to_rfc3339()),
            runtime: ep.runtime,
            overview: ep.overview,
            has_file: ep.has_file,
            monitored: ep.monitored,
            absolute_episode_number: ep.absolute_episode_number,
            scene_absolute_episode_number: ep.scene_absolute_episode_number,
            scene_episode_number: ep.scene_episode_number,
            scene_season_number: ep.scene_season_number,
            unverified_scene_numbering: ep.unverified_scene_numbering,
            end_time: None,
            grab_date: None,
            series_title: None,
            images: vec![],
        }
    }
}

/// GET /api/v3/episode - List episodes with optional filters
pub async fn get_episodes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EpisodeQuery>,
) -> Result<Json<Vec<EpisodeResource>>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let episodes = match (query.series_id, query.season_number) {
        (Some(series_id), Some(season)) => {
            repo.get_by_series_and_season(series_id as i64, season).await
                .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episodes: {}", e)))?
        }
        (Some(series_id), None) => {
            repo.get_by_series_id(series_id as i64).await
                .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episodes: {}", e)))?
        }
        _ => {
            // Parse comma-separated episode IDs if provided
            if let Some(ids_str) = query.episode_ids {
                let ids: Vec<i64> = ids_str
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();

                let mut episodes = Vec::new();
                for id in ids {
                    if let Some(ep) = repo.get_by_id(id).await
                        .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episode: {}", e)))? {
                        episodes.push(ep);
                    }
                }
                episodes
            } else {
                // Return empty if no filter - avoids returning entire DB
                vec![]
            }
        }
    };

    let resources: Vec<EpisodeResource> = episodes.into_iter().map(Into::into).collect();
    Ok(Json(resources))
}

/// GET /api/v3/episode/:id - Get a specific episode
pub async fn get_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<EpisodeResource>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let episode = repo.get_by_id(id as i64).await
        .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episode: {}", e)))?
        .ok_or(EpisodeError::NotFound)?;

    Ok(Json(episode.into()))
}

/// PUT /api/v3/episode/:id - Update an episode
pub async fn update_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(body): Json<EpisodeResource>,
) -> Result<Json<EpisodeResource>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let mut episode = repo.get_by_id(id as i64).await
        .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episode: {}", e)))?
        .ok_or(EpisodeError::NotFound)?;

    // Apply updates from body
    episode.monitored = body.monitored;

    repo.update(&episode).await
        .map_err(|e| EpisodeError::Internal(format!("Failed to update episode: {}", e)))?;

    tracing::info!("Updated episode: id={}, title={}", episode.id, episode.title);

    Ok(Json(episode.into()))
}

/// PUT /api/v3/episode/monitor - Bulk update monitored status
pub async fn update_episode_monitor(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MonitorEpisodesRequest>,
) -> Result<Json<Vec<EpisodeResource>>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let episodes = repo.update_monitored(&body.episode_ids, body.monitored).await
        .map_err(|e| EpisodeError::Internal(format!("Failed to update episodes: {}", e)))?;

    tracing::info!("Updated monitored status for {} episodes", episodes.len());

    let resources: Vec<EpisodeResource> = episodes.into_iter().map(Into::into).collect();
    Ok(Json(resources))
}

/// Error type for episode operations
#[derive(Debug)]
pub enum EpisodeError {
    NotFound,
    Internal(String),
}

impl IntoResponse for EpisodeError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            EpisodeError::NotFound => (StatusCode::NOT_FOUND, "Episode not found".to_string()),
            EpisodeError::Internal(msg) => {
                tracing::error!("Episode error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_episodes))
        .route("/{id}", get(get_episode).put(update_episode))
        .route("/monitor", put(update_episode_monitor))
}
