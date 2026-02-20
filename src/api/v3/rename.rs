//! Rename API endpoints
//! Provides preview of episode file renames according to naming format

use axum::{extract::{Query, State}, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;

use crate::core::datastore::repositories::{EpisodeFileRepository, EpisodeRepository, SeriesRepository};
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

    // Get series info
    let series = match series_repo.get_by_id(series_id).await {
        Ok(Some(s)) => s,
        _ => return Json(vec![]),
    };

    // Get episodes for this series
    let episodes = match episode_repo.get_by_series_id(series_id).await {
        Ok(eps) => eps,
        Err(_) => return Json(vec![]),
    };

    // Get episode files for this series
    let files = match file_repo.get_by_series_id(series_id).await {
        Ok(f) => f,
        Err(_) => return Json(vec![]),
    };

    // Filter by season if specified
    let files: Vec<_> = if let Some(season) = query.season_number {
        files.into_iter().filter(|f| f.season_number == season).collect()
    } else {
        files
    };

    let mut renames = Vec::new();

    for file in files {
        // Find all episodes linked to this file
        let file_episodes: Vec<_> = episodes.iter()
            .filter(|e| e.episode_file_id == Some(file.id))
            .collect();

        if file_episodes.is_empty() {
            continue;
        }

        // Get episode numbers
        let episode_numbers: Vec<i32> = file_episodes.iter()
            .map(|e| e.episode_number)
            .collect();

        // Generate new filename according to naming format
        let new_filename = generate_episode_filename(
            &series.title,
            file.season_number,
            &episode_numbers,
            &file_episodes.first().map(|e| e.title.clone()).unwrap_or_default(),
            file.release_group.as_deref(),
            &file.path,
        );

        // Only include if the filename would change
        let current_filename = Path::new(&file.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if new_filename != current_filename {
            // Build new full path
            let parent = Path::new(&file.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| series.path.clone());

            let new_path = format!("{}/{}", parent, new_filename);

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

    Json(renames)
}

/// Generate a filename according to naming format
/// Default format: {Series Title} - S{Season:00}E{Episode:00} - {Episode Title}
fn generate_episode_filename(
    series_title: &str,
    season: i32,
    episodes: &[i32],
    episode_title: &str,
    release_group: Option<&str>,
    original_path: &str,
) -> String {
    // Get file extension from original
    let extension = Path::new(original_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");

    // Clean series title for filename
    let clean_series = sanitize_for_filename(series_title);

    // Build episode part (e.g., S01E01 or S01E01E02 for multi-episode)
    let episode_part = if episodes.len() == 1 {
        format!("S{:02}E{:02}", season, episodes[0])
    } else {
        let ep_str: Vec<String> = episodes.iter()
            .map(|e| format!("E{:02}", e))
            .collect();
        format!("S{:02}{}", season, ep_str.join(""))
    };

    // Clean episode title
    let clean_episode_title = if episode_title.is_empty() {
        "Episode".to_string()
    } else {
        sanitize_for_filename(episode_title)
    };

    // Build filename
    let base_name = if let Some(group) = release_group {
        format!("{} - {} - {} [{}]",
            clean_series,
            episode_part,
            clean_episode_title,
            group
        )
    } else {
        format!("{} - {} - {}",
            clean_series,
            episode_part,
            clean_episode_title
        )
    };

    format!("{}.{}", base_name, extension)
}

/// Sanitize a string for use in a filename
fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_rename))
}
