#![allow(dead_code, unused_imports, unused_variables)]
//! Series API endpoints
//! CRUD operations for TV series

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::SeriesDbModel;
use crate::core::datastore::repositories::{EpisodeFileRepository, SeriesRepository};
use crate::core::mediafiles::{
    compute_file_hash, delete_series_folder, derive_quality_from_media, move_series_folder,
    update_episode_file_paths, MediaAnalyzer,
};
use crate::web::AppState;

/// Query parameters for listing series
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeriesListQuery {
    pub include_season_images: Option<bool>,
}

/// Create series router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            get(list_series)
                .post(create_series)
                .put(update_series_by_body),
        )
        .route(
            "/{id}",
            get(get_series).put(update_series).delete(delete_series),
        )
        .route("/{id}/refresh", post(refresh_series))
        .route("/{id}/rematch", post(rematch_series))
        .route("/{id}/rescan", post(rescan_series))
        .route("/{id}/cleanup", post(cleanup_series))
        .route("/lookup", get(lookup_series))
        .route("/import", post(import_series))
}

/// List all series
async fn list_series(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<SeriesListQuery>,
) -> Result<Json<Vec<SeriesResponse>>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let db_series = repo
        .get_all()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?;

    let mut series_list = Vec::with_capacity(db_series.len());
    for s in db_series {
        let mut response = SeriesResponse::from(s);
        // Enrich with seasons and statistics
        enrich_series_response(&mut response, &state.db).await;
        series_list.push(response);
    }

    Ok(Json(series_list))
}

/// Get a single series by ID
async fn get_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = SeriesResponse::from(series);
    enrich_series_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Enrich a SeriesResponse with seasons and statistics from the database
async fn enrich_series_response(
    response: &mut SeriesResponse,
    db: &crate::core::datastore::Database,
) {
    use sqlx::Row;

    let mut total_episodes = 0i32;
    let mut total_episode_files = 0i32;
    let mut total_size: i64 = 0;

    // Fetch seasons and episode statistics
    // episode_count/episode_file_count only count monitored episodes so the
    // progress bar reflects "what the user cares about". total_episode_count
    // is the unfiltered count for reference.
    let pool = db.pool();
    let mut total_all_episodes = 0i32;
    if let Ok(rows) = sqlx::query(
        r#"
        SELECT
            e.season_number,
            COUNT(DISTINCT e.id)::int as total_episode_count,
            COUNT(DISTINCT CASE WHEN e.monitored = true THEN e.id END)::int as episode_count,
            COUNT(DISTINCT CASE WHEN e.has_file = true AND e.monitored = true THEN e.id END)::int as episode_file_count,
            MAX(CASE WHEN e.monitored = true THEN 1 ELSE 0 END)::int as monitored,
            COALESCE(SUM(ef.size), 0)::bigint as size_on_disk
        FROM episodes e
        LEFT JOIN episode_files ef ON ef.id = e.episode_file_id
        WHERE e.series_id = $1
        GROUP BY e.season_number
        ORDER BY e.season_number
        "#
    )
    .bind(response.id)
    .fetch_all(pool)
    .await {
        response.seasons = rows.iter().map(|row| {
            let total_episode_count: i32 = row.try_get("total_episode_count").unwrap_or(0);
            let episode_count: i32 = row.try_get("episode_count").unwrap_or(0);
            let episode_file_count: i32 = row.try_get("episode_file_count").unwrap_or(0);
            let size_on_disk: i64 = row.try_get("size_on_disk").unwrap_or(0);
            total_all_episodes += total_episode_count;
            total_episodes += episode_count;
            total_episode_files += episode_file_count;
            total_size += size_on_disk;

            SeasonResource {
                season_number: row.try_get("season_number").unwrap_or(0),
                monitored: row.try_get::<i32, _>("monitored").unwrap_or(0) == 1,
                statistics: Some(SeasonStatistics {
                    episode_file_count,
                    episode_count,
                    total_episode_count,
                    percent_of_episodes: if episode_count > 0 {
                        (episode_file_count as f64 / episode_count as f64) * 100.0
                    } else {
                        0.0
                    },
                    size_on_disk,
                }),
            }
        }).collect();
    }

    // Update statistics
    let percent = if total_episodes > 0 {
        (total_episode_files as f64 / total_episodes as f64) * 100.0
    } else {
        0.0
    };

    response.statistics = Some(SeriesStatistics {
        season_count: response.seasons.len() as i32,
        episode_file_count: total_episode_files,
        episode_count: total_episodes,
        total_episode_count: total_all_episodes,
        size_on_disk: total_size,
        release_groups: vec![],
        percent_of_episodes: percent,
    });
}

