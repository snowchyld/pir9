//! Calendar API endpoints

use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::EpisodeDbModel;
use crate::core::datastore::repositories::{EpisodeRepository, SeriesRepository};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CalendarQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub unmonitored: Option<bool>,
    pub include_series: Option<bool>,
    pub include_episode_file: Option<bool>,
    pub include_episode_images: Option<bool>,
    pub include_unmonitored: Option<bool>,
    pub tags: Option<String>,
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
    pub end_time: Option<String>,
    pub grab_date: Option<String>,
    pub series_title: Option<String>,
    pub series: Option<serde_json::Value>,
}

/// Convert EpisodeDbModel to EpisodeResource for calendar
fn episode_to_calendar_resource(
    episode: &EpisodeDbModel,
    series_title: Option<String>,
    series_json: Option<serde_json::Value>,
) -> EpisodeResource {
    let end_time = episode.air_date_utc.map(|start| {
        let end = start + Duration::minutes(episode.runtime as i64);
        end.to_rfc3339()
    });

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
        end_time,
        grab_date: None,
        series_title,
        series: series_json,
    }
}

/// GET /api/v3/calendar
pub async fn get_calendar(
    State(state): State<Arc<AppState>>,
    query: Query<CalendarQuery>,
) -> Json<Vec<EpisodeResource>> {
    // Parse date range
    let now = Utc::now();
    let start = query
        .start
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| now - Duration::days(7));

    let end = query
        .end
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| now + Duration::days(28));

    let include_unmonitored =
        query.unmonitored.unwrap_or(false) || query.include_unmonitored.unwrap_or(false);
    let include_series = query.include_series.unwrap_or(false);
    let include_specials = true; // Season 0

    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());

    // Fetch episodes in date range
    let episodes = match episode_repo
        .get_by_air_date_range(start, end, include_unmonitored, include_specials)
        .await
    {
        Ok(eps) => eps,
        Err(e) => {
            tracing::error!("Failed to fetch calendar episodes: {}", e);
            return Json(vec![]);
        }
    };

    // Pre-fetch all series for efficiency
    let all_series = series_repo.get_all().await.unwrap_or_default();
    let series_map: std::collections::HashMap<i64, _> =
        all_series.into_iter().map(|s| (s.id, s)).collect();

    // Convert to resources
    let mut records = Vec::new();
    for episode in &episodes {
        let series = series_map.get(&episode.series_id);
        let series_title = series.map(|s| s.title.clone());

        let series_json = if include_series {
            series.map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "title": s.title,
                    "path": s.path,
                    "tvdbId": s.tvdb_id,
                    "status": s.status,
                    "network": s.network,
                    "runtime": s.runtime,
                })
            })
        } else {
            None
        };

        records.push(episode_to_calendar_resource(
            episode,
            series_title,
            series_json,
        ));
    }

    Json(records)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_calendar))
}
