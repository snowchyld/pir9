//! Import preview handler and types.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

use super::fetch::fetch_all_downloads;
use crate::core::datastore::repositories::{
    DownloadClientRepository, EpisodeFileRepository, MovieRepository, SeriesRepository,
};
use crate::core::download::clients::create_client_from_model;
use crate::web::AppState;

// ─── Import Preview ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewResponse {
    pub id: i64,
    pub title: String,
    pub content_type: String,
    pub series: Option<ImportPreviewSeries>,
    pub movie: Option<ImportPreviewMovie>,
    pub output_path: String,
    pub files: Vec<ImportPreviewFile>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<ImportPreviewEpisode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewSeries {
    pub id: i64,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewMovie {
    pub id: i64,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewFile {
    pub source_file: String,
    pub source_size: i64,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    /// All matched episode numbers (for multi-episode files like E07-E08)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub episode_numbers: Vec<i32>,
    pub episode_title: Option<String>,
    pub destination_path: Option<String>,
    pub matched: bool,
    pub existing_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_file_size: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewEpisode {
    pub id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: String,
    pub has_file: bool,
    pub file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportQueueBody {
    pub overrides: Option<std::collections::HashMap<String, EpisodeOverride>>,
    /// Explicit series ID from the import preview UI — used as fallback when
    /// download title doesn't match any series (e.g., "Serenity" for Firefly S00E01)
    pub series_id: Option<i64>,
    /// Source file paths to force-reimport even if identical (same size) to existing files.
    /// Used when the destination file is damaged but hasn't been rescanned/rehashed yet.
    pub force_reimport: Option<Vec<String>>,
    /// Source file paths to skip during import (user chose "Do not import").
    /// Frontend-only state — not persisted, so files reappear on next import attempt.
    pub skip_files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeOverride {
    pub season_number: i32,
    /// Single episode (legacy, fallback when episode_numbers is absent)
    pub episode_number: Option<i32>,
    /// Multiple episodes for multi-episode files
    pub episode_numbers: Option<Vec<i32>>,
}

impl EpisodeOverride {
    /// Get all episode numbers, preferring episode_numbers over episode_number
    pub fn episodes(&self) -> Vec<i32> {
        if let Some(ref nums) = self.episode_numbers {
            nums.clone()
        } else if let Some(num) = self.episode_number {
            vec![num]
        } else {
            vec![]
        }
    }
}

/// GET /api/v5/queue/{id}/import-preview
/// Preview what an import will do before committing
pub(super) async fn get_import_preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ImportPreviewResponse>, StatusCode> {
    use crate::core::datastore::repositories::{EpisodeRepository, RemotePathMappingRepository};
    use crate::core::download::import::compute_destination_path;
    use crate::core::parser::parse_title;
    use crate::core::queue::UNTRACKED_ID_BASE;
    use crate::core::scanner::{is_video_file, parse_episodes_from_filename};

    let client_repo = DownloadClientRepository::new(state.db.clone());

    // Resolve download: tracked (id < UNTRACKED_ID_BASE) or untracked
    let (
        download_id,
        download_client_id,
        title,
        tracked_series_id,
        tracked_movie_id,
        stored_output_path,
    ) = if id < UNTRACKED_ID_BASE {
        match state.tracked.find_by_id(id).await {
            Some(td) => (
                td.download_id,
                td.client_id,
                td.title,
                if td.series_id > 0 {
                    Some(td.series_id)
                } else {
                    None
                },
                if td.movie_id > 0 {
                    Some(td.movie_id)
                } else {
                    None
                },
                None::<String>,
            ),
            None => return Err(StatusCode::NOT_FOUND),
        }
    } else {
        let downloads = fetch_all_downloads(&state, true).await;
        match downloads.into_iter().find(|d| d.id == id) {
            Some(dl) => {
                let dl_id = dl.download_id.ok_or(StatusCode::NOT_FOUND)?;
                let client_name = dl.download_client.unwrap_or_default();
                let clients = client_repo.get_all().await.unwrap_or_default();
                let client_id = clients
                    .iter()
                    .find(|c| c.name == client_name)
                    .map(|c| c.id)
                    .ok_or(StatusCode::NOT_FOUND)?;
                (
                    dl_id,
                    client_id,
                    dl.title,
                    dl.series_id.filter(|&sid| sid > 0),
                    dl.movie_id,
                    dl.output_path,
                )
            }
            None => return Err(StatusCode::NOT_FOUND),
        }
    };

    // Try to get live status from client; fall back to stored output_path if gone
    let client_model = client_repo
        .get_by_id(download_client_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let client =
        create_client_from_model(&client_model).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let live_status = client.get_download(&download_id).await.unwrap_or(None);

    let raw_output_path = live_status
        .as_ref()
        .and_then(|s| s.output_path.clone())
        .or(stored_output_path)
        .ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;

    // Apply remote path mappings
    let output_path = {
        let mapping_repo = RemotePathMappingRepository::new(state.db.clone());
        let mut mapped = raw_output_path.clone();
        if let Ok(mappings) = mapping_repo.get_all().await {
            for m in &mappings {
                if mapped.starts_with(&m.remote_path) {
                    mapped = mapped.replacen(&m.remote_path, &m.local_path, 1);
                    break;
                }
            }
        }
        mapped
    };

    // Get file list: try download client first, fall back to scanning the output path
    let dl_files = if live_status.is_some() {
        client.get_files(&download_id).await.unwrap_or_default()
    } else {
        // Client doesn't have this download anymore — scan the filesystem
        use crate::core::download::clients::DownloadFile;
        let scan_path = output_path.clone();
        tokio::task::spawn_blocking(move || {
            let path = std::path::Path::new(&scan_path);
            if !path.exists() {
                return vec![];
            }
            if path.is_file() {
                let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
                return vec![DownloadFile {
                    name: path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    size,
                }];
            }
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let size = std::fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0);
                        files.push(DownloadFile {
                            name: p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            size,
                        });
                    } else if p.is_dir() {
                        // One level of subdirectory
                        if let Ok(sub_entries) = std::fs::read_dir(&p) {
                            let dir_name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            for sub_entry in sub_entries.flatten() {
                                let sp = sub_entry.path();
                                if sp.is_file() {
                                    let size =
                                        std::fs::metadata(&sp).map(|m| m.len() as i64).unwrap_or(0);
                                    let name = format!(
                                        "{}/{}",
                                        dir_name,
                                        sp.file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default()
                                    );
                                    files.push(DownloadFile { name, size });
                                }
                            }
                        }
                    }
                }
            }
            files
        })
        .await
        .unwrap_or_default()
    };

    let media_config = state.config.read().media.clone();

    // ── Movie preview ──
    if let Some(movie_id) = tracked_movie_id {
        let movie_repo = MovieRepository::new(state.db.clone());
        let movie = movie_repo
            .get_by_id(movie_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let files: Vec<ImportPreviewFile> = dl_files
            .iter()
            .map(|f| {
                let filename = f.name.split('/').last().unwrap_or(&f.name);
                let is_video = is_video_file(std::path::Path::new(filename));
                ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: None,
                    episode_number: None,
                    episode_numbers: Vec::new(),
                    episode_title: None,
                    destination_path: if is_video {
                        Some(movie.path.clone())
                    } else {
                        None
                    },
                    matched: is_video,
                    existing_file: movie.has_file,
                    existing_file_size: None,
                }
            })
            .collect();

        return Ok(Json(ImportPreviewResponse {
            id,
            title,
            content_type: "movie".to_string(),
            series: None,
            movie: Some(ImportPreviewMovie {
                id: movie.id,
                title: movie.title,
                path: movie.path,
            }),
            output_path,
            files,
            episodes: vec![],
        }));
    }

    // ── Series preview ──
    let series_repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    // Resolve series: tracked series_id or parse from title
    let series = if let Some(sid) = tracked_series_id {
        series_repo
            .get_by_id(sid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?
    } else {
        // Try to match from release title
        let parsed = parse_title(&title).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
        crate::core::download::import::match_series_standalone(&state.db, &parsed)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?
    };

    // Load all episodes for this series
    let all_episodes = episode_repo
        .get_by_series_id(series.id)
        .await
        .unwrap_or_default();

    // Episodes that already have files (use has_file flag from episode model)
    let episodes_with_files: HashSet<i64> = all_episodes
        .iter()
        .filter(|e| e.has_file)
        .map(|e| e.id)
        .collect();

    // Load episode file sizes for existing file comparison
    let episode_file_repo = EpisodeFileRepository::new(state.db.clone());
    let file_size_map: std::collections::HashMap<i64, i64> = episode_file_repo
        .get_by_series_id(series.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.id, f.size))
        .collect();

    // Parse the release title for quality/group info
    let parsed_info = parse_title(&title).unwrap_or_default();

    // Build preview for each file
    let mut preview_files = Vec::new();
    for f in &dl_files {
        let filename = f.name.split('/').last().unwrap_or(&f.name);
        let is_video = is_video_file(std::path::Path::new(filename));

        if !is_video {
            preview_files.push(ImportPreviewFile {
                source_file: f.name.clone(),
                source_size: f.size,
                season_number: None,
                episode_number: None,
                episode_numbers: Vec::new(),
                episode_title: None,
                destination_path: None,
                matched: false,
                existing_file: false,
                existing_file_size: None,
            });
            continue;
        }

        let parsed_eps = parse_episodes_from_filename(filename);

        if parsed_eps.is_empty() {
            // Try special matching as fallback
            let specials: Vec<(i32, &str)> = all_episodes
                .iter()
                .filter(|e| e.season_number == 0)
                .map(|e| (e.episode_number, e.title.as_str()))
                .collect();
            if let Some((season, ep)) =
                crate::core::scanner::match_special_by_title(filename, &series.title, &specials)
            {
                let matched_ep = all_episodes
                    .iter()
                    .find(|e| e.season_number == season && e.episode_number == ep);
                let has_file = matched_ep
                    .map(|e| episodes_with_files.contains(&e.id))
                    .unwrap_or(false);
                let dest = matched_ep.map(|e| {
                    compute_destination_path(
                        &media_config,
                        &series,
                        season,
                        filename,
                        &[e.clone()],
                        &parsed_info,
                    )
                    .to_string_lossy()
                    .to_string()
                });
                let existing_size = matched_ep
                    .and_then(|e| e.episode_file_id)
                    .and_then(|fid| file_size_map.get(&fid).copied());
                preview_files.push(ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: Some(season),
                    episode_number: Some(ep),
                    episode_numbers: vec![ep],
                    episode_title: matched_ep.map(|e| e.title.clone()),
                    destination_path: dest,
                    matched: true,
                    existing_file: has_file,
                    existing_file_size: existing_size,
                });
            } else {
                preview_files.push(ImportPreviewFile {
                    source_file: f.name.clone(),
                    source_size: f.size,
                    season_number: None,
                    episode_number: None,
                    episode_numbers: Vec::new(),
                    episode_title: None,
                    destination_path: None,
                    matched: false,
                    existing_file: false,
                    existing_file_size: None,
                });
            }
            continue;
        }

        // For each parsed episode (handles multi-episode files)
        let (season, first_ep) = parsed_eps[0];
        let matched_episodes: Vec<_> = parsed_eps
            .iter()
            .filter_map(|&(s, e)| {
                all_episodes
                    .iter()
                    .find(|ep| ep.season_number == s && ep.episode_number == e)
                    .cloned()
            })
            .collect();

        let has_file = matched_episodes
            .iter()
            .any(|e| episodes_with_files.contains(&e.id));

        let dest = if !matched_episodes.is_empty() {
            Some(
                compute_destination_path(
                    &media_config,
                    &series,
                    season,
                    filename,
                    &matched_episodes,
                    &parsed_info,
                )
                .to_string_lossy()
                .to_string(),
            )
        } else {
            None
        };

        let existing_size = matched_episodes
            .first()
            .and_then(|e| e.episode_file_id)
            .and_then(|fid| file_size_map.get(&fid).copied());
        let all_ep_nums: Vec<i32> = matched_episodes.iter().map(|e| e.episode_number).collect();
        preview_files.push(ImportPreviewFile {
            source_file: f.name.clone(),
            source_size: f.size,
            season_number: Some(season),
            episode_number: Some(first_ep),
            episode_numbers: all_ep_nums,
            episode_title: if matched_episodes.len() > 1 {
                Some(
                    matched_episodes
                        .iter()
                        .map(|e| e.title.as_str())
                        .collect::<Vec<_>>()
                        .join(" + "),
                )
            } else {
                matched_episodes.first().map(|e| e.title.clone())
            },
            destination_path: dest,
            matched: !matched_episodes.is_empty(),
            existing_file: has_file,
            existing_file_size: existing_size,
        });
    }

    // Build episode list for manual matching dropdowns
    let preview_episodes: Vec<ImportPreviewEpisode> = all_episodes
        .iter()
        .map(|e| ImportPreviewEpisode {
            id: e.id,
            season_number: e.season_number,
            episode_number: e.episode_number,
            title: e.title.clone(),
            has_file: e.has_file,
            file_size: e
                .episode_file_id
                .and_then(|fid| file_size_map.get(&fid).copied()),
        })
        .collect();

    Ok(Json(ImportPreviewResponse {
        id,
        title,
        content_type: if series.series_type == 2 {
            "anime".to_string()
        } else {
            "series".to_string()
        },
        series: Some(ImportPreviewSeries {
            id: series.id,
            title: series.title,
            path: series.path,
        }),
        movie: None,
        output_path,
        files: preview_files,
        episodes: preview_episodes,
    }))
}