/// Create a new series
async fn create_series(
    State(state): State<Arc<AppState>>,
    Json(options): Json<CreateSeriesRequest>,
) -> Result<Json<SeriesResponse>, ApiError> {
    // Validate request
    options.validate()?;

    let repo = SeriesRepository::new(state.db.clone());

    // Check if series already exists
    if let Some(_existing) = repo
        .get_by_tvdb_id(options.tvdb_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
    {
        return Err(ApiError::Validation("Series already exists".to_string()));
    }

    // Generate clean title and slug
    let clean_title = clean_title(&options.title);
    let sort_title = clean_title.clone();
    let title_slug = generate_slug(&options.title);

    // Get paths using helper methods
    let full_path = options.get_full_path();
    let root_folder_path = options.get_root_folder_path();

    // Parse first_aired date if provided
    let first_aired = options
        .first_aired
        .as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let series_type = match options.series_type.as_str() {
        "anime" => 1,
        "daily" => 2,
        _ => 0, // standard
    };

    let db_series = SeriesDbModel {
        id: 0, // Will be set by DB
        tvdb_id: options.tvdb_id,
        tv_rage_id: options.tvrage_id.unwrap_or(0),
        tv_maze_id: 0,
        imdb_id: options.imdb_id.clone(),
        tmdb_id: options.tmdb_id.unwrap_or(0),
        title: options.title.clone(),
        clean_title,
        sort_title,
        status: 0, // Continuing
        overview: options.overview.clone(),
        monitored: options.monitored,
        monitor_new_items: 0,
        quality_profile_id: options.quality_profile_id,
        language_profile_id: options.language_profile_id,
        season_folder: options.season_folder,
        series_type,
        title_slug,
        path: full_path,
        root_folder_path,
        year: options.year.unwrap_or(0),
        first_aired,
        last_aired: None,
        runtime: options.runtime.unwrap_or(0),
        network: options.network.clone(),
        certification: options.certification.clone(),
        use_scene_numbering: false,
        added: Utc::now(),
        last_info_sync: None,
        imdb_rating: None,
        imdb_votes: None,
    };

    let id = repo
        .insert(&db_series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create series: {}", e)))?;

    tracing::info!("Created series: id={}, title={}", id, options.title);

    // Spawn background task to refresh series (fetch episodes) and rescan disk
    let db_clone = state.db.clone();
    let metadata_svc = state.metadata_service.clone();
    let series_id = id;
    let series_title = options.title.clone();
    tokio::spawn(async move {
        tracing::info!(
            "Auto-refreshing new series: {} (id={})",
            series_title,
            series_id
        );

        // Trigger refresh to fetch episodes from IMDB + Skyhook
        if let Err(e) = auto_refresh_series(series_id, &db_clone, &metadata_svc).await {
            tracing::error!("Failed to auto-refresh series {}: {}", series_id, e);
        }

        // Trigger disk scan to find existing files
        if let Err(e) = auto_scan_series(series_id, &db_clone).await {
            tracing::error!("Failed to auto-scan series {}: {}", series_id, e);
        }
    });

    // Fetch the created series
    let created = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch created series: {}", e)))?
        .ok_or(ApiError::Internal(
            "Series not found after creation".to_string(),
        ))?;

    let mut response = SeriesResponse::from(created);
    enrich_series_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Import multiple series at once (bulk create)
/// Supports importing from disk where tvdbId may not be known - will lookup via Skyhook
async fn import_series(
    State(state): State<Arc<AppState>>,
    Json(series_list): Json<Vec<ImportSeriesRequest>>,
) -> Result<Json<Vec<SeriesResponse>>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());
    let mut results = Vec::with_capacity(series_list.len());

    for import_req in series_list {
        // Extract folder name and year from path
        let folder_name = import_req
            .path
            .as_ref()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or("")
            .to_string();

        let folder_year = extract_year_from_folder(&folder_name);

        // Get the title to use for lookup (from title field or extract from path)
        let lookup_title = import_req.title.clone().unwrap_or_else(|| {
            // Strip year suffix like " (2020)" from folder name
            if let Some(idx) = folder_name.rfind(" (") {
                folder_name[..idx].to_string()
            } else {
                folder_name.clone()
            }
        });

        if lookup_title.is_empty() {
            tracing::warn!("Skipping import: no title or path provided");
            continue;
        }

        // If tvdbId is not provided or is 0, lookup the series
        let tvdb_id = if import_req.tvdb_id.unwrap_or(0) <= 0 {
            tracing::info!(
                "Looking up series for import: {} (year={:?})",
                lookup_title,
                folder_year
            );
            match lookup_series_by_title_and_year(&lookup_title, folder_year, &state.imdb_client)
                .await
            {
                Ok(Some(result)) => {
                    tracing::info!(
                        "Found series: {} (tvdbId={}, year={:?})",
                        result.title,
                        result.tvdb_id,
                        result.year
                    );
                    result.tvdb_id
                }
                Ok(None) => {
                    tracing::warn!("No series found for: {}", lookup_title);
                    continue;
                }
                Err(e) => {
                    tracing::error!("Failed to lookup series {}: {}", lookup_title, e);
                    continue;
                }
            }
        } else {
            import_req.tvdb_id.unwrap()
        };

        // Check if series already exists
        if let Ok(Some(_existing)) = repo.get_by_tvdb_id(tvdb_id).await {
            tracing::info!("Series already exists, skipping: tvdbId={}", tvdb_id);
            continue;
        }

        // Convert to CreateSeriesRequest for the rest of the logic
        let options = CreateSeriesRequest {
            tvdb_id,
            title: import_req.title.unwrap_or(lookup_title),
            quality_profile_id: import_req.quality_profile_id.unwrap_or(1),
            root_folder_path: import_req.root_folder_path,
            path: import_req.path,
            monitored: import_req.monitored.unwrap_or(true),
            series_type: import_req
                .series_type
                .unwrap_or_else(|| "standard".to_string()),
            season_folder: import_req.season_folder.unwrap_or(true),
            tags: import_req.tags.unwrap_or_default(),
            add_options: AddOptionsRequest::default(),
            year: None,
            overview: None,
            network: None,
            runtime: None,
            imdb_id: None,
            certification: None,
            first_aired: None,
            tvrage_id: None,
            tmdb_id: None,
            language_profile_id: import_req.language_profile_id,
            seasons: vec![],
            images: vec![],
        };

        // Generate clean title and slug
        let clean_title_val = clean_title(&options.title);
        let sort_title = clean_title_val.clone();
        let title_slug = generate_slug(&options.title);

        // Get paths using helper methods
        let full_path = options.get_full_path();
        let root_folder_path = options.get_root_folder_path();

        // Parse first_aired date if provided
        let first_aired = options
            .first_aired
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let series_type = match options.series_type.as_str() {
            "anime" => 1,
            "daily" => 2,
            _ => 0, // standard
        };

        let db_series = SeriesDbModel {
            id: 0,
            tvdb_id: options.tvdb_id,
            tv_rage_id: options.tvrage_id.unwrap_or(0),
            tv_maze_id: 0,
            imdb_id: options.imdb_id.clone(),
            tmdb_id: options.tmdb_id.unwrap_or(0),
            title: options.title.clone(),
            clean_title: clean_title_val,
            sort_title,
            status: 0,
            overview: options.overview.clone(),
            monitored: options.monitored,
            monitor_new_items: 0,
            quality_profile_id: options.quality_profile_id,
            language_profile_id: options.language_profile_id,
            season_folder: options.season_folder,
            series_type,
            title_slug,
            path: full_path,
            root_folder_path,
            year: options.year.unwrap_or(0),
            first_aired,
            last_aired: None,
            runtime: options.runtime.unwrap_or(0),
            network: options.network.clone(),
            certification: options.certification.clone(),
            use_scene_numbering: false,
            added: Utc::now(),
            last_info_sync: None,
            imdb_rating: None,
            imdb_votes: None,
        };

        match repo.insert(&db_series).await {
            Ok(id) => {
                tracing::info!("Imported series: id={}, title={}", id, options.title);
                if let Ok(Some(created)) = repo.get_by_id(id).await {
                    results.push(SeriesResponse::from(created));

                    // Queue auto-refresh for this series
                    let db_clone = state.db.clone();
                    let metadata_svc = state.metadata_service.clone();
                    let series_title = options.title.clone();
                    tokio::spawn(async move {
                        tracing::info!(
                            "Auto-refreshing imported series: {} (id={})",
                            series_title,
                            id
                        );
                        if let Err(e) = auto_refresh_series(id, &db_clone, &metadata_svc).await {
                            tracing::error!("Failed to auto-refresh series {}: {}", id, e);
                        }
                        if let Err(e) = auto_scan_series(id, &db_clone).await {
                            tracing::error!("Failed to auto-scan series {}: {}", id, e);
                        }
                    });
                }
            }
            Err(e) => {
                tracing::error!("Failed to import series {}: {}", options.title, e);
            }
        }
    }

    tracing::info!("Bulk import complete: {} series imported", results.len());
    Ok(Json(results))
}

/// Update an existing series
async fn update_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateSeriesRequest>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let mut series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Apply updates
    if let Some(title) = update.title {
        series.title = title.clone();
        series.clean_title = clean_title(&title);
        series.sort_title = series.clean_title.clone();
    }
    if let Some(monitored) = update.monitored {
        series.monitored = monitored;
    }
    if let Some(quality_profile_id) = update.quality_profile_id {
        series.quality_profile_id = quality_profile_id;
    }
    if let Some(path) = update.path {
        series.path = path;
    }
    if let Some(season_folder) = update.season_folder {
        series.season_folder = season_folder;
    }
    if let Some(series_type) = update.series_type {
        series.series_type = match series_type.as_str() {
            "anime" => 1,
            "daily" => 2,
            _ => 0,
        };
    }

    repo.update(&series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series: {}", e)))?;

    tracing::info!("Updated series: id={}", id);

    Ok(Json(SeriesResponse::from(series)))
}

/// Update a series (ID in body, for compatibility with Sonarr frontend)
/// PUT /api/v5/series?moveFiles=true
async fn update_series_by_body(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UpdateSeriesQuery>,
    Json(body): Json<UpdateSeriesBodyRequest>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());
    let id = body.id;

    let mut series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // If metadata hasn't been synced yet (year=0), refresh before folder operations
    // so the path gets the correct year instead of "(0)"
    if series.year == 0 {
        tracing::info!("Series {} has year=0, refreshing metadata before edit", id);
        if let Err(e) = auto_refresh_series(id, &state.db, &state.metadata_service).await {
            tracing::warn!("Failed to refresh metadata for series {}: {}", id, e);
        } else {
            // Re-fetch series with updated metadata (year, title, path may have changed)
            series = repo
                .get_by_id(id)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
                .ok_or(ApiError::NotFound)?;
        }
    }

    let old_path = series.path.clone();

    // Apply updates from flattened request
    if let Some(title) = body.update.title {
        series.title = title.clone();
        series.clean_title = clean_title(&title);
        series.sort_title = series.clean_title.clone();
    }
    if let Some(monitored) = body.update.monitored {
        series.monitored = monitored;
    }
    if let Some(quality_profile_id) = body.update.quality_profile_id {
        series.quality_profile_id = quality_profile_id;
    }
    if let Some(mut path) = body.update.path {
        // If the incoming path has "(0)" but metadata refresh resolved the year, fix it
        if series.year > 0 && path.contains("(0)") {
            path = path.replace("(0)", &format!("({})", series.year));
        }
        series.path = path;
    }
    if let Some(season_folder) = body.update.season_folder {
        series.season_folder = season_folder;
    }
    if let Some(series_type) = body.update.series_type {
        series.series_type = match series_type.as_str() {
            "anime" => 1,
            "daily" => 2,
            _ => 0,
        };
    }

    // Handle moveFiles if path changed
    if params.move_files && series.path != old_path {
        tracing::info!("Moving series files from {} to {}", old_path, series.path);

        let old_path_ref = std::path::Path::new(&old_path);
        let new_path_ref = std::path::Path::new(&series.path);

        // Move the files
        match move_series_folder(old_path_ref, new_path_ref) {
            Ok(result) => {
                tracing::info!(
                    "Moved {} files, created {} directories",
                    result.files_moved,
                    result.dirs_created
                );

                if !result.errors.is_empty() {
                    for (path, err) in &result.errors {
                        tracing::warn!("Error moving {}: {}", path.display(), err);
                    }
                }

                // Update episode file paths in database
                let file_repo = EpisodeFileRepository::new(state.db.clone());
                if let Ok(mut episode_files) = file_repo.get_by_series_id(id).await {
                    update_episode_file_paths(old_path_ref, new_path_ref, &mut episode_files);

                    // Save updated paths
                    for file in episode_files {
                        if let Err(e) = file_repo.update(&file).await {
                            tracing::warn!("Failed to update episode file path: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to move series files: {}", e);
                return Err(ApiError::Internal(format!("Failed to move files: {}", e)));
            }
        }
    }

    repo.update(&series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series: {}", e)))?;

    tracing::info!("Updated series: id={}, moveFiles={}", id, params.move_files);

    Ok(Json(SeriesResponse::from(series)))
}

/// Delete a series
async fn delete_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(params): Query<DeleteSeriesQuery>,
) -> Result<(), ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    // Check if series exists
    let series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Delete files from disk first (if requested), before removing from database
    if params.delete_files {
        let series_path = std::path::Path::new(&series.path);
        match delete_series_folder(series_path) {
            Ok(file_count) => {
                tracing::info!(
                    "Deleted {} files from series folder: {}",
                    file_count,
                    series.path
                );
            }
            Err(e) => {
                tracing::error!("Failed to delete series folder {}: {}", series.path, e);
                // Continue with database deletion even if file deletion fails
                // User can manually clean up files
            }
        }
    }

    // Delete from database
    repo.delete(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete series: {}", e)))?;

    tracing::info!(
        "Deleted series: id={}, title={}, delete_files={}",
        id,
        series.title,
        params.delete_files
    );

    Ok(())
}

/// Refresh series from metadata sources (IMDB + Skyhook)
async fn refresh_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<SeriesResponse>, ApiError> {
    use crate::core::datastore::models::EpisodeDbModel;
    use crate::core::datastore::repositories::EpisodeRepository;
    use crate::core::metadata::EpisodeEnrichment;

    let repo = SeriesRepository::new(state.db.clone());
    let episode_repo = EpisodeRepository::new(state.db.clone());

    let mut series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!(
        "Refreshing series: {} (TVDB: {})",
        series.title,
        series.tvdb_id
    );

    // Fetch merged metadata from IMDB + Skyhook
    let metadata = state
        .metadata_service
        .fetch_series_metadata(series.tvdb_id, series.imdb_id.as_deref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata: {}", e)))?;

    // Update year first so strip_title_year can use it
    if let Some(year) = metadata.year {
        series.year = year;
    } else if let Some(first_aired) = &metadata.first_aired {
        if let Some(year_str) = first_aired.split('-').next() {
            if let Ok(year) = year_str.parse::<i32>() {
                series.year = year;
            }
        }
    }

    // Update series title from upstream — merge_metadata strips year when Skyhook
    // provides a year field, but some series have year: null with year baked into
    // the title (e.g., "Echo (2023)"). Use the local series year as a fallback.
    let clean_metadata_title = strip_title_year(&metadata.title, series.year);
    series.title = clean_metadata_title.clone();
    series.clean_title = clean_title(&clean_metadata_title);
    series.sort_title = series.clean_title.clone();

    // Update series metadata from merged result
    series.overview = metadata.overview;
    series.status = match metadata
        .status
        .as_deref()
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("continuing") => 0,
        Some("ended") => 1,
        Some("upcoming") => 2,
        _ => 0,
    };
    series.network = metadata.network;
    series.runtime = metadata.runtime.unwrap_or(series.runtime);
    series.certification = metadata.certification;
    // Year was already updated above (before title strip)
    if let Some(first_aired) = &metadata.first_aired {
        series.first_aired = NaiveDate::parse_from_str(first_aired, "%Y-%m-%d").ok();
    }
    // Capture imdb_id from metadata if we don't have one
    if series.imdb_id.is_none() {
        series.imdb_id = metadata.imdb_id.clone();
    }
    // Apply IMDB ratings
    series.imdb_rating = metadata.imdb_rating;
    series.imdb_votes = metadata.imdb_votes;
    series.last_info_sync = Some(Utc::now());

    // Update series in database
    repo.update(&series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series: {}", e)))?;

    // Sync episodes from Skyhook (episodes always come from Skyhook for tvdb_id matching)
    let mut episodes_added = 0;
    let mut episodes_updated = 0;

    for ep in metadata.episodes {
        let air_date = ep
            .air_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let air_date_utc = ep
            .air_date_utc
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let existing = if ep.tvdb_id > 0 {
            // Try tvdb_id first, fall back to season/episode match
            let by_tvdb = episode_repo.get_by_tvdb_id(ep.tvdb_id).await.ok().flatten();
            if by_tvdb.is_some() {
                by_tvdb
            } else {
                episode_repo
                    .get_by_series_season_episode(id, ep.season_number, ep.episode_number)
                    .await
                    .ok()
                    .flatten()
            }
        } else {
            episode_repo
                .get_by_series_season_episode(id, ep.season_number, ep.episode_number)
                .await
                .ok()
                .flatten()
        };

        match existing {
            Some(mut episode) => {
                episode.title = ep
                    .title
                    .unwrap_or_else(|| format!("Episode {}", ep.episode_number));
                episode.overview = ep.overview;
                episode.air_date = air_date;
                episode.air_date_utc = air_date_utc;
                episode.runtime = ep.runtime.unwrap_or(0);
                episode.absolute_episode_number = ep.absolute_episode_number;
                if ep.tvdb_id > 0 {
                    episode.tvdb_id = ep.tvdb_id;
                }

                episode_repo
                    .update(&episode)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Failed to update episode: {}", e)))?;
                episodes_updated += 1;
            }
            None => {
                let episode = EpisodeDbModel {
                    id: 0,
                    series_id: id,
                    tvdb_id: ep.tvdb_id,
                    episode_file_id: None,
                    season_number: ep.season_number,
                    episode_number: ep.episode_number,
                    absolute_episode_number: ep.absolute_episode_number,
                    scene_absolute_episode_number: None,
                    scene_episode_number: None,
                    scene_season_number: None,
                    title: ep
                        .title
                        .unwrap_or_else(|| format!("Episode {}", ep.episode_number)),
                    overview: ep.overview,
                    air_date,
                    air_date_utc,
                    runtime: ep.runtime.unwrap_or(0),
                    has_file: false,
                    monitored: series.monitored,
                    unverified_scene_numbering: false,
                    added: Utc::now(),
                    last_search_time: None,
                    imdb_id: None,
                    imdb_rating: None,
                    imdb_votes: None,
                };

                episode_repo
                    .insert(&episode)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Failed to insert episode: {}", e)))?;
                episodes_added += 1;
            }
        }
    }

    // Enrich episodes with IMDB ratings after Skyhook sync
    if let Some(imdb_id) = &series.imdb_id {
        let all_episodes = episode_repo
            .get_by_series_id(id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to fetch episodes: {}", e)))?;
        let mut enrichments: Vec<EpisodeEnrichment> = all_episodes
            .iter()
            .map(|ep| EpisodeEnrichment {
                season_number: ep.season_number,
                episode_number: ep.episode_number,
                imdb_id: None,
                imdb_rating: None,
                imdb_votes: None,
            })
            .collect();

        if let Ok(enriched) = state
            .metadata_service
            .enrich_episodes_with_imdb(imdb_id, &mut enrichments)
            .await
        {
            if enriched > 0 {
                // Apply enrichments back to DB
                for (ep, enr) in all_episodes.iter().zip(enrichments.iter()) {
                    if enr.imdb_id.is_some() {
                        let mut updated_ep = ep.clone();
                        updated_ep.imdb_id = enr.imdb_id.clone();
                        updated_ep.imdb_rating = enr.imdb_rating;
                        updated_ep.imdb_votes = enr.imdb_votes;
                        let _ = episode_repo.update(&updated_ep).await;
                    }
                }
            }
        }
    }

    tracing::info!(
        "Refreshed series {}: {} episodes added, {} updated",
        series.title,
        episodes_added,
        episodes_updated
    );

    // Return updated series
    let updated = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch updated series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let mut response = SeriesResponse::from(updated);
    enrich_series_response(&mut response, &state.db).await;

    Ok(Json(response))
}

/// Request to re-match a series to a different TVDB/IMDB entry
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RematchRequest {
    tvdb_id: i64,
    imdb_id: Option<String>,
}

/// Re-match a series to a different TVDB/IMDB entry and refresh all metadata.
///
/// Unlike a normal refresh, rematch also updates the title, slug, and clears
/// cached poster images since the series identity itself is changing.
async fn rematch_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<RematchRequest>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let mut series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    tracing::info!(
        "Rematching series '{}' (id={}) from tvdb_id={} to tvdb_id={}, imdb_id={:?}",
        series.title,
        id,
        series.tvdb_id,
        req.tvdb_id,
        req.imdb_id
    );

    // Update external IDs
    series.tvdb_id = req.tvdb_id;
    series.imdb_id = req.imdb_id;

    // Save the updated IDs before refreshing metadata
    repo.update(&series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series IDs: {}", e)))?;

    // Delete old episodes — the series identity is changing, so the old show's
    // episodes are meaningless and would cause duplicate key violations when
    // the new show's episodes are inserted (same season/episode numbers).
    {
        use crate::core::datastore::repositories::EpisodeRepository;
        let episode_repo = EpisodeRepository::new(state.db.clone());
        episode_repo
            .delete_by_series_id(id)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to delete old episodes: {}", e)))?;
        tracing::info!("Deleted old episodes for series id={} before rematch", id);
    }

    // Run the standard metadata refresh (episodes, ratings, overview, etc.)
    if let Err(e) = auto_refresh_series(id, &state.db, &state.metadata_service).await {
        tracing::error!("Failed to refresh after rematch: {}", e);
        return Err(ApiError::Internal(format!(
            "Rematch saved but metadata refresh failed: {}",
            e
        )));
    }

    // Now apply rematch-specific updates that auto_refresh_series doesn't do:
    // update title, slug, and clear cached images.
    // Fetch the metadata again to get the canonical title from Skyhook.
    let mut series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    let metadata = state
        .metadata_service
        .fetch_series_metadata(series.tvdb_id, series.imdb_id.as_deref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch metadata for title: {}", e)))?;

    // Update title from the new TVDB entry (strip year suffix)
    let new_title = strip_title_year(&metadata.title, series.year);
    series.title = new_title.clone();
    series.clean_title = clean_title(&new_title);
    series.sort_title = series.clean_title.clone();

    // Regenerate slug with year for disambiguation (e.g., "revenge-2011")
    let slug_base = if series.year > 0 {
        format!("{} {}", new_title, series.year)
    } else {
        new_title.clone()
    };
    series.title_slug = generate_slug(&slug_base);

    repo.update(&series)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to update series title/slug: {}", e)))?;

    // Clear cached poster/fanart/banner images so fresh ones are fetched from the new TVDB ID
    let cache_dir = format!("cache/MediaCover/Series/{}", id);
    if let Err(e) = tokio::fs::remove_dir_all(&cache_dir).await {
        // Not fatal — cache miss will just re-fetch from Skyhook
        tracing::debug!("Could not clear image cache at {}: {}", cache_dir, e);
    } else {
        tracing::info!("Cleared image cache for series id={}", id);
    }

    // Return the fully-updated series
    let mut response = SeriesResponse::from(series);
    enrich_series_response(&mut response, &state.db).await;

    tracing::info!("Rematch complete for series id={}", id);

    Ok(Json(response))
}

/// Skyhook show response (for refresh)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookShowResponse {
    tvdb_id: i64,
    title: String,
    overview: Option<String>,
    status: Option<String>,
    year: Option<i32>,
    first_aired: Option<String>,
    runtime: Option<i32>,
    network: Option<String>,
    content_rating: Option<String>,
    genres: Option<Vec<String>>,
    images: Option<Vec<SkyhookImage>>,
    seasons: Option<Vec<SkyhookSeason>>,
    episodes: Option<Vec<SkyhookEpisode>>,
    rating: Option<SkyhookRatings>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookEpisode {
    tvdb_id: i64,
    season_number: i32,
    episode_number: i32,
    absolute_episode_number: Option<i32>,
    title: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
    air_date_utc: Option<String>,
    runtime: Option<i32>,
}

/// Rescan series files
async fn rescan_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // TODO: Queue a disk scan command

    Ok(Json(SeriesResponse::from(series)))
}

/// POST /api/v5/series/{id}/cleanup — Remove orphan duplicate files from series folder
///
/// Walks the series folder for video files not tracked in the DB. For each orphan,
/// computes a BLAKE3 hash and compares against tracked files. If a match is found,
/// the orphan is a leftover duplicate and is deleted.
async fn cleanup_series(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<CleanupResult>, ApiError> {
    let repo = SeriesRepository::new(state.db.clone());

    let series = repo
        .get_by_id(id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch series: {}", e)))?
        .ok_or(ApiError::NotFound)?;

    // Step 1: Clean up orphan DB records (not referenced by any episode)
    let db_cleaned = sqlx::query(
        "DELETE FROM episode_files WHERE series_id = $1
         AND id NOT IN (SELECT episode_file_id FROM episodes WHERE episode_file_id IS NOT NULL AND series_id = $1)"
    )
    .bind(id)
    .execute(state.db.pool())
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    if db_cleaned > 0 {
        tracing::info!(
            "Cleanup: removed {} orphan DB records for series {}",
            db_cleaned,
            id
        );
    }

    // Step 2: Get all tracked file paths (via episodes, not series_id, to handle cross-series file links)
    let tracked_paths_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT ef.path FROM episode_files ef
         JOIN episodes e ON e.episode_file_id = ef.id
         WHERE e.series_id = $1",
    )
    .bind(id)
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();
    let tracked_paths: std::collections::HashSet<String> =
        tracked_paths_rows.into_iter().map(|(p,)| p).collect();

    // Step 3: Walk the series folder recursively for untracked video files
    let series_path = std::path::Path::new(&series.path);
    let mut orphan_paths = Vec::new();
    collect_orphan_videos(series_path, &tracked_paths, &mut orphan_paths).await;

    if orphan_paths.is_empty() {
        return Ok(Json(CleanupResult {
            orphan_db_records_removed: db_cleaned,
            orphan_files_removed: 0,
            orphan_files_found: 0,
            errors: vec![],
        }));
    }

    // Step 4: Build size→path index of tracked files for fast matching
    let mut tracked_by_size: std::collections::HashMap<u64, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    for path_str in &tracked_paths {
        let tf_path = std::path::Path::new(path_str);
        if let Ok(meta) = tokio::fs::metadata(tf_path).await {
            tracked_by_size
                .entry(meta.len())
                .or_default()
                .push(tf_path.to_path_buf());
        }
    }

    // Step 5: For each orphan, check size match first, then hash-confirm
    let orphan_count = orphan_paths.len() as u64;
    let mut removed = 0u64;
    let mut errors = Vec::new();

    for orphan in &orphan_paths {
        let orphan_size = match tokio::fs::metadata(orphan).await {
            Ok(meta) => meta.len(),
            Err(e) => {
                errors.push(format!("Failed to stat {}: {}", orphan.display(), e));
                continue;
            }
        };

        // Only hash if a tracked file has the exact same size
        let size_matches = match tracked_by_size.get(&orphan_size) {
            Some(paths) => paths.clone(),
            None => continue, // No tracked file with same size — not a duplicate
        };

        // Hash the orphan
        let orphan_hash = match compute_file_hash(orphan).await {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("Failed to hash {}: {}", orphan.display(), e));
                continue;
            }
        };

        // Hash the size-matched tracked files and compare
        let mut is_duplicate = false;
        for tracked_path in &size_matches {
            if let Ok(tracked_hash) = compute_file_hash(tracked_path).await {
                if tracked_hash == orphan_hash {
                    is_duplicate = true;
                    break;
                }
            }
        }

        if is_duplicate {
            if let Err(e) = tokio::fs::remove_file(orphan).await {
                errors.push(format!("Failed to remove {}: {}", orphan.display(), e));
            } else {
                tracing::info!("Cleanup: removed orphan duplicate {}", orphan.display());
                removed += 1;
            }
        }
    }

    if removed > 0 {
        crate::core::logging::log_info(
            "SeriesCleanup",
            &format!(
                "Cleaned up series '{}': {} orphan DB records, {} orphan files removed",
                series.title, db_cleaned, removed
            ),
        )
        .await;
    }

    Ok(Json(CleanupResult {
        orphan_db_records_removed: db_cleaned,
        orphan_files_removed: removed,
        orphan_files_found: orphan_count,
        errors,
    }))
}

/// Recursively collect video files not in the tracked set
async fn collect_orphan_videos(
    dir: &std::path::Path,
    tracked: &std::collections::HashSet<String>,
    orphans: &mut Vec<std::path::PathBuf>,
) {
    let mut walker = match tokio::fs::read_dir(dir).await {
        Ok(w) => w,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = walker.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(collect_orphan_videos(&path, tracked, orphans)).await;
        } else if path.is_file() && crate::core::scanner::is_video_file(&path) {
            let path_str = path.to_string_lossy().to_string();
            if !tracked.contains(&path_str) {
                orphans.push(path);
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub orphan_db_records_removed: u64,
    pub orphan_files_removed: u64,
    pub orphan_files_found: u64,
    pub errors: Vec<String>,
}

/// Lookup series by name using Skyhook (Sonarr's metadata service)
async fn lookup_series(
    Query(params): Query<LookupQuery>,
) -> Result<Json<Vec<SeriesLookupResult>>, ApiError> {
    let term = &params.term;

    // Query Sonarr's Skyhook service (their public metadata API)
    let url = format!(
        "http://skyhook.sonarr.tv/v1/tvdb/search/en/?term={}",
        urlencoding::encode(term)
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "Sonarr-rs/0.1.0")
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to query Skyhook: {}", e)))?;

    if !response.status().is_success() {
        return Err(ApiError::Internal(format!(
            "Skyhook returned status: {}",
            response.status()
        )));
    }

    let skyhook_results: Vec<SkyhookSearchResult> = response
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse Skyhook response: {}", e)))?;

    // Convert Skyhook results to our format
    let results: Vec<SeriesLookupResult> = skyhook_results
        .into_iter()
        .map(|s| SeriesLookupResult {
            tvdb_id: s.tvdb_id,
            title: s.title,
            sort_title: s.sort_title.unwrap_or_default(),
            overview: s.overview,
            year: s.year.unwrap_or(0),
            status: s.status.unwrap_or_else(|| "unknown".to_string()),
            images: s
                .images
                .unwrap_or_default()
                .into_iter()
                .map(|img| SeriesImage {
                    cover_type: img.cover_type,
                    url: img.url,
                })
                .collect(),
            seasons: s
                .seasons
                .unwrap_or_default()
                .into_iter()
                .map(|season| SeasonResource {
                    season_number: season.season_number,
                    monitored: true,
                    statistics: None,
                })
                .collect(),
            ratings: Ratings {
                votes: s.ratings.as_ref().and_then(|r| r.votes).unwrap_or(0),
                value: s.ratings.as_ref().and_then(|r| r.value).unwrap_or(0.0),
            },
            genres: s.genres.unwrap_or_default(),
            network: s.network,
            runtime: s.runtime.unwrap_or(0),
            tvrage_id: s.tvrage_id.unwrap_or(0),
            imdb_id: s.imdb_id,
            certification: s.certification,
            first_aired: s.first_aired,
        })
        .collect();

    Ok(Json(results))
}

// Helper functions

/// Strip trailing ` (YYYY)` from a title when it matches the given year.
/// Used as a secondary pass after merge_metadata — catches cases where Skyhook
/// returns year: null but embeds the year in the title for disambiguation.
fn strip_title_year(title: &str, year: i32) -> String {
    if year > 0 {
        let suffix = format!(" ({})", year);
        if title.ends_with(&suffix) {
            return title[..title.len() - suffix.len()].to_string();
        }
    }
    title.to_string()
}

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

/// Extract a year (1900-2099) from a folder name like "The Flash (2014)" or "The.Flash.2014"
fn extract_year_from_folder(folder: &str) -> Option<i32> {
    static FOLDER_YEAR_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"[\s.\(_-]((?:19|20)\d{2})[\s.\)_-]?$").expect("valid regex")
    });
    // Also try parenthesized year anywhere: "Show (2014)"
    static PAREN_YEAR_RE: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"\((\d{4})\)").expect("valid regex"));

    if let Some(caps) = PAREN_YEAR_RE.captures(folder) {
        if let Ok(y) = caps[1].parse::<i32>() {
            if (1900..2100).contains(&y) {
                return Some(y);
            }
        }
    }
    if let Some(caps) = FOLDER_YEAR_RE.captures(folder) {
        if let Ok(y) = caps[1].parse::<i32>() {
            if (1900..2100).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

/// Lookup a series by title (and optional year) using IMDB-first, Skyhook-fallback strategy.
///
/// 1. If IMDB is enabled, search IMDB and filter by year for the best match.
///    If an IMDB match is found, search Skyhook by the IMDB title+year to get the tvdb_id.
/// 2. Fall back to Skyhook search, sorting results by year proximity instead of
///    blindly taking the first result.
async fn lookup_series_by_title_and_year(
    title: &str,
    year: Option<i32>,
    imdb_client: &crate::core::imdb::ImdbClient,
) -> Result<Option<SkyhookSearchResult>, String> {
    // --- IMDB-first path ---
    if imdb_client.is_enabled() {
        if let Ok(imdb_results) = imdb_client.search_series(title, 10).await {
            if !imdb_results.is_empty() {
                // Pick the IMDB result whose start_year best matches
                let best = if let Some(yr) = year {
                    imdb_results
                        .iter()
                        .filter(|s| s.start_year.is_some())
                        .min_by_key(|s| (s.start_year.unwrap_or(0) - yr).unsigned_abs())
                        .or(imdb_results.first())
                } else {
                    imdb_results.first()
                };

                if let Some(imdb_match) = best {
                    // Validate year if we have one
                    let year_ok = match (year, imdb_match.start_year) {
                        (Some(y), Some(sy)) => (y - sy).abs() <= 1,
                        _ => true,
                    };

                    if year_ok {
                        tracing::info!(
                            "IMDB match: {} ({}) [{}]",
                            imdb_match.title,
                            imdb_match.start_year.unwrap_or(0),
                            imdb_match.imdb_id,
                        );

                        // Now find the corresponding Skyhook entry using refined search
                        let skyhook_term = if let Some(sy) = imdb_match.start_year {
                            format!("{} {}", imdb_match.title, sy)
                        } else {
                            imdb_match.title.clone()
                        };

                        if let Ok(Some(skyhook)) =
                            search_skyhook_by_title_year(&skyhook_term, imdb_match.start_year).await
                        {
                            return Ok(Some(skyhook));
                        }
                    }
                }
            }
        }
    }

    // --- Skyhook fallback ---
    search_skyhook_by_title_year(title, year).await
}

/// Search Skyhook and pick the result whose year best matches the expected year.
async fn search_skyhook_by_title_year(
    title: &str,
    year: Option<i32>,
) -> Result<Option<SkyhookSearchResult>, String> {
    let url = format!(
        "http://skyhook.sonarr.tv/v1/tvdb/search/en/?term={}",
        urlencoding::encode(title)
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "Sonarr-rs/0.1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to query Skyhook: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Skyhook returned status: {}", response.status()));
    }

    let results: Vec<SkyhookSearchResult> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Skyhook response: {}", e))?;

    if results.is_empty() {
        return Ok(None);
    }

    // If we have a year, sort by year proximity; otherwise take the first result
    match year {
        Some(yr) => {
            let best = results.into_iter().min_by_key(|s| {
                let s_year = s.year.unwrap_or(0);
                if s_year == 0 {
                    // No year info — deprioritize but don't eliminate
                    1000i32
                } else {
                    (s_year - yr).abs()
                }
            });
            Ok(best)
        }
        None => Ok(results.into_iter().next()),
    }
}

/// Auto-refresh a series: fetch episodes from IMDB + Skyhook
async fn auto_refresh_series(
    series_id: i64,
    db: &crate::core::datastore::Database,
    metadata_service: &crate::core::metadata::MetadataService,
) -> Result<(), String> {
    use crate::core::datastore::models::EpisodeDbModel;
    use crate::core::datastore::repositories::{EpisodeRepository, SeriesRepository};
    use crate::core::metadata::EpisodeEnrichment;

    let series_repo = SeriesRepository::new(db.clone());
    let episode_repo = EpisodeRepository::new(db.clone());

    let mut series = series_repo
        .get_by_id(series_id)
        .await
        .map_err(|e| format!("Failed to fetch series: {}", e))?
        .ok_or_else(|| "Series not found".to_string())?;

    tracing::info!(
        "Fetching episodes for {} (TVDB: {})",
        series.title,
        series.tvdb_id
    );

    // Fetch merged metadata from IMDB + Skyhook
    let metadata = metadata_service
        .fetch_series_metadata(series.tvdb_id, series.imdb_id.as_deref())
        .await
        .map_err(|e| format!("Failed to fetch metadata: {}", e))?;

    // Update year first so strip_title_year can use it
    let old_year = series.year;
    if let Some(year) = metadata.year {
        series.year = year;
    } else if let Some(first_aired) = &metadata.first_aired {
        if let Some(year_str) = first_aired.split('-').next() {
            if let Ok(year) = year_str.parse::<i32>() {
                series.year = year;
            }
        }
    }

    // If year was 0 (unresolved) and we now have a real year, fix the path
    if old_year == 0 && series.year > 0 && series.path.contains("(0)") {
        let old_path = series.path.clone();
        series.path = series.path.replace("(0)", &format!("({})", series.year));
        tracing::info!("Fixed series path year: {} -> {}", old_path, series.path);
    }

    // Update series title from upstream — strip year suffix using local series year
    let clean_metadata_title = strip_title_year(&metadata.title, series.year);
    series.title = clean_metadata_title.clone();
    series.clean_title = clean_title(&clean_metadata_title);
    series.sort_title = series.clean_title.clone();

    // Update series metadata
    series.overview = metadata.overview;
    series.status = match metadata
        .status
        .as_deref()
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("continuing") => 0,
        Some("ended") => 1,
        Some("upcoming") => 2,
        _ => series.status,
    };
    series.network = metadata.network;
    series.runtime = metadata.runtime.unwrap_or(series.runtime);
    series.certification = metadata.certification;
    if let Some(first_aired) = &metadata.first_aired {
        series.first_aired = NaiveDate::parse_from_str(first_aired, "%Y-%m-%d").ok();
    }
    // Capture imdb_id if discovered
    if series.imdb_id.is_none() {
        series.imdb_id = metadata.imdb_id.clone();
    }
    series.imdb_rating = metadata.imdb_rating;
    series.imdb_votes = metadata.imdb_votes;
    series.last_info_sync = Some(Utc::now());

    series_repo
        .update(&series)
        .await
        .map_err(|e| format!("Failed to update series: {}", e))?;

    // Sync episodes
    let mut added = 0;
    let mut updated = 0;
    for ep in metadata.episodes {
        let air_date = ep
            .air_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let air_date_utc = ep
            .air_date_utc
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let existing = episode_repo
            .get_by_series_season_episode(series_id, ep.season_number, ep.episode_number)
            .await
            .ok()
            .flatten();

        let title = ep
            .title
            .unwrap_or_else(|| format!("Episode {}", ep.episode_number));

        if let Some(mut existing_ep) = existing {
            // Update metadata from Skyhook, preserve local state
            existing_ep.tvdb_id = ep.tvdb_id;
            existing_ep.title = title;
            existing_ep.overview = ep.overview;
            existing_ep.air_date = air_date;
            existing_ep.air_date_utc = air_date_utc;
            existing_ep.runtime = ep.runtime.unwrap_or(0);
            existing_ep.absolute_episode_number = ep.absolute_episode_number;
            if episode_repo.update(&existing_ep).await.is_ok() {
                updated += 1;
            }
        } else {
            let episode = EpisodeDbModel {
                id: 0,
                series_id,
                tvdb_id: ep.tvdb_id,
                episode_file_id: None,
                season_number: ep.season_number,
                episode_number: ep.episode_number,
                absolute_episode_number: ep.absolute_episode_number,
                scene_absolute_episode_number: None,
                scene_episode_number: None,
                scene_season_number: None,
                title,
                overview: ep.overview,
                air_date,
                air_date_utc,
                runtime: ep.runtime.unwrap_or(0),
                has_file: false,
                monitored: series.monitored,
                unverified_scene_numbering: false,
                added: Utc::now(),
                last_search_time: None,
                imdb_id: None,
                imdb_rating: None,
                imdb_votes: None,
            };
            if episode_repo.insert(&episode).await.is_ok() {
                added += 1;
            }
        }
    }

    // Enrich episodes with IMDB ratings
    if let Some(imdb_id) = &series.imdb_id {
        let all_episodes = episode_repo
            .get_by_series_id(series_id)
            .await
            .map_err(|e| format!("Failed to fetch episodes: {}", e))?;
        let mut enrichments: Vec<EpisodeEnrichment> = all_episodes
            .iter()
            .map(|ep| EpisodeEnrichment {
                season_number: ep.season_number,
                episode_number: ep.episode_number,
                imdb_id: None,
                imdb_rating: None,
                imdb_votes: None,
            })
            .collect();

        if let Ok(enriched) = metadata_service
            .enrich_episodes_with_imdb(imdb_id, &mut enrichments)
            .await
        {
            if enriched > 0 {
                for (ep, enr) in all_episodes.iter().zip(enrichments.iter()) {
                    if enr.imdb_id.is_some() {
                        let mut updated_ep = ep.clone();
                        updated_ep.imdb_id = enr.imdb_id.clone();
                        updated_ep.imdb_rating = enr.imdb_rating;
                        updated_ep.imdb_votes = enr.imdb_votes;
                        let _ = episode_repo.update(&updated_ep).await;
                    }
                }
            }
        }
    }

    tracing::info!(
        "Synced episodes for {}: {} added, {} updated",
        series.title,
        added,
        updated
    );
    Ok(())
}

/// Auto-scan a series: scan disk for existing episode files
async fn auto_scan_series(
    series_id: i64,
    db: &crate::core::datastore::Database,
) -> Result<(), String> {
    use crate::core::datastore::repositories::{
        EpisodeFileRepository, EpisodeRepository, SeriesRepository,
    };
    use std::path::Path;

    let series_repo = SeriesRepository::new(db.clone());
    let episode_repo = EpisodeRepository::new(db.clone());

    let series = series_repo
        .get_by_id(series_id)
        .await
        .map_err(|e| format!("Failed to fetch series: {}", e))?
        .ok_or_else(|| "Series not found".to_string())?;

    let series_path = Path::new(&series.path);
    if !series_path.exists() {
        tracing::info!("Series path does not exist yet: {}", series.path);
        return Ok(());
    }

    tracing::info!("Scanning disk for {}: {}", series.title, series.path);

    // Get all episodes for this series
    let episodes = episode_repo
        .get_by_series_id(series_id)
        .await
        .map_err(|e| format!("Failed to fetch episodes: {}", e))?;

    // Walk the series directory looking for video files
    let mut episodes_matched = 0;

    fn scan_directory(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_directory(&path, files);
                } else if is_video_file(&path) {
                    files.push(path);
                }
            }
        }
    }

    fn is_video_file(path: &Path) -> bool {
        crate::core::scanner::is_video_file(path)
    }

    let mut video_files = Vec::new();
    scan_directory(series_path, &mut video_files);
    let files_found = video_files.len();

    // Get existing episode files to avoid duplicates
    let file_repo = EpisodeFileRepository::new(db.clone());
    let existing_files = file_repo
        .get_by_series_id(series_id)
        .await
        .unwrap_or_default();
    let existing_paths: std::collections::HashSet<_> =
        existing_files.iter().map(|f| f.path.clone()).collect();

    // Try to match files to episodes using filename parsing
    for file_path in &video_files {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Skip if file already exists in database
        if existing_paths.contains(&file_path_str) {
            continue;
        }

        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            // Parse season and episode from filename (e.g., "S01E01", "1x01", etc.)
            if let Some((season, episode)) = parse_season_episode(file_name) {
                // Find matching episode
                if let Some(mut ep) = episodes
                    .iter()
                    .find(|e| e.season_number == season && e.episode_number == episode)
                    .cloned()
                {
                    // Get file size
                    let file_size = std::fs::metadata(file_path)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);

                    // Calculate relative path from series folder
                    let relative_path = file_path
                        .strip_prefix(series_path)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| file_name.to_string());

                    // Real media analysis via FFmpeg probe
                    let media_info_result = MediaAnalyzer::analyze(file_path).await;
                    let media_info = media_info_result
                        .as_ref()
                        .ok()
                        .and_then(|info| serde_json::to_string(info).ok());

                    // Derive quality from actual resolution, fallback to filename
                    let quality_str = match &media_info_result {
                        Ok(info) => {
                            let quality = derive_quality_from_media(info, file_name);
                            serde_json::to_string(&quality).unwrap_or_else(|_| {
                                r#"{"quality":{"id":1,"name":"HDTV-720p"}}"#.to_string()
                            })
                        }
                        Err(_) => {
                            let quality = parse_quality_from_filename(file_name);
                            serde_json::to_string(&quality).unwrap_or_else(|_| {
                                r#"{"quality":{"id":1,"name":"HDTV-720p"}}"#.to_string()
                            })
                        }
                    };

                    // BLAKE3 file hash
                    let file_hash = compute_file_hash(file_path).await.ok();

                    // Create episode file record
                    let episode_file = crate::core::datastore::models::EpisodeFileDbModel {
                        id: 0,
                        series_id,
                        season_number: season,
                        relative_path,
                        path: file_path_str.clone(),
                        size: file_size,
                        date_added: Utc::now(),
                        scene_name: Some(file_name.to_string()),
                        release_group: parse_release_group(file_name),
                        quality: quality_str,
                        languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                        media_info,
                        original_file_path: Some(file_path_str.clone()),
                        file_hash,
                    };

                    // Insert episode file
                    if let Ok(file_id) = file_repo.insert(&episode_file).await {
                        // Create mapping from episode to file
                        let pool = db.pool();
                        let _ = sqlx::query(
                            "INSERT INTO episode_file_mapping (episode_file_id, episode_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                        )
                        .bind(file_id)
                        .bind(ep.id)
                        .execute(pool)
                        .await;

                        // Update episode has_file flag
                        if !ep.has_file {
                            ep.has_file = true;
                            let _ = episode_repo.update(&ep).await;
                        }

                        episodes_matched += 1;
                    }
                }
            }
        }
    }

    tracing::info!(
        "Disk scan complete for {}: {} files found, {} episodes matched",
        series.title,
        files_found,
        episodes_matched
    );

    Ok(())
}

