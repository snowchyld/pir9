//! Wanted API endpoints (missing and cutoff unmet)

use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::EpisodeDbModel;
use crate::core::datastore::repositories::{
    EpisodeRepository, SeriesRepository, TrackedDownloadRepository,
};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct WantedQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_series: Option<bool>,
    pub include_images: Option<bool>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Serialize)]
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
    pub series: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedResourcePagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<EpisodeResource>,
}

/// Convert EpisodeDbModel to EpisodeResource
fn episode_to_resource(
    episode: &EpisodeDbModel,
    series_json: Option<serde_json::Value>,
) -> EpisodeResource {
    EpisodeResource {
        id: episode.id as i32,
        series_id: episode.series_id as i32,
        tvdb_id: episode.tvdb_id as i32,
        episode_file_id: episode.episode_file_id.unwrap_or(0) as i32,
        season_number: episode.season_number,
        episode_number: episode.episode_number,
        title: episode.title.clone(),
        air_date: episode.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        air_date_utc: episode.air_date_utc.map(|d| d.to_rfc3339()),
        runtime: episode.runtime,
        overview: episode.overview.clone(),
        has_file: episode.has_file,
        monitored: episode.monitored,
        absolute_episode_number: episode.absolute_episode_number,
        scene_absolute_episode_number: episode.scene_absolute_episode_number,
        scene_episode_number: episode.scene_episode_number,
        scene_season_number: episode.scene_season_number,
        unverified_scene_numbering: episode.unverified_scene_numbering,
        series: series_json,
    }
}

/// GET /api/v3/wanted/missing
pub async fn get_wanted_missing(
    State(state): State<Arc<AppState>>,
    query: Query<WantedQuery>,
) -> Json<WantedResourcePagingResource> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let sort_key = query.sort_key.clone().unwrap_or("airDateUtc".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or("descending".to_string());
    let monitored_only = query.monitored.unwrap_or(true);
    let include_series = query.include_series.unwrap_or(true);

    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());
    let tracked_repo = TrackedDownloadRepository::new(state.db.clone());

    // Get episode IDs that are actively downloading so we can exclude them
    let downloading_episode_ids: std::collections::HashSet<i64> = tracked_repo
        .get_all_active()
        .await
        .unwrap_or_default()
        .iter()
        .flat_map(|d| serde_json::from_str::<Vec<i64>>(&d.episode_ids).unwrap_or_default())
        .collect();

    // Fetch missing episodes
    let (episodes, total) = match episode_repo
        .get_missing(monitored_only, page, page_size, &sort_key, &sort_direction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to fetch missing episodes: {}", e);
            return Json(WantedResourcePagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            });
        }
    };

    // Filter out episodes that are currently downloading
    let filtered_count = episodes.len();
    let episodes: Vec<_> = episodes
        .into_iter()
        .filter(|ep| !downloading_episode_ids.contains(&ep.id))
        .collect();
    let total = total - (filtered_count - episodes.len()) as i64;

    // Optionally include series data
    let mut records = Vec::new();
    for episode in &episodes {
        let series_json = if include_series {
            match series_repo.get_by_id(episode.series_id).await {
                Ok(Some(series)) => Some(serde_json::json!({
                    "id": series.id,
                    "title": series.title,
                    "titleSlug": series.title_slug,
                    "path": series.path,
                    "tvdbId": series.tvdb_id,
                    "status": series.status,
                    "network": series.network,
                })),
                _ => None,
            }
        } else {
            None
        };

        records.push(episode_to_resource(episode, series_json));
    }

    Json(WantedResourcePagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total as i32,
        records,
    })
}

/// GET /api/v3/wanted/cutoff
pub async fn get_wanted_cutoff(
    State(state): State<Arc<AppState>>,
    query: Query<WantedQuery>,
) -> Json<WantedResourcePagingResource> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let sort_key = query.sort_key.clone().unwrap_or("airDateUtc".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or("descending".to_string());
    let include_series = query.include_series.unwrap_or(true);

    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());

    // Fetch cutoff unmet episodes
    let (episodes, total) = match episode_repo
        .get_cutoff_unmet(page, page_size, &sort_key, &sort_direction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to fetch cutoff unmet episodes: {}", e);
            return Json(WantedResourcePagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            });
        }
    };

    // Optionally include series data
    let mut records = Vec::new();
    for episode in &episodes {
        let series_json = if include_series {
            match series_repo.get_by_id(episode.series_id).await {
                Ok(Some(series)) => Some(serde_json::json!({
                    "id": series.id,
                    "title": series.title,
                    "titleSlug": series.title_slug,
                    "path": series.path,
                    "tvdbId": series.tvdb_id,
                })),
                _ => None,
            }
        } else {
            None
        };

        records.push(episode_to_resource(episode, series_json));
    }

    Json(WantedResourcePagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total as i32,
        records,
    })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/missing", get(get_wanted_missing))
        .route("/cutoff", get(get_wanted_cutoff))
}
