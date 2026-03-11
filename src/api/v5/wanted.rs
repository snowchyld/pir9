//! Wanted API endpoints (v5)

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
    EpisodeRepository, MovieRepository, SeriesRepository, TrackedDownloadRepository,
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
    /// Accepted for backwards compatibility but series data is always included
    pub include_series: Option<bool>,
    pub monitored: Option<bool>,
    /// Filter by content type: "series" (types 0,1), "anime" (type 2), or omit for all
    pub content_type: Option<String>,
}

/// Map content_type query param to series_type values
fn content_type_to_series_types(content_type: &Option<String>) -> Option<Vec<i32>> {
    match content_type.as_deref() {
        Some("series") => Some(vec![0, 1]), // standard + daily
        Some("anime") => Some(vec![2]),
        _ => None, // no filter
    }
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
    pub series: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<EpisodeResource>,
}

/// Movie resource for wanted/missing movies
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieResource {
    pub id: i64,
    pub title: String,
    pub sort_title: String,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub year: i32,
    pub monitored: bool,
    pub has_file: bool,
    pub title_slug: String,
    pub path: String,
    pub status: i32,
    pub added: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedMoviePagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<MovieResource>,
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
        series: series_json,
    }
}

/// GET /api/v5/wanted/missing
pub async fn get_wanted_missing(
    State(state): State<Arc<AppState>>,
    query: Query<WantedQuery>,
) -> Json<WantedPagingResource> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let sort_key = query.sort_key.clone().unwrap_or("airDateUtc".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or("descending".to_string());
    let monitored_only = query.monitored.unwrap_or(true);
    let series_types = content_type_to_series_types(&query.content_type);

    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());
    let td_repo = TrackedDownloadRepository::new(state.db.clone());

    // Exclude episodes that are actively downloading
    let exclude_ids: Vec<i64> = td_repo
        .get_all_active()
        .await
        .unwrap_or_default()
        .iter()
        .flat_map(|td| serde_json::from_str::<Vec<i64>>(&td.episode_ids).unwrap_or_default())
        .collect();

    let (episodes, total) = match episode_repo
        .get_missing(
            monitored_only,
            page,
            page_size,
            &sort_key,
            &sort_direction,
            &exclude_ids,
            series_types.as_deref(),
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to fetch missing episodes: {}", e);
            return Json(WantedPagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            });
        }
    };

    // Always include series data — the frontend needs it for display and navigation
    let mut records = Vec::new();
    for episode in &episodes {
        let series_json = match series_repo.get_by_id(episode.series_id).await {
            Ok(Some(series)) => Some(serde_json::json!({
                "id": series.id,
                "title": series.title,
                "titleSlug": series.title_slug,
                "path": series.path,
                "tvdbId": series.tvdb_id,
                "seriesType": series.series_type,
            })),
            _ => None,
        };

        records.push(episode_to_resource(episode, series_json));
    }

    Json(WantedPagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total as i32,
        records,
    })
}

/// GET /api/v5/wanted/cutoff
pub async fn get_wanted_cutoff(
    State(state): State<Arc<AppState>>,
    query: Query<WantedQuery>,
) -> Json<WantedPagingResource> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let sort_key = query.sort_key.clone().unwrap_or("airDateUtc".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or("descending".to_string());
    let series_types = content_type_to_series_types(&query.content_type);

    let episode_repo = EpisodeRepository::new(state.db.clone());
    let series_repo = SeriesRepository::new(state.db.clone());

    let (episodes, total) = match episode_repo
        .get_cutoff_unmet(
            page,
            page_size,
            &sort_key,
            &sort_direction,
            series_types.as_deref(),
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to fetch cutoff unmet: {}", e);
            return Json(WantedPagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            });
        }
    };

    // Always include series data — the frontend needs it for display and navigation
    let mut records = Vec::new();
    for episode in &episodes {
        let series_json = match series_repo.get_by_id(episode.series_id).await {
            Ok(Some(series)) => Some(serde_json::json!({
                "id": series.id,
                "title": series.title,
                "titleSlug": series.title_slug,
                "path": series.path,
                "tvdbId": series.tvdb_id,
                "seriesType": series.series_type,
            })),
            _ => None,
        };

        records.push(episode_to_resource(episode, series_json));
    }

    Json(WantedPagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total as i32,
        records,
    })
}

/// GET /api/v5/wanted/missing/movies
pub async fn get_wanted_missing_movies(
    State(state): State<Arc<AppState>>,
    query: Query<WantedQuery>,
) -> Json<WantedMoviePagingResource> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let sort_key = query.sort_key.clone().unwrap_or("sortTitle".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or("ascending".to_string());
    let monitored_only = query.monitored.unwrap_or(true);

    let movie_repo = MovieRepository::new(state.db.clone());

    let (movies, total) = match movie_repo
        .get_missing(monitored_only, page, page_size, &sort_key, &sort_direction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to fetch missing movies: {}", e);
            return Json(WantedMoviePagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            });
        }
    };

    let records: Vec<MovieResource> = movies
        .iter()
        .map(|m| MovieResource {
            id: m.id,
            title: m.title.clone(),
            sort_title: m.sort_title.clone(),
            tmdb_id: m.tmdb_id,
            imdb_id: m.imdb_id.clone(),
            year: m.year,
            monitored: m.monitored,
            has_file: m.has_file,
            title_slug: m.title_slug.clone(),
            path: m.path.clone(),
            status: m.status,
            added: m.added.to_rfc3339(),
        })
        .collect();

    Json(WantedMoviePagingResource {
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
        .route("/missing/movies", get(get_wanted_missing_movies))
}