/// Parse quality from filename
pub fn parse_quality_from_filename(filename: &str) -> serde_json::Value {
    let filename_lower = filename.to_lowercase();

    let (id, name) = if filename_lower.contains("2160p") || filename_lower.contains("4k") {
        (19, "HDTV-2160p")
    } else if filename_lower.contains("1080p") {
        if filename_lower.contains("bluray") || filename_lower.contains("bdrip") {
            (7, "Bluray-1080p")
        } else if filename_lower.contains("web") {
            (3, "WEBDL-1080p")
        } else {
            (9, "HDTV-1080p")
        }
    } else if filename_lower.contains("720p") {
        if filename_lower.contains("bluray") || filename_lower.contains("bdrip") {
            (6, "Bluray-720p")
        } else if filename_lower.contains("web") {
            (5, "WEBDL-720p")
        } else {
            (4, "HDTV-720p")
        }
    } else if filename_lower.contains("480p") || filename_lower.contains("dvd") {
        (2, "DVD")
    } else {
        (1, "SDTV")
    };

    serde_json::json!({
        "quality": { "id": id, "name": name },
        "revision": { "version": 1, "real": 0, "isRepack": false }
    })
}

/// Parse release group from filename
pub fn parse_release_group(filename: &str) -> Option<String> {
    // Look for release group at end of filename (before extension)
    // Common pattern: "Show.Name.S01E01.1080p.WEB-DL.x264-GROUP.mkv"
    let name = filename.rsplit('.').nth(1)?; // Remove extension
    if let Some(pos) = name.rfind('-') {
        let group = &name[pos + 1..];
        if !group.is_empty() && group.len() < 20 {
            return Some(group.to_string());
        }
    }
    None
}

