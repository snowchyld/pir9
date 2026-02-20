//! Episode API endpoints (v5)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::datastore::repositories::{EpisodeFileRepository, EpisodeRepository};
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_episodes))
        .route("/{id}", get(get_episode).put(update_episode))
        .route("/monitor", put(update_episode_monitor))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ListEpisodesQuery {
    pub series_id: Option<i64>,
    pub season_number: Option<i32>,
    pub episode_ids: Option<String>, // comma-separated IDs
    pub episode_file_id: Option<i64>,
    pub include_images: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEpisodeRequest {
    pub monitored: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorEpisodesRequest {
    pub episode_ids: Vec<i64>,
    pub monitored: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeResponse {
    pub id: i64,
    pub series_id: i64,
    pub tvdb_id: i64,
    pub episode_file_id: Option<i64>,
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
    pub images: Vec<ImageResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_file: Option<EpisodeFileResource>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFileResource {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub relative_path: String,
    pub path: String,
    pub size: i64,
    pub date_added: String,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub media_info: Option<serde_json::Value>,
    pub quality_cutoff_not_met: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResource {
    pub cover_type: String,
    pub url: String,
    pub remote_url: Option<String>,
}

impl EpisodeResponse {
    /// Create from episode model without episode file
    fn from_episode(ep: crate::core::datastore::models::EpisodeDbModel) -> Self {
        Self {
            id: ep.id,
            series_id: ep.series_id,
            tvdb_id: ep.tvdb_id,
            episode_file_id: ep.episode_file_id,
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
            images: vec![],
            episode_file: None,
        }
    }

    /// Create from episode model with optional episode file
    fn from_episode_with_file(
        ep: crate::core::datastore::models::EpisodeDbModel,
        file: Option<&crate::core::datastore::models::EpisodeFileDbModel>,
    ) -> Self {
        let mut response = Self::from_episode(ep);
        if let Some(ef) = file {
            response.episode_file = Some(EpisodeFileResource::from_db_model(ef));
        }
        response
    }
}

impl EpisodeFileResource {
    fn from_db_model(ef: &crate::core::datastore::models::EpisodeFileDbModel) -> Self {
        // Parse quality JSON
        let quality: QualityModel =
            serde_json::from_str(&ef.quality).unwrap_or_else(|_| QualityModel {
                quality: QualityResource {
                    id: 0,
                    name: "Unknown".to_string(),
                    source: "unknown".to_string(),
                    resolution: 0,
                },
                revision: RevisionResource {
                    version: 1,
                    real: 0,
                    is_repack: false,
                },
            });

        // Parse languages JSON
        let languages: Vec<LanguageResource> =
            serde_json::from_str(&ef.languages).unwrap_or_else(|_| {
                vec![LanguageResource {
                    id: 1,
                    name: "English".to_string(),
                }]
            });

        // Parse media info JSON
        let media_info: Option<serde_json::Value> = ef
            .media_info
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        Self {
            id: ef.id,
            series_id: ef.series_id,
            season_number: ef.season_number,
            relative_path: ef.relative_path.clone(),
            path: ef.path.clone(),
            size: ef.size,
            date_added: ef.date_added.to_rfc3339(),
            scene_name: ef.scene_name.clone(),
            release_group: ef.release_group.clone(),
            languages,
            quality,
            custom_formats: vec![],
            custom_format_score: 0,
            media_info,
            quality_cutoff_not_met: false,
        }
    }
}

impl From<crate::core::datastore::models::EpisodeDbModel> for EpisodeResponse {
    fn from(ep: crate::core::datastore::models::EpisodeDbModel) -> Self {
        Self::from_episode(ep)
    }
}

/// GET /api/v5/episode - List episodes with optional filters
async fn list_episodes(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListEpisodesQuery>,
) -> Result<Json<Vec<EpisodeResponse>>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());
    let file_repo = EpisodeFileRepository::new(state.db.clone());

    let episodes = match (params.series_id, params.season_number) {
        (Some(series_id), Some(season)) => {
            repo.get_by_series_and_season(series_id, season)
                .await
                .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episodes: {}", e)))?
        }
        (Some(series_id), None) => repo
            .get_by_series_id(series_id)
            .await
            .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episodes: {}", e)))?,
        _ => {
            // Parse comma-separated episode IDs if provided
            if let Some(ids_str) = params.episode_ids {
                let ids: Vec<i64> = ids_str
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();

                let mut episodes = Vec::new();
                for id in ids {
                    if let Some(ep) = repo.get_by_id(id).await.map_err(|e| {
                        EpisodeError::Internal(format!("Failed to fetch episode: {}", e))
                    })? {
                        episodes.push(ep);
                    }
                }
                episodes
            } else {
                // Return empty if no filter provided to avoid returning entire database
                vec![]
            }
        }
    };

    // Build a map of episode files by ID for efficient lookup
    let episode_files: HashMap<i64, crate::core::datastore::models::EpisodeFileDbModel> =
        if let Some(series_id) = params.series_id {
            file_repo
                .get_by_series_id(series_id)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|ef| (ef.id, ef))
                .collect()
        } else {
            HashMap::new()
        };

    // Convert episodes to responses with episode file data
    let responses: Vec<EpisodeResponse> = episodes
        .into_iter()
        .map(|ep| {
            let file = ep.episode_file_id.and_then(|fid| episode_files.get(&fid));
            EpisodeResponse::from_episode_with_file(ep, file)
        })
        .collect();

    Ok(Json(responses))
}

/// GET /api/v5/episode/{id} - Get a specific episode
async fn get_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<EpisodeResponse>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());
    let file_repo = EpisodeFileRepository::new(state.db.clone());

    let episode = repo
        .get_by_id(id)
        .await
        .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episode: {}", e)))?
        .ok_or(EpisodeError::NotFound)?;

    // Fetch episode file if present
    let episode_file = if let Some(file_id) = episode.episode_file_id {
        file_repo.get_by_id(file_id).await.ok().flatten()
    } else {
        None
    };

    Ok(Json(EpisodeResponse::from_episode_with_file(
        episode,
        episode_file.as_ref(),
    )))
}

/// PUT /api/v5/episode/{id} - Update an episode
async fn update_episode(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateEpisodeRequest>,
) -> Result<Json<EpisodeResponse>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let mut episode = repo
        .get_by_id(id)
        .await
        .map_err(|e| EpisodeError::Internal(format!("Failed to fetch episode: {}", e)))?
        .ok_or(EpisodeError::NotFound)?;

    // Apply updates
    if let Some(monitored) = update.monitored {
        episode.monitored = monitored;
    }

    repo.update(&episode)
        .await
        .map_err(|e| EpisodeError::Internal(format!("Failed to update episode: {}", e)))?;

    tracing::info!(
        "Updated episode: id={}, title={}",
        episode.id,
        episode.title
    );

    Ok(Json(episode.into()))
}

/// PUT /api/v5/episode/monitor - Bulk update monitored status
async fn update_episode_monitor(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MonitorEpisodesRequest>,
) -> Result<Json<Vec<EpisodeResponse>>, EpisodeError> {
    let repo = EpisodeRepository::new(state.db.clone());

    let episodes = repo
        .update_monitored(&body.episode_ids, body.monitored)
        .await
        .map_err(|e| EpisodeError::Internal(format!("Failed to update episodes: {}", e)))?;

    tracing::info!("Updated monitored status for {} episodes", episodes.len());

    let responses: Vec<EpisodeResponse> = episodes.into_iter().map(Into::into).collect();
    Ok(Json(responses))
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}
