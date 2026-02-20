//! Rename API endpoints (v3)
//! Provides preview of episode file renames according to naming format.
//! Delegates to the naming template engine for proper format rendering.

use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{
    EpisodeFileRepository, EpisodeRepository, SeriesRepository,
};
use crate::core::naming;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameQuery {
    pub series_id: Option<i64>,
    pub season_number: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEpisodeResource {
    pub series_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub episode_file_id: i64,
    pub existing_path: String,
    pub new_path: String,
}

/// GET /api/v3/rename - Preview file renames for a series
pub async fn get_rename(
    State(state): State<Arc<AppState>>,
    query: Query<RenameQuery>,
) -> Json<Vec<RenameEpisodeResource>> {
    let series_id = match query.series_id {
        Some(id) => id,
        None => return Json(vec![]),
    };

    let series_repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());
    let file_repo = EpisodeFileRepository::new(state.db.clone());

    let series = match series_repo.get_by_id(series_id).await {
        Ok(Some(s)) => s,
        _ => return Json(vec![]),
    };

    let episodes = match episode_repo.get_by_series_id(series_id).await {
        Ok(eps) => eps,
        Err(_) => return Json(vec![]),
    };

    let files = match file_repo.get_by_series_id(series_id).await {
        Ok(f) => f,
        Err(_) => return Json(vec![]),
    };

    // Filter by season if specified
    let files: Vec<_> = if let Some(season) = query.season_number {
        files
            .into_iter()
            .filter(|f| f.season_number == season)
            .collect()
    } else {
        files
    };

    let config = state.config.read().media.clone();
    let mut renames = Vec::new();

    for file in files {
        let file_episodes: Vec<_> = episodes
            .iter()
            .filter(|e| e.episode_file_id == Some(file.id))
            .cloned()
            .collect();

        if file_episodes.is_empty() {
            continue;
        }

        let quality: crate::core::profiles::qualities::QualityModel =
            serde_json::from_str(&file.quality).unwrap_or_default();

        let ctx = naming::EpisodeNamingContext {
            series: &series,
            episodes: &file_episodes,
            quality: &quality,
            release_group: file.release_group.as_deref(),
        };

        let new_filename = naming::build_episode_filename(&config, &ctx);
        let season_folder = naming::build_season_folder(&config, file.season_number);

        let ext = std::path::Path::new(&file.path)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let new_relative = format!("{}/{}{}", season_folder, new_filename, ext);
        let new_path = format!("{}/{}", series.path, new_relative);

        if new_path != file.path {
            let episode_numbers: Vec<i32> =
                file_episodes.iter().map(|e| e.episode_number).collect();

            renames.push(RenameEpisodeResource {
                series_id,
                season_number: file.season_number,
                episode_numbers,
                episode_file_id: file.id,
                existing_path: file.path.clone(),
                new_path,
            });
        }
    }

    renames.sort_by(|a, b| {
        a.season_number
            .cmp(&b.season_number)
            .then_with(|| a.episode_numbers.cmp(&b.episode_numbers))
    });

    Json(renames)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_rename))
}