/// Parse season and episode numbers from filename
fn parse_season_episode(filename: &str) -> Option<(i32, i32)> {
    // Try S01E01 format (allows optional separator between S##/E##: "S02 E01", "S02.E01", "S02E01")
    let re_sxxexx = regex::Regex::new(r"[Ss](\d{1,2})[\s._-]*[Ee](\d{1,3})").ok()?;
    if let Some(caps) = re_sxxexx.captures(filename) {
        let season: i32 = caps.get(1)?.as_str().parse().ok()?;
        let episode: i32 = caps.get(2)?.as_str().parse().ok()?;
        return Some((season, episode));
    }

    // Try 1x01 format
    let re_nxnn = regex::Regex::new(r"(\d{1,2})x(\d{1,2})").ok()?;
    if let Some(caps) = re_nxnn.captures(filename) {
        let season: i32 = caps.get(1)?.as_str().parse().ok()?;
        let episode: i32 = caps.get(2)?.as_str().parse().ok()?;
        return Some((season, episode));
    }

    // Try Season 1 Episode 1 format
    let re_verbose = regex::Regex::new(r"[Ss]eason\s*(\d+).*[Ee]pisode\s*(\d+)").ok()?;
    if let Some(caps) = re_verbose.captures(filename) {
        let season: i32 = caps.get(1)?.as_str().parse().ok()?;
        let episode: i32 = caps.get(2)?.as_str().parse().ok()?;
        return Some((season, episode));
    }

    None
}

/// Skyhook API response structures
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSearchResult {
    tvdb_id: i64,
    title: String,
    sort_title: Option<String>,
    overview: Option<String>,
    year: Option<i32>,
    status: Option<String>,
    images: Option<Vec<SkyhookImage>>,
    seasons: Option<Vec<SkyhookSeason>>,
    ratings: Option<SkyhookRatings>,
    genres: Option<Vec<String>>,
    network: Option<String>,
    runtime: Option<i32>,
    tvrage_id: Option<i64>,
    imdb_id: Option<String>,
    certification: Option<String>,
    first_aired: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookImage {
    cover_type: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookSeason {
    season_number: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkyhookRatings {
    votes: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_rating_value")]
    value: Option<f64>,
}

/// Deserialize rating value that could be a string or a number
fn deserialize_rating_value<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_f64()),
        Some(serde_json::Value::String(s)) => s
            .parse::<f64>()
            .map(Some)
            .map_err(|_| D::Error::custom("invalid rating value")),
        _ => Ok(None),
    }
}

// Request/Response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesRequest {
    pub tvdb_id: i64,
    pub title: String,
    pub quality_profile_id: i64,
    #[serde(default)]
    pub root_folder_path: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub monitored: bool,
    #[serde(default = "default_series_type")]
    pub series_type: String,
    #[serde(default)]
    pub season_folder: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    #[serde(default)]
    pub add_options: AddOptionsRequest,
    // Optional fields from lookup
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub network: Option<String>,
    pub runtime: Option<i32>,
    pub imdb_id: Option<String>,
    pub certification: Option<String>,
    pub first_aired: Option<String>,
    pub tvrage_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub language_profile_id: Option<i64>,
    #[serde(default)]
    pub seasons: Vec<SeasonResource>,
    #[serde(default)]
    pub images: Vec<SeriesImage>,
}

fn default_series_type() -> String {
    "standard".to_string()
}

impl CreateSeriesRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.tvdb_id <= 0 {
            return Err(ApiError::Validation("tvdbId must be positive".to_string()));
        }
        if self.title.is_empty() {
            return Err(ApiError::Validation("title is required".to_string()));
        }
        // Either path or rootFolderPath must be provided
        if self.path.is_none() && self.root_folder_path.as_ref().is_none_or(|s| s.is_empty()) {
            return Err(ApiError::Validation(
                "path or rootFolderPath is required".to_string(),
            ));
        }
        Ok(())
    }

    /// Get the full path for this series (either from path field or constructed from rootFolderPath)
    fn get_full_path(&self) -> String {
        if let Some(path) = &self.path {
            if !path.is_empty() {
                return path.clone();
            }
        }
        // Construct from rootFolderPath
        let root = self.root_folder_path.as_deref().unwrap_or("");
        let folder_name = format!("{} ({})", self.title, self.year.unwrap_or(0));
        format!("{}/{}", root.trim_end_matches('/'), folder_name)
    }

    /// Get the root folder path (either from rootFolderPath field or extracted from path)
    fn get_root_folder_path(&self) -> String {
        if let Some(root) = &self.root_folder_path {
            if !root.is_empty() {
                return root.clone();
            }
        }
        // Extract from path by removing the last component
        if let Some(path) = &self.path {
            if let Some(idx) = path.rfind('/') {
                return path[..idx].to_string();
            }
        }
        String::new()
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddOptionsRequest {
    #[serde(default)]
    pub ignore_episodes_with_files: bool,
    #[serde(default)]
    pub ignore_episodes_without_files: bool,
    #[serde(default)]
    pub search_for_missing_episodes: bool,
}

/// Request for importing series from disk
/// tvdbId is optional - if not provided, will lookup by title
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSeriesRequest {
    #[serde(default)]
    pub tvdb_id: Option<i64>,
    pub title: Option<String>,
    pub path: Option<String>,
    pub root_folder_path: Option<String>,
    pub quality_profile_id: Option<i64>,
    pub language_profile_id: Option<i64>,
    #[serde(default)]
    pub monitored: Option<bool>,
    pub series_type: Option<String>,
    #[serde(default)]
    pub season_folder: Option<bool>,
    #[serde(default)]
    pub tags: Option<Vec<i64>>,
}

/// Query parameters for series update (moveFiles)
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSeriesQuery {
    #[serde(default)]
    pub move_files: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSeriesRequest {
    pub title: Option<String>,
    pub monitored: Option<bool>,
    pub quality_profile_id: Option<i64>,
    pub path: Option<String>,
    pub season_folder: Option<bool>,
    pub series_type: Option<String>,
}

/// Full series update request (includes ID in body)
/// Used when PUT is sent to /series without ID in path
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSeriesBodyRequest {
    pub id: i64,
    #[serde(flatten)]
    pub update: UpdateSeriesRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSeriesQuery {
    #[serde(default)]
    pub delete_files: bool,
    #[serde(default)]
    pub add_import_list_exclusion: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub term: String,
}

/// Full series response matching Sonarr API v3/v5
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResponse {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub alternate_titles: Vec<AlternateTitleResource>,
    pub sort_title: String,
    pub status: String,
    pub ended: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_airing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_airing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_time: Option<String>,
    pub images: Vec<SeriesImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_language: Option<LanguageResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_poster: Option<String>,
    pub seasons: Vec<SeasonResource>,
    pub year: i32,
    pub path: String,
    pub quality_profile_id: i64,
    pub season_folder: bool,
    pub monitored: bool,
    pub monitor_new_items: String,
    pub use_scene_numbering: bool,
    pub runtime: i32,
    pub tvdb_id: i64,
    pub tv_rage_id: i64,
    pub tv_maze_id: i64,
    pub tmdb_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_aired: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_aired: Option<String>,
    pub series_type: String,
    pub clean_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    pub title_slug: String,
    pub root_folder_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certification: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<i32>,
    pub added: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_options: Option<AddOptionsResource>,
    pub ratings: Ratings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_rating: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_votes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<SeriesStatistics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episodes_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_profile_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitleResource {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_season_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddOptionsResource {
    #[serde(default)]
    pub ignore_episodes_with_files: bool,
    #[serde(default)]
    pub ignore_episodes_without_files: bool,
    #[serde(default)]
    pub monitor: String,
    #[serde(default)]
    pub search_for_missing_episodes: bool,
    #[serde(default)]
    pub search_for_cutoff_unmet_episodes: bool,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatistics {
    pub season_count: i32,
    pub episode_file_count: i32,
    pub episode_count: i32,
    pub total_episode_count: i32,
    pub size_on_disk: i64,
    #[serde(default)]
    pub release_groups: Vec<String>,
    pub percent_of_episodes: f64,
}

impl From<SeriesDbModel> for SeriesResponse {
    fn from(s: SeriesDbModel) -> Self {
        let status = match s.status {
            0 => "continuing",
            1 => "ended",
            2 => "upcoming",
            3 => "deleted",
            _ => "unknown",
        };
        let series_type = match s.series_type {
            1 => "anime",
            2 => "daily",
            _ => "standard",
        };
        let monitor_new_items = match s.monitor_new_items {
            1 => "all",
            2 => "none",
            _ => "all",
        };

        // Construct default images with cache-busting timestamp.
        // last_info_sync changes on every metadata refresh/rematch, so the browser
        // will fetch fresh images instead of serving stale cached ones.
        let cache_bust = s
            .last_info_sync
            .map(|ts| format!("?t={}", ts.timestamp()))
            .unwrap_or_default();
        let images = vec![
            SeriesImage {
                cover_type: "poster".to_string(),
                url: format!("/MediaCover/Series/{}/poster.jpg{}", s.id, cache_bust),
            },
            SeriesImage {
                cover_type: "fanart".to_string(),
                url: format!("/MediaCover/Series/{}/fanart.jpg{}", s.id, cache_bust),
            },
            SeriesImage {
                cover_type: "banner".to_string(),
                url: format!("/MediaCover/Series/{}/banner.jpg{}", s.id, cache_bust),
            },
        ];

        let ended = s.status == 1;

        // Extract folder name from path
        let folder = s.path.split('/').next_back().map(|f| f.to_string());

        Self {
            id: s.id,
            title: s.title,
            alternate_titles: vec![],
            sort_title: s.sort_title,
            status: status.to_string(),
            ended,
            profile_name: None,
            overview: s.overview,
            next_airing: None,
            previous_airing: None,
            network: s.network,
            air_time: None,
            images,
            original_language: Some(LanguageResource {
                id: 1,
                name: "English".to_string(),
            }),
            remote_poster: None,
            seasons: vec![], // TODO: populate from database
            year: s.year,
            path: s.path,
            quality_profile_id: s.quality_profile_id,
            season_folder: s.season_folder,
            monitored: s.monitored,
            monitor_new_items: monitor_new_items.to_string(),
            use_scene_numbering: s.use_scene_numbering,
            runtime: s.runtime,
            tvdb_id: s.tvdb_id,
            tv_rage_id: s.tv_rage_id,
            tv_maze_id: s.tv_maze_id,
            tmdb_id: s.tmdb_id,
            first_aired: s.first_aired.map(|d| d.to_string()),
            last_aired: s.last_aired.map(|d| d.to_string()),
            series_type: series_type.to_string(),
            clean_title: s.clean_title,
            imdb_id: s.imdb_id,
            title_slug: s.title_slug,
            root_folder_path: s.root_folder_path,
            folder,
            certification: s.certification,
            genres: vec![], // TODO: populate from database
            tags: vec![],   // TODO: populate from database
            added: s.added.to_rfc3339(),
            add_options: None,
            ratings: Ratings {
                votes: s.imdb_votes.map(|v| v as i64).unwrap_or(0),
                value: s.imdb_rating.map(|r| r as f64).unwrap_or(0.0),
            },
            imdb_rating: s.imdb_rating,
            imdb_votes: s.imdb_votes,
            statistics: Some(SeriesStatistics::default()),
            episodes_changed: None,
            language_profile_id: s.language_profile_id,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesLookupResult {
    pub tvdb_id: i64,
    pub title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: i32,
    pub status: String,
    pub images: Vec<SeriesImage>,
    pub seasons: Vec<SeasonResource>,
    pub ratings: Ratings,
    pub genres: Vec<String>,
    pub network: Option<String>,
    pub runtime: i32,
    pub tvrage_id: i64,
    pub imdb_id: Option<String>,
    pub certification: Option<String>,
    pub first_aired: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SeriesImage {
    pub cover_type: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SeasonResource {
    pub season_number: i32,
    pub monitored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<SeasonStatistics>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SeasonStatistics {
    pub episode_file_count: i32,
    pub episode_count: i32,
    pub total_episode_count: i32,
    pub percent_of_episodes: f64,
    pub size_on_disk: i64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ratings {
    pub votes: i64,
    pub value: f64,
}

// Error types

#[derive(Debug)]
pub enum ApiError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}
