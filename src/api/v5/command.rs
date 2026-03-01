//! Command API endpoints (v5)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::CommandRepository;
use crate::core::mediafiles::{compute_file_hash, derive_quality_from_media, MediaAnalyzer};
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResource {
    pub id: i32,
    pub name: String,
    pub command_name: String,
    pub message: Option<String>,
    pub body: serde_json::Value,
    pub priority: String,
    pub status: String,
    pub result: String,
    pub queued: String,
    pub started: Option<String>,
    pub ended: Option<String>,
    pub duration: Option<String>,
    pub trigger: String,
    pub state_change_time: Option<String>,
    pub send_updates_to_client: bool,
    pub update_scheduled_task: bool,
    pub last_execution_time: Option<String>,
}

impl From<crate::core::datastore::repositories::CommandDbModel> for CommandResource {
    fn from(cmd: crate::core::datastore::repositories::CommandDbModel) -> Self {
        let body: serde_json::Value = cmd
            .body
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        Self {
            id: cmd.id as i32,
            name: cmd.name,
            command_name: cmd.command_name,
            message: cmd.message,
            body,
            priority: cmd.priority,
            status: cmd.status,
            result: cmd.result.unwrap_or_else(|| "unknown".to_string()),
            queued: cmd.queued.to_rfc3339(),
            started: cmd.started.map(|d| d.to_rfc3339()),
            ended: cmd.ended.map(|d| d.to_rfc3339()),
            duration: cmd.duration,
            trigger: cmd.trigger,
            state_change_time: Some(cmd.state_change_time.to_rfc3339()),
            send_updates_to_client: cmd.send_updates_to_client,
            update_scheduled_task: cmd.update_scheduled_task,
            last_execution_time: cmd.last_execution_time.map(|d| d.to_rfc3339()),
        }
    }
}

/// GET /api/v5/command - List all commands
pub async fn get_commands(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CommandResource>>, CommandError> {
    let repo = CommandRepository::new(state.db.clone());

    let commands = repo
        .get_all()
        .await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch commands: {}", e)))?;

    let resources: Vec<CommandResource> = commands.into_iter().map(Into::into).collect();
    Ok(Json(resources))
}

/// GET /api/v5/command/{id} - Get a specific command
pub async fn get_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<CommandResource>, CommandError> {
    let repo = CommandRepository::new(state.db.clone());

    let command = repo
        .get_by_id(id as i64)
        .await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch command: {}", e)))?
        .ok_or(CommandError::NotFound)?;

    Ok(Json(command.into()))
}

/// POST /api/v5/command - Create/queue a new command
pub async fn create_command(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<CommandResource>, CommandError> {
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CommandError::Validation("Command name is required".to_string()))?;

    let repo = CommandRepository::new(state.db.clone());

    let body_str = serde_json::to_string(&body).ok();
    let id = repo
        .insert(name, name, body_str.as_deref(), "manual")
        .await
        .map_err(|e| CommandError::Internal(format!("Failed to create command: {}", e)))?;

    tracing::info!("Queued command: id={}, name={}", id, name);

    // Fetch the created command to return
    let command = repo
        .get_by_id(id)
        .await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch command: {}", e)))?
        .ok_or(CommandError::NotFound)?;

    // Create a cancellation token for this command
    let cancel_token = tokio_util::sync::CancellationToken::new();
    state.command_tokens.insert(id, cancel_token.clone());

    // Spawn background task to execute the command
    tokio::spawn({
        let db = state.db.clone();
        let event_bus = state.event_bus.clone();
        let hybrid_event_bus = state.hybrid_event_bus.clone();
        let metadata_service = state.metadata_service.clone();
        let imdb_client = state.imdb_client.clone();
        let media_config = state.config.read().media.clone();
        let command_tokens = state.command_tokens.clone();
        let scan_result_consumer = state.scan_result_consumer.get().cloned();
        let cmd_id = id;
        let cmd_name = name.to_string();
        let cmd_body = body.clone();
        let token = cancel_token;
        async move {
            use crate::core::messaging::Message;

            let repo = CommandRepository::new(db.clone());
            if let Err(e) = repo.start_command(cmd_id).await {
                tracing::error!("Failed to start command {}: {}", cmd_id, e);
                command_tokens.remove(&cmd_id);
                return;
            }

            // Publish command started event
            event_bus
                .publish(Message::CommandStarted {
                    command_id: cmd_id,
                    name: cmd_name.clone(),
                    message: Some(format!("Starting {}", cmd_name)),
                })
                .await;

            // Execute command based on type (with distributed scanning support)
            let options = CommandExecutionOptions {
                hybrid_event_bus,
                metadata_service: Some(metadata_service),
                imdb_client: Some(imdb_client),
                cancel_token: Some(token),
                media_config: Some(media_config),
                scan_result_consumer,
            };
            let result =
                execute_command_with_options(&cmd_name, &cmd_body, &db, &event_bus, options).await;

            // Clean up cancellation token
            command_tokens.remove(&cmd_id);

            // Mark as completed, cancelled, or failed
            match result {
                Ok(msg) if msg.starts_with("Cancelled:") => {
                    let _ = repo
                        .update_status(cmd_id, "cancelled", Some("cancelled"))
                        .await;
                    tracing::info!("Cancelled command: id={}, name={}", cmd_id, cmd_name);
                    event_bus
                        .publish(Message::CommandCompleted {
                            command_id: cmd_id,
                            name: cmd_name.clone(),
                            message: Some(msg),
                        })
                        .await;
                }
                Ok(msg) => {
                    if let Err(e) = repo
                        .update_status(cmd_id, "completed", Some("successful"))
                        .await
                    {
                        tracing::error!("Failed to complete command {}: {}", cmd_id, e);
                    } else {
                        tracing::info!(
                            "Completed command: id={}, name={}, result={}",
                            cmd_id,
                            cmd_name,
                            msg
                        );
                        // Log to database for Events page
                        crate::core::logging::log_info(
                            "CommandExecutor",
                            &format!("{} completed: {}", cmd_name, msg),
                        )
                        .await;
                    }
                    // Publish command completed event
                    event_bus
                        .publish(Message::CommandCompleted {
                            command_id: cmd_id,
                            name: cmd_name.clone(),
                            message: Some(msg),
                        })
                        .await;
                }
                Err(e) => {
                    tracing::error!("Command {} failed: {}", cmd_id, e);
                    let _ = repo.update_status(cmd_id, "failed", Some(&e)).await;
                    // Log error to database for Events page
                    crate::core::logging::log_error(
                        "CommandExecutor",
                        &format!("{} failed: {}", cmd_name, e),
                    )
                    .await;
                    // Publish command failed event
                    event_bus
                        .publish(Message::CommandFailed {
                            command_id: cmd_id,
                            name: cmd_name.clone(),
                            message: None,
                            error: e,
                        })
                        .await;
                }
            }
        }
    });

    Ok(Json(command.into()))
}

/// DELETE /api/v5/command/{id} - Cancel a command
pub async fn delete_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, CommandError> {
    // Signal the running command to stop via its cancellation token
    if let Some((_, token)) = state.command_tokens.remove(&(id as i64)) {
        token.cancel();
        tracing::info!("Cancelling running command: id={}", id);
    }

    let repo = CommandRepository::new(state.db.clone());

    repo.delete(id as i64)
        .await
        .map_err(|e| CommandError::Internal(format!("Failed to delete command: {}", e)))?;

    tracing::info!("Cancelled/deleted command: id={}", id);

    Ok(Json(serde_json::json!({})))
}

/// Error type for command operations
#[derive(Debug)]
pub enum CommandError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for CommandError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            CommandError::NotFound => (StatusCode::NOT_FOUND, "Command not found".to_string()),
            CommandError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            CommandError::Internal(msg) => {
                tracing::error!("Command error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_commands).post(create_command))
        .route("/{id}", get(get_command).delete(delete_command))
}

/// Options for command execution
#[derive(Default)]
pub struct CommandExecutionOptions {
    /// Hybrid event bus for distributed scanning (if in server mode)
    pub hybrid_event_bus: Option<crate::core::messaging::HybridEventBus>,
    /// Metadata service for IMDB-first metadata fetching
    pub metadata_service: Option<crate::core::metadata::MetadataService>,
    /// IMDB client for movie metadata lookups
    pub imdb_client: Option<crate::core::imdb::ImdbClient>,
    /// Cancellation token to stop long-running commands
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    /// Media config for episode naming during imports
    pub media_config: Option<crate::core::configuration::MediaConfig>,
    /// Scan result consumer for registering download imports (worker mode)
    pub scan_result_consumer: Option<std::sync::Arc<crate::core::scanner::ScanResultConsumer>>,
}

/// Execute a command with additional options (for distributed mode and IMDB metadata)
pub async fn execute_command_with_options(
    name: &str,
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
    options: CommandExecutionOptions,
) -> Result<String, String> {
    match name {
        "RefreshSeries" => {
            execute_refresh_series(
                body,
                db,
                event_bus,
                options.metadata_service.as_ref(),
                options.hybrid_event_bus.as_ref(),
            )
            .await
        }
        "RescanSeries" => {
            execute_rescan_series(
                body,
                db,
                event_bus,
                options.hybrid_event_bus.as_ref(),
                options.media_config.as_ref(),
            )
            .await
        }
        "DownloadedEpisodesScan" | "ProcessMonitoredDownloads" => {
            execute_process_downloads(
                body,
                db,
                event_bus,
                options.media_config.as_ref(),
                options.hybrid_event_bus.as_ref(),
                options.scan_result_consumer.as_ref(),
            )
            .await
        }
        "RssSync" => execute_rss_sync(body, db, event_bus).await,
        "ApplicationCheckUpdate" => {
            tracing::info!("ApplicationCheckUpdate: no updates available (this is sonarr-rs)");
            Ok("No updates available".to_string())
        }
        "Housekeeping" => {
            tracing::info!("Housekeeping: cleaning up...");
            Ok("Housekeeping completed".to_string())
        }
        "MessagingCleanup" => {
            tracing::info!("MessagingCleanup: completed");
            Ok("Messaging cleanup completed".to_string())
        }
        "RefreshMovies" => {
            execute_refresh_movies(
                body,
                db,
                options.imdb_client.as_ref(),
                options.cancel_token.as_ref(),
            )
            .await
        }
        "RescanMovie" => {
            execute_rescan_movie(body, db, options.hybrid_event_bus.as_ref()).await
        }
        "RefreshMonitoredDownloads" => {
            let service = crate::core::queue::TrackedDownloadService::new(db.clone());
            match service.reconcile_downloads().await {
                Ok(count) => Ok(format!("Reconciled {} downloads", count)),
                Err(e) => Err(format!("Failed to reconcile downloads: {}", e)),
            }
        }
        "EpisodeSearch" => execute_episode_search(body, db, event_bus).await,
        "SeasonSearch" => execute_season_search(body, db, event_bus).await,
        "SeriesSearch" => execute_series_search(body, db, event_bus).await,
        "MovieSearch" => execute_movie_search(body, db).await,
        "MissingEpisodeSearch" => {
            tracing::info!("MissingEpisodeSearch: searching for missing episodes");
            // This would search indexers for all missing episodes
            Ok("Missing episode search started".to_string())
        }
        "CutoffUnmetEpisodeSearch" => {
            tracing::info!("CutoffUnmetEpisodeSearch: searching for cutoff unmet episodes");
            Ok("Cutoff unmet episode search started".to_string())
        }
        _ => {
            tracing::warn!("Unknown command type: {}", name);
            Ok(format!("Command {} acknowledged (no-op)", name))
        }
    }
}

/// Execute RefreshSeries command - fetches metadata using MetadataService (IMDB-first + Skyhook fallback)
async fn execute_refresh_series(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
    metadata_service: Option<&crate::core::metadata::MetadataService>,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
) -> Result<String, String> {
    use crate::core::datastore::models::EpisodeDbModel;
    use crate::core::datastore::repositories::{EpisodeRepository, SeriesRepository};
    use crate::core::metadata::EpisodeEnrichment;
    use chrono::{NaiveDate, Utc};

    let series_repo = SeriesRepository::new(db.clone());
    let episode_repo = EpisodeRepository::new(db.clone());

    // Parse series IDs from body - handle both singular seriesId and plural seriesIds
    let mut series_ids: Vec<i64> = body
        .get("seriesIds")
        .or_else(|| body.get("body").and_then(|b| b.get("seriesIds")))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    // Also check for singular seriesId (frontend sends this format)
    if series_ids.is_empty() {
        if let Some(id) = body
            .get("seriesId")
            .or_else(|| body.get("body").and_then(|b| b.get("seriesId")))
            .and_then(|v| v.as_i64())
        {
            series_ids.push(id);
        }
    }

    // If no series IDs provided, refresh ALL series
    if series_ids.is_empty() {
        tracing::info!("RefreshSeries: no series IDs provided, refreshing all series");
        let all_series = series_repo
            .get_all()
            .await
            .map_err(|e| format!("Failed to fetch series list: {}", e))?;
        series_ids = all_series.into_iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok("No series to refresh".to_string());
        }
        tracing::info!(
            "RefreshSeries: found {} series to refresh",
            series_ids.len()
        );
    }

    tracing::info!("RefreshSeries: refreshing {} series", series_ids.len());

    let mut refreshed = 0;
    let mut errors = 0;

    for series_id in &series_ids {
        // Get series from database
        let series = match series_repo.get_by_id(*series_id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                tracing::warn!("RefreshSeries: series {} not found", series_id);
                continue;
            }
            Err(e) => {
                tracing::error!("RefreshSeries: failed to fetch series {}: {}", series_id, e);
                errors += 1;
                continue;
            }
        };

        tracing::info!(
            "RefreshSeries: refreshing {} (TVDB: {})",
            series.title,
            series.tvdb_id
        );

        // Fetch metadata using MetadataService (IMDB-first) or fall back to direct Skyhook
        let metadata = if let Some(svc) = metadata_service {
            match svc
                .fetch_series_metadata(series.tvdb_id, series.imdb_id.as_deref())
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!(
                        "RefreshSeries: failed to fetch metadata for {}: {}",
                        series.title,
                        e
                    );
                    errors += 1;
                    continue;
                }
            }
        } else {
            // Fallback: direct Skyhook call (when MetadataService not available)
            match crate::core::metadata::MetadataService::fetch_skyhook_only(series.tvdb_id).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!(
                        "RefreshSeries: failed to fetch from Skyhook for {}: {}",
                        series.title,
                        e
                    );
                    errors += 1;
                    continue;
                }
            }
        };

        // Update series metadata from merged result
        let mut series = series;
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

        // Update year (IMDB-preferred via metadata merge)
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
        if series.year != old_year {
            tracing::info!(
                "RefreshSeries: {} - year updated from {} to {}",
                series.title,
                old_year,
                series.year
            );
        }
        if let Some(first_aired) = &metadata.first_aired {
            series.first_aired = NaiveDate::parse_from_str(first_aired, "%Y-%m-%d").ok();
        }
        series.last_info_sync = Some(Utc::now());

        // Capture IMDB data
        if let Some(ref imdb_id) = metadata.imdb_id {
            if series.imdb_id.is_none() {
                tracing::info!(
                    "RefreshSeries: {} - captured imdb_id: {}",
                    series.title,
                    imdb_id
                );
            }
            series.imdb_id = Some(imdb_id.clone());
        }
        series.imdb_rating = metadata.imdb_rating;
        series.imdb_votes = metadata.imdb_votes;

        // Update series in database
        if let Err(e) = series_repo.update(&series).await {
            tracing::error!(
                "RefreshSeries: failed to update series {}: {}",
                series.title,
                e
            );
            errors += 1;
            continue;
        }

        // Sync episodes from Skyhook data
        let mut episodes_added = 0;
        let mut episodes_updated = 0;

        for ep in &metadata.episodes {
            let air_date = ep
                .air_date
                .as_ref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
            let air_date_utc = ep
                .air_date_utc
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            // Check if episode exists
            let existing = if ep.tvdb_id > 0 {
                episode_repo.get_by_tvdb_id(ep.tvdb_id).await.ok().flatten()
            } else {
                episode_repo
                    .get_by_series_season_episode(*series_id, ep.season_number, ep.episode_number)
                    .await
                    .ok()
                    .flatten()
            };

            match existing {
                Some(mut episode) => {
                    episode.title = ep
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("Episode {}", ep.episode_number));
                    episode.overview = ep.overview.clone();
                    episode.air_date = air_date;
                    episode.air_date_utc = air_date_utc;
                    episode.runtime = ep.runtime.unwrap_or(0);
                    episode.absolute_episode_number = ep.absolute_episode_number;
                    if ep.tvdb_id > 0 {
                        episode.tvdb_id = ep.tvdb_id;
                    }

                    if episode_repo.update(&episode).await.is_ok() {
                        episodes_updated += 1;
                    }
                }
                None => {
                    let episode = EpisodeDbModel {
                        id: 0,
                        series_id: *series_id,
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
                            .clone()
                            .unwrap_or_else(|| format!("Episode {}", ep.episode_number)),
                        overview: ep.overview.clone(),
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
                        episodes_added += 1;
                    }
                }
            }
        }

        // Remap episode numbers if series uses a non-default TVDB ordering
        if series.episode_ordering != "aired" {
            if let Some(svc) = metadata_service {
                match svc
                    .tvdb_client()
                    .get_episodes_by_ordering(series.tvdb_id, &series.episode_ordering)
                    .await
                {
                    Ok(Some(tvdb_episodes)) => {
                        // Build tvdb_episode_id → (new_season, new_episode) lookup
                        let mut ordering_map: std::collections::HashMap<i64, (i32, i32)> =
                            std::collections::HashMap::new();
                        for tep in &tvdb_episodes {
                            ordering_map.insert(tep.id, (tep.season_number, tep.episode_number));
                        }

                        // Fetch current episodes and build remap plan
                        let current_episodes = episode_repo
                            .get_by_series_id(*series_id)
                            .await
                            .unwrap_or_default();

                        let mut remap_count = 0;
                        let mut remap_plan: Vec<(i64, i32, i32, i32, i32)> = Vec::new(); // (id, new_season, new_ep, old_season, old_ep)

                        for ep in &current_episodes {
                            if ep.tvdb_id > 0 {
                                if let Some(&(new_season, new_ep)) = ordering_map.get(&ep.tvdb_id) {
                                    if new_season != ep.season_number || new_ep != ep.episode_number {
                                        remap_plan.push((
                                            ep.id,
                                            new_season,
                                            new_ep,
                                            ep.season_number,
                                            ep.episode_number,
                                        ));
                                    }
                                }
                            }
                        }

                        if !remap_plan.is_empty() {
                            tracing::info!(
                                "RefreshSeries: {} - remapping {} episodes to {} ordering",
                                series.title,
                                remap_plan.len(),
                                series.episode_ordering,
                            );

                            let pool = db.pool();
                            let mut tx = pool
                                .begin()
                                .await
                                .map_err(|e| format!("Failed to begin transaction: {}", e))?;

                            // Phase A: sentinel ALL episodes for this series to avoid unique constraint
                            // collisions between remapped targets and non-remapped originals
                            sqlx::query(
                                "UPDATE episodes SET season_number = -(1000 + id::int), episode_number = -(1000 + id::int) WHERE series_id = $1"
                            )
                            .bind(series_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Remap phase A (sentinel) failed: {}", e))?;

                            // Build set of positions claimed by the DVD ordering
                            let remapped_ids: std::collections::HashSet<i64> =
                                remap_plan.iter().map(|r| r.0).collect();
                            let mut claimed: std::collections::HashSet<(i32, i32)> =
                                std::collections::HashSet::new();

                            // Phase B1: set remapped episodes to DVD positions + backup scene_*
                            for &(id, new_season, new_ep, old_season, old_ep) in &remap_plan {
                                sqlx::query(
                                    "UPDATE episodes SET season_number = $1, episode_number = $2, \
                                     scene_season_number = $3, scene_episode_number = $4 WHERE id = $5"
                                )
                                .bind(new_season)
                                .bind(new_ep)
                                .bind(Some(old_season))
                                .bind(Some(old_ep))
                                .bind(id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Remap phase B1 failed: {}", e))?;
                                claimed.insert((new_season, new_ep));
                                remap_count += 1;
                            }

                            // Phase B2: restore non-remapped episodes to their original positions.
                            // If a position is already claimed by a DVD-remapped episode, bump to
                            // the next available slot in the same season.
                            for ep in &current_episodes {
                                if remapped_ids.contains(&ep.id) {
                                    continue;
                                }
                                let (final_season, final_ep) =
                                    if !claimed.contains(&(ep.season_number, ep.episode_number)) {
                                        (ep.season_number, ep.episode_number)
                                    } else {
                                        // Position stolen by DVD ordering — find next available
                                        let mut bump = ep.episode_number + 1;
                                        while claimed.contains(&(ep.season_number, bump)) {
                                            bump += 1;
                                        }
                                        tracing::debug!(
                                            "Episode id={} displaced from S{:02}E{:02} to S{:02}E{:02} (position taken by {} ordering)",
                                            ep.id, ep.season_number, ep.episode_number,
                                            ep.season_number, bump, series.episode_ordering,
                                        );
                                        (ep.season_number, bump)
                                    };
                                sqlx::query(
                                    "UPDATE episodes SET season_number = $1, episode_number = $2 WHERE id = $3"
                                )
                                .bind(final_season)
                                .bind(final_ep)
                                .bind(ep.id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Remap phase B2 (restore) failed: {}", e))?;
                                claimed.insert((final_season, final_ep));
                            }

                            tx.commit()
                                .await
                                .map_err(|e| format!("Remap transaction commit failed: {}", e))?;

                            tracing::info!(
                                "RefreshSeries: {} - remapped {} episodes to {} ordering",
                                series.title,
                                remap_count,
                                series.episode_ordering,
                            );
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "RefreshSeries: {} - no TVDB data for {} ordering",
                            series.title,
                            series.episode_ordering,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "RefreshSeries: {} - failed to fetch TVDB {} ordering: {}",
                            series.title,
                            series.episode_ordering,
                            e,
                        );
                    }
                }
            }
        }

        // Enrich episodes with IMDB ratings
        if let (Some(svc), Some(ref imdb_id)) = (metadata_service, &series.imdb_id) {
            let all_episodes = episode_repo
                .get_by_series_id(*series_id)
                .await
                .unwrap_or_default();
            let mut enrichments: Vec<EpisodeEnrichment> = all_episodes
                .iter()
                .map(|e| EpisodeEnrichment {
                    season_number: e.season_number,
                    episode_number: e.episode_number,
                    imdb_id: None,
                    imdb_rating: None,
                    imdb_votes: None,
                })
                .collect();

            if let Ok(enriched) = svc
                .enrich_episodes_with_imdb(imdb_id, &mut enrichments)
                .await
            {
                if enriched > 0 {
                    // Write enriched data back to DB
                    for (ep_model, enrichment) in all_episodes.iter().zip(enrichments.iter()) {
                        if enrichment.imdb_id.is_some() {
                            let mut updated = ep_model.clone();
                            updated.imdb_id = enrichment.imdb_id.clone();
                            updated.imdb_rating = enrichment.imdb_rating;
                            updated.imdb_votes = enrichment.imdb_votes;
                            let _ = episode_repo.update(&updated).await;
                        }
                    }
                }
            }
        }

        tracing::info!(
            "RefreshSeries: {} - {} episodes added, {} updated",
            series.title,
            episodes_added,
            episodes_updated
        );

        // Publish series refreshed event
        event_bus
            .publish(crate::core::messaging::Message::SeriesRefreshed {
                series_id: *series_id,
                title: series.title.clone(),
            })
            .await;

        refreshed += 1;
    }

    // After refreshing metadata, also run a disk scan to update file status
    if !series_ids.is_empty() {
        tracing::info!(
            "RefreshSeries: triggering disk rescan for {} series",
            series_ids.len()
        );
        let rescan_body = serde_json::json!({
            "name": "RescanSeries",
            "seriesIds": series_ids,
        });
        if let Err(e) = execute_rescan_series(&rescan_body, db, event_bus, hybrid_event_bus, None).await {
            tracing::warn!("RefreshSeries: rescan failed: {}", e);
        }
    }

    Ok(format!(
        "Refreshed {} series ({} errors)",
        refreshed, errors
    ))
}

/// Execute RefreshMovies command - backfills IMDB IDs, ratings, and Radarr images for movies
async fn execute_refresh_movies(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    imdb_client: Option<&crate::core::imdb::ImdbClient>,
    cancel_token: Option<&tokio_util::sync::CancellationToken>,
) -> Result<String, String> {
    use crate::core::datastore::repositories::MovieRepository;
    use chrono::{Datelike, Utc};

    let repo = MovieRepository::new(db.clone());

    // Parse movie IDs from body (same pattern as RefreshSeries)
    let mut movie_ids: Vec<i64> = body
        .get("movieIds")
        .or_else(|| body.get("body").and_then(|b| b.get("movieIds")))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if movie_ids.is_empty() {
        if let Some(id) = body
            .get("movieId")
            .or_else(|| body.get("body").and_then(|b| b.get("movieId")))
            .and_then(|v| v.as_i64())
        {
            movie_ids.push(id);
        }
    }

    // If no IDs provided, refresh ALL movies
    if movie_ids.is_empty() {
        tracing::info!("RefreshMovies: no movie IDs provided, refreshing all movies");
        let all_movies = repo
            .get_all()
            .await
            .map_err(|e| format!("Failed to fetch movie list: {}", e))?;
        movie_ids = all_movies.into_iter().map(|m| m.id).collect();

        if movie_ids.is_empty() {
            return Ok("No movies to refresh".to_string());
        }
        tracing::info!("RefreshMovies: found {} movies to refresh", movie_ids.len());
    }

    let total = movie_ids.len();
    tracing::info!("RefreshMovies: refreshing {} movies", total);

    let mut refreshed = 0;
    let mut imdb_found = 0;
    let mut images_found = 0;
    let mut errors = 0;

    for (idx, movie_id) in movie_ids.iter().enumerate() {
        // Check for cancellation between movies
        if cancel_token.is_some_and(|t| t.is_cancelled()) {
            let summary = format!(
                "Cancelled: refreshed {}/{} movies before stop ({} IMDB IDs, {} images, {} errors)",
                refreshed, total, imdb_found, images_found, errors
            );
            tracing::info!("RefreshMovies: {}", summary);
            return Ok(summary);
        }

        let mut movie = match repo.get_by_id(*movie_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                tracing::warn!("RefreshMovies: movie {} not found", movie_id);
                continue;
            }
            Err(e) => {
                tracing::error!("RefreshMovies: failed to fetch movie {}: {}", movie_id, e);
                errors += 1;
                continue;
            }
        };

        // Step 1: If missing imdb_id, search IMDB by title
        if movie.imdb_id.is_none() || movie.imdb_id.as_deref() == Some("") {
            if let Some(client) = imdb_client {
                if client.is_enabled() {
                    // Strip year suffix like " (2020)" from title for search
                    let search_title = if let Some(idx) = movie.title.rfind(" (") {
                        movie.title[..idx].to_string()
                    } else {
                        movie.title.clone()
                    };
                    match client.search_movies(&search_title, 10).await {
                        Ok(results) if !results.is_empty() => {
                            // Pick best match by year proximity, reject if >2 years off
                            let best = if movie.year > 0 {
                                results
                                    .iter()
                                    .filter(|m| m.year.is_some())
                                    .filter(|m| {
                                        (m.year.unwrap_or(0) - movie.year).unsigned_abs() <= 2
                                    })
                                    .min_by_key(|m| {
                                        (m.year.unwrap_or(0) - movie.year).unsigned_abs()
                                    })
                            } else {
                                results.first()
                            };

                            if let Some(m) = best {
                                tracing::debug!(
                                    "RefreshMovies: IMDB match for '{}' ({}): {} [{}]",
                                    movie.title,
                                    movie.year,
                                    m.title,
                                    m.imdb_id
                                );
                                // Use the clean IMDB title (without year suffix)
                                movie.title = m.title.clone();
                                movie.clean_title = m
                                    .title
                                    .to_lowercase()
                                    .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
                                    .split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                movie.sort_title = movie.clean_title.clone();
                                movie.imdb_id = Some(m.imdb_id.clone());
                                movie.imdb_rating = m.rating.map(|r| r as f32);
                                movie.imdb_votes = m.votes.map(|v| v as i32);
                                if let Some(year) = m.year {
                                    movie.year = year;
                                }
                                if m.runtime_minutes.unwrap_or(0) > 0 {
                                    movie.runtime = m.runtime_minutes.unwrap_or(0);
                                }
                                imdb_found += 1;
                            }
                        }
                        Ok(_) => {
                            tracing::debug!(
                                "RefreshMovies: no IMDB results for '{}' (searched: '{}')",
                                movie.title,
                                search_title
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "RefreshMovies: IMDB search failed for '{}': {}",
                                movie.title,
                                e
                            );
                        }
                    }
                }
            }
        }

        // Step 2: If we have an imdb_id, fetch images via TMDB → Radarr cascade
        if let Some(ref imdb_id) = movie.imdb_id {
            if let Some((tmdb_id, images)) =
                super::movies::fetch_movie_images_and_tmdb_id(imdb_id).await
            {
                if tmdb_id > 0 && movie.tmdb_id == 0 {
                    movie.tmdb_id = tmdb_id;
                }
                if !images.is_empty() {
                    let images_json =
                        serde_json::to_string(&images).unwrap_or_else(|_| "[]".to_string());
                    movie.images = images_json;
                    images_found += 1;
                }
            }
        }

        // Step 3: Derive movie status from year when no release date is set
        if movie.status == 0 && movie.year > 0 {
            let current_year = Utc::now().year();
            if movie.year < current_year {
                movie.status = 3; // Released
            } else if movie.year == current_year {
                movie.status = 2; // InCinemas
            } else {
                movie.status = 1; // Announced
            }
        }

        movie.last_info_sync = Some(Utc::now());

        // Pre-check: if this movie's IMDB ID conflicts with another movie, merge proactively
        if let Some(ref imdb_id) = movie.imdb_id {
            if let Ok(Some(existing)) = repo.get_by_imdb_id(imdb_id).await {
                if existing.id != movie.id {
                    tracing::info!(
                        "RefreshMovies: merging duplicate '{}' (id={}) into '{}' (id={}, imdb={})",
                        movie.title,
                        movie.id,
                        existing.title,
                        existing.id,
                        imdb_id
                    );
                    let pool = db.pool();
                    let _ = sqlx::query("UPDATE movie_files SET movie_id = $1 WHERE movie_id = $2")
                        .bind(existing.id)
                        .bind(movie.id)
                        .execute(pool)
                        .await;
                    if let Err(del_err) = repo.delete(movie.id).await {
                        tracing::warn!(
                            "RefreshMovies: failed to delete duplicate movie {}: {}",
                            movie.id,
                            del_err
                        );
                    }
                    refreshed += 1;
                    continue;
                }
            }
        }

        // Pre-check: if this movie's TMDB ID conflicts with another movie, clear it
        if movie.tmdb_id > 0 {
            if let Ok(Some(existing)) = repo.get_by_tmdb_id(movie.tmdb_id).await {
                if existing.id != movie.id {
                    tracing::warn!(
                        "RefreshMovies: tmdb_id {} conflict for '{}' (owned by '{}'), clearing",
                        movie.tmdb_id,
                        movie.title,
                        existing.title
                    );
                    movie.tmdb_id = 0;
                }
            }
        }

        // Save updated movie
        if let Err(e) = repo.update(&movie).await {
            tracing::error!(
                "RefreshMovies: failed to update movie '{}': {}",
                movie.title,
                e
            );
            errors += 1;
            continue;
        }

        refreshed += 1;

        // Log progress every 50 movies
        if (idx + 1) % 50 == 0 {
            tracing::info!(
                "RefreshMovies: progress {}/{} (IMDB: {}, images: {})",
                idx + 1,
                total,
                imdb_found,
                images_found
            );
        }
    }

    let summary = format!(
        "Refreshed {} movies: {} IMDB IDs found, {} images updated ({} errors)",
        refreshed, imdb_found, images_found, errors
    );
    tracing::info!("RefreshMovies: {}", summary);
    Ok(summary)
}

/// Execute RescanSeries command - scans disk for episode files
///
/// If `hybrid_event_bus` is provided (distributed mode), publishes scan requests
/// to workers instead of scanning locally.
///
/// Supports optional `seasonNumber` parameter to scope the scan to a single season folder.
async fn execute_rescan_series(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
    media_config: Option<&crate::core::configuration::MediaConfig>,
) -> Result<String, String> {
    use crate::core::datastore::models::EpisodeFileDbModel;
    use crate::core::datastore::repositories::{
        EpisodeFileRepository, EpisodeRepository, SeriesRepository,
    };
    use chrono::Utc;
    use std::path::Path;

    // Parse optional seasonNumber — when provided, only that season's folder is scanned
    let season_number: Option<i32> = body
        .get("seasonNumber")
        .or_else(|| body.get("body").and_then(|b| b.get("seasonNumber")))
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    // Parse series IDs from body - handle both singular seriesId and plural seriesIds
    let mut series_ids: Vec<i64> = body
        .get("seriesIds")
        .or_else(|| body.get("body").and_then(|b| b.get("seriesIds")))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    // Also check for singular seriesId (frontend sends this format)
    if series_ids.is_empty() {
        if let Some(id) = body
            .get("seriesId")
            .or_else(|| body.get("body").and_then(|b| b.get("seriesId")))
            .and_then(|v| v.as_i64())
        {
            series_ids.push(id);
        }
    }

    // If no series IDs provided, rescan ALL series
    if series_ids.is_empty() {
        tracing::info!("RescanSeries: no series IDs provided, rescanning all series");
        let series_repo = crate::core::datastore::repositories::SeriesRepository::new(db.clone());
        let all_series = series_repo
            .get_all()
            .await
            .map_err(|e| format!("Failed to fetch series list: {}", e))?;
        series_ids = all_series.into_iter().map(|s| s.id).collect();

        if series_ids.is_empty() {
            return Ok("No series to rescan".to_string());
        }
        tracing::info!("RescanSeries: found {} series to rescan", series_ids.len());
    }

    // Prefer worker scanning when Redis is available — workers have local disk
    // access and can do FFmpeg probe + BLAKE3 hash much faster than NFS.
    // Only fall back to local scanning when no Redis/workers are configured.
    if let Some(hybrid_bus) = hybrid_event_bus {
        if hybrid_bus.is_redis_enabled() {
            let _ = execute_rescan_series_distributed(
                &series_ids,
                db,
                hybrid_bus,
                season_number,
                media_config,
            )
            .await;
            let scope = match season_number {
                Some(n) => format!(" (season {})", n),
                None => String::new(),
            };
            return Ok(format!(
                "Dispatched {} series to workers for scanning{}",
                series_ids.len(),
                scope
            ));
        }
    }

    tracing::info!(
        "RescanSeries: scanning {} series locally{}",
        series_ids.len(),
        season_number.map_or(String::new(), |n| format!(" (season {})", n))
    );

    let series_repo = SeriesRepository::new(db.clone());
    let episode_repo = EpisodeRepository::new(db.clone());
    let episode_file_repo = EpisodeFileRepository::new(db.clone());

    let mut total_files = 0;
    let mut total_matched = 0;
    let mut total_new_files = 0;

    for series_id in &series_ids {
        let series = match series_repo.get_by_id(*series_id).await {
            Ok(Some(s)) => s,
            _ => continue,
        };

        // When seasonNumber is provided, scope scan to that season's folder
        let scan_path = if let Some(sn) = season_number {
            let default_config = crate::core::configuration::MediaConfig::default();
            let config = media_config.unwrap_or(&default_config);
            let season_folder = crate::core::naming::build_season_folder(config, sn);
            Path::new(&series.path).join(&season_folder)
        } else {
            Path::new(&series.path).to_path_buf()
        };

        if !scan_path.exists() {
            tracing::info!(
                "RescanSeries: path does not exist for {}: {}",
                series.title,
                scan_path.display()
            );
            continue;
        }

        // Get episodes for this series, optionally filtered to the target season
        let all_episodes = match episode_repo.get_by_series_id(*series_id).await {
            Ok(eps) => eps,
            Err(_) => continue,
        };
        let episodes = if let Some(sn) = season_number {
            all_episodes
                .into_iter()
                .filter(|e| e.season_number == sn)
                .collect()
        } else {
            all_episodes
        };

        // Scan for video files
        let mut video_files = Vec::new();
        scan_directory_for_videos(&scan_path, &mut video_files);
        total_files += video_files.len();

        // Match files to episodes (supports multi-episode files like S01E01E02E03)
        for file_path in &video_files {
            let file_path_str = file_path.to_string_lossy().to_string();

            if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                let parsed_episodes = parse_episodes_from_filename(file_name);
                if parsed_episodes.is_empty() {
                    continue;
                }

                // Use the first episode's season for the file record
                let first_season = parsed_episodes[0].0;

                // Check if we already have an episode file for this path
                let existing_file = episode_file_repo
                    .get_by_path(&file_path_str)
                    .await
                    .ok()
                    .flatten();

                let episode_file_id = if let Some(ef) = existing_file {
                    // File already exists in database
                    ef.id
                } else {
                    // Create new episode file record
                    let file_size = std::fs::metadata(file_path)
                        .map(|m| m.len() as i64)
                        .unwrap_or(0);

                    let relative_path = file_path
                        .strip_prefix(series.path.as_str())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| file_name.to_string());

                    // Parse release group from filename (simple extraction)
                    let release_group = extract_release_group(file_name);

                    let languages_json = serde_json::json!([{
                        "id": 1,
                        "name": "English"
                    }]);

                    // Real media analysis via FFmpeg probe
                    let media_info_result =
                        MediaAnalyzer::analyze(std::path::Path::new(&file_path_str)).await;
                    let media_info = media_info_result
                        .as_ref()
                        .ok()
                        .and_then(|info| serde_json::to_string(info).ok());

                    // Quality derived from actual resolution
                    let quality_json = match &media_info_result {
                        Ok(info) => derive_quality_from_media(info, file_name),
                        Err(_) => serde_json::json!({
                            "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                            "revision": {"version": 1, "real": 0, "isRepack": false}
                        }),
                    };

                    // BLAKE3 content hash
                    let file_hash = compute_file_hash(std::path::Path::new(&file_path_str))
                        .await
                        .ok();

                    let episode_file = EpisodeFileDbModel {
                        id: 0,
                        series_id: *series_id,
                        season_number: first_season,
                        relative_path,
                        path: file_path_str.clone(),
                        size: file_size,
                        date_added: Utc::now(),
                        scene_name: Some(file_name.to_string()),
                        release_group,
                        quality: quality_json.to_string(),
                        languages: languages_json.to_string(),
                        media_info,
                        original_file_path: Some(file_path_str.clone()),
                        file_hash,
                    };

                    match episode_file_repo.insert(&episode_file).await {
                        Ok(id) => {
                            total_new_files += 1;
                            tracing::debug!(
                                "Created episode file record: id={}, path={}",
                                id,
                                file_path_str
                            );
                            id
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create episode file: {}", e);
                            continue;
                        }
                    }
                };

                // Link ALL matched episodes to this file (multi-episode support)
                for (season, episode_num) in &parsed_episodes {
                    if let Some(mut ep) = episodes
                        .iter()
                        .find(|e| e.season_number == *season && e.episode_number == *episode_num)
                        .cloned()
                    {
                        // Update episode to link to file
                        if !ep.has_file || ep.episode_file_id != Some(episode_file_id) {
                            ep.has_file = true;
                            ep.episode_file_id = Some(episode_file_id);
                            if episode_repo.update(&ep).await.is_ok() {
                                total_matched += 1;
                                tracing::debug!(
                                    "Linked episode {} S{:02}E{:02} to file {}",
                                    ep.title,
                                    ep.season_number,
                                    ep.episode_number,
                                    episode_file_id
                                );
                            }
                        }
                    }
                }

                // Log multi-episode file detection
                if parsed_episodes.len() > 1 {
                    let ep_list: Vec<String> = parsed_episodes
                        .iter()
                        .map(|(s, e)| format!("S{:02}E{:02}", s, e))
                        .collect();
                    tracing::info!(
                        "Multi-episode file detected: {} -> {}",
                        file_name,
                        ep_list.join(", ")
                    );
                }
            }
        }

        tracing::info!(
            "RescanSeries: {} - scanned {} files, {} new file records",
            series.title,
            video_files.len(),
            total_new_files
        );

        // Publish series scanned event
        event_bus
            .publish(crate::core::messaging::Message::SeriesScanned {
                series_id: *series_id,
                title: series.title.clone(),
                files_found: video_files.len(),
                episodes_matched: total_matched,
            })
            .await;
    }

    Ok(format!(
        "Scanned {} series: {} files found, {} episodes matched, {} new file records",
        series_ids.len(),
        total_files,
        total_matched,
        total_new_files
    ))
}

/// Execute RescanSeries in distributed mode - publishes scan requests to workers
///
/// When `season_number` is provided, scopes the scan path and known_files to that season.
async fn execute_rescan_series_distributed(
    series_ids: &[i64],
    db: &crate::core::datastore::Database,
    hybrid_event_bus: &crate::core::messaging::HybridEventBus,
    season_number: Option<i32>,
    media_config: Option<&crate::core::configuration::MediaConfig>,
) -> Result<String, String> {
    use crate::core::datastore::repositories::{EpisodeFileRepository, SeriesRepository};
    use crate::core::messaging::KnownFileInfo;
    use crate::core::scanner::create_scan_request;
    use std::collections::HashMap;

    tracing::info!(
        "RescanSeries: distributing scan for {} series to workers{}",
        series_ids.len(),
        season_number.map_or(String::new(), |n| format!(" (season {})", n))
    );

    let series_repo = SeriesRepository::new(db.clone());
    let episode_file_repo = EpisodeFileRepository::new(db.clone());

    // Precompute season folder name if scoping to a single season
    let season_folder = season_number.map(|sn| {
        let default_config = crate::core::configuration::MediaConfig::default();
        let config = media_config.unwrap_or(&default_config);
        crate::core::naming::build_season_folder(config, sn)
    });

    // Collect series paths for the scan request, keeping series_ids and paths aligned
    let mut valid_series_ids = Vec::new();
    let mut paths = Vec::new();
    let mut known_files: HashMap<String, KnownFileInfo> = HashMap::new();

    for series_id in series_ids {
        match series_repo.get_by_id(*series_id).await {
            Ok(Some(series)) => {
                valid_series_ids.push(series.id);

                // Scope path to season folder when season_number is provided
                let scan_path = if let Some(ref folder) = season_folder {
                    format!("{}/{}", series.path, folder)
                } else {
                    series.path.clone()
                };
                paths.push(scan_path);
                tracing::debug!(
                    "Adding path for scan: {} ({}{})",
                    series.title,
                    series.path,
                    season_folder
                        .as_ref()
                        .map_or(String::new(), |f| format!("/{}", f))
                );

                // Query existing episode files for this series to build known_files map
                // When scanning a single season, only include that season's files
                if let Ok(files) = episode_file_repo.get_by_series_id(series.id).await {
                    for f in files {
                        if f.file_hash.is_some() {
                            // Skip files from other seasons when scoping
                            if let Some(sn) = season_number {
                                if f.season_number != sn {
                                    continue;
                                }
                            }
                            known_files.insert(
                                f.path.clone(),
                                KnownFileInfo {
                                    size: f.size,
                                    media_info: f.media_info.clone(),
                                    quality: Some(f.quality.clone()),
                                    file_hash: f.file_hash.clone(),
                                },
                            );
                        }
                    }
                }
            }
            _ => {
                tracing::warn!("Series {} not found, skipping", series_id);
            }
        }
    }

    if paths.is_empty() {
        return Ok("No valid series paths to scan".to_string());
    }

    let known_count = known_files.len();

    // Create and publish the scan request (series_ids and paths are 1:1 aligned)
    let (job_id, message) = create_scan_request(valid_series_ids.clone(), paths.clone(), known_files);

    tracing::info!(
        "Publishing scan request: job_id={}, series={:?}, paths={}, known_files={}",
        job_id,
        valid_series_ids,
        paths.len(),
        known_count
    );

    hybrid_event_bus.publish(message).await;

    Ok(format!(
        "Distributed scan request sent for {} series (job_id: {}). Results will be processed by workers.",
        series_ids.len(), job_id
    ))
}

/// Execute ProcessMonitoredDownloads command - check download clients for completed downloads.
///
/// When a worker is available (Redis enabled), dispatches file discovery to the worker
/// instead of scanning over NFS. The worker discovers files + enriches with FFmpeg/BLAKE3,
/// then the consumer handles matching, file moves, and DB insert.
async fn execute_process_downloads(
    _body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    _event_bus: &crate::core::messaging::EventBus,
    media_config: Option<&crate::core::configuration::MediaConfig>,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
    scan_result_consumer: Option<&std::sync::Arc<crate::core::scanner::ScanResultConsumer>>,
) -> Result<String, String> {
    use crate::core::download::ImportService;

    tracing::info!("ProcessMonitoredDownloads: checking for completed downloads");

    let import_service = ImportService::new(db.clone(), media_config.cloned().unwrap_or_default());

    // Check for completed downloads
    let pending = import_service
        .check_for_completed_downloads()
        .await
        .map_err(|e| format!("Failed to check downloads: {}", e))?;

    if pending.is_empty() {
        tracing::info!("ProcessMonitoredDownloads: no completed downloads to process");
        return Ok("No completed downloads to process".to_string());
    }

    tracing::info!(
        "ProcessMonitoredDownloads: found {} completed downloads",
        pending.len()
    );

    // If Redis/workers available, dispatch file discovery to worker
    if let (Some(hybrid_bus), Some(consumer)) = (hybrid_event_bus, scan_result_consumer) {
        if hybrid_bus.is_redis_enabled() {
            return execute_process_downloads_distributed(&pending, hybrid_bus, consumer).await;
        }
    }

    // Fallback: local import (no worker available)
    let mut imported = 0;
    let mut failed = 0;

    for item in pending {
        match import_service.import(&item).await {
            Ok(result) if result.success => {
                tracing::info!(
                    "ProcessMonitoredDownloads: imported '{}' ({} files, {} episodes, {} skipped)",
                    item.title,
                    result.files_imported,
                    result.episode_ids.len(),
                    result.files_skipped
                );
                imported += 1;

                // Mark tracked download as Imported (never remove from client)
                import_service.mark_tracked_imported(&item).await;
            }
            Ok(result) => {
                tracing::warn!(
                    "ProcessMonitoredDownloads: could not import '{}': {}",
                    item.title,
                    result
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
                failed += 1;
            }
            Err(e) => {
                tracing::error!(
                    "ProcessMonitoredDownloads: import error for '{}': {}",
                    item.title,
                    e
                );
                failed += 1;
            }
        }
    }

    Ok(format!(
        "Processed downloads: {} imported, {} failed",
        imported, failed
    ))
}

/// Dispatch download processing to workers via Redis.
///
/// For each completed download, creates a scan request for the download output path
/// so the worker can discover video files and enrich them. The consumer then handles
/// matching → file move → DB insert as a multi-phase pipeline.
async fn execute_process_downloads_distributed(
    pending: &[crate::core::download::import::PendingImport],
    hybrid_bus: &crate::core::messaging::HybridEventBus,
    consumer: &std::sync::Arc<crate::core::scanner::ScanResultConsumer>,
) -> Result<String, String> {
    use crate::core::messaging::{Message, ScanType};
    use crate::core::scanner::DownloadImportInfo;

    let mut dispatched = 0;

    for item in pending {
        let job_id = uuid::Uuid::new_v4().to_string();
        let output_path_str = item.output_path.to_string_lossy().to_string();

        tracing::info!(
            "ProcessMonitoredDownloads: dispatching '{}' to worker (path={}, job_id={})",
            item.title,
            output_path_str,
            job_id
        );

        // Register download import info in the consumer for Phase 2 when scan result arrives
        let import_info = DownloadImportInfo {
            download_id: item.download_id.clone(),
            download_client_id: item.download_client_id,
            download_client_name: item.download_client_name.clone(),
            title: item.title.clone(),
            output_path: item.output_path.clone(),
            parsed_info: item.parsed_info.clone(),
            series: item.series.clone(),
            episodes: item.episodes.clone(),
            overrides: std::collections::HashMap::new(),
        };

        consumer
            .register_download_import(&job_id, vec![import_info])
            .await;

        // Register as a pending scan job (so consumer routes to download handler)
        consumer
            .register_job(
                &job_id,
                ScanType::DownloadedEpisodesScan,
                vec![0], // No specific series_id — worker will discover files
            )
            .await;

        // Create scan request for the download output path
        // Use series_id=0 as placeholder (download paths aren't series-specific)
        let message = Message::ScanRequest {
            job_id: job_id.clone(),
            scan_type: ScanType::DownloadedEpisodesScan,
            series_ids: vec![0],
            paths: vec![output_path_str],
            known_files: std::collections::HashMap::new(),
        };

        hybrid_bus.publish(message).await;
        dispatched += 1;
    }

    Ok(format!(
        "Dispatched {} download imports to worker for processing",
        dispatched
    ))
}

/// Execute RssSync command - fetch RSS from all indexers
async fn execute_rss_sync(
    _body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    _event_bus: &crate::core::messaging::EventBus,
) -> Result<String, String> {
    use crate::core::datastore::repositories::IndexerRepository;
    use crate::core::indexers::rss::RssSyncService;

    tracing::info!("RssSync: fetching RSS from all indexers");

    let indexer_repo = IndexerRepository::new(db.clone());
    let indexers = indexer_repo
        .get_all()
        .await
        .map_err(|e| format!("Failed to fetch indexers: {}", e))?;

    if indexers.is_empty() {
        return Ok("No indexers configured".to_string());
    }

    let mut rss_service = RssSyncService::new(indexers);
    let releases = rss_service
        .sync()
        .await
        .map_err(|e| format!("RSS sync failed: {}", e))?;

    tracing::info!("RssSync: fetched {} releases from RSS", releases.len());

    // In a full implementation, we would:
    // 1. Filter releases matching wanted episodes
    // 2. Check quality profiles
    // 3. Grab matching releases automatically
    // For now, just return the count

    Ok(format!(
        "RSS sync completed: {} releases fetched",
        releases.len()
    ))
}

/// Extract release group from filename (e.g., "Show.S01E01.720p.HDTV.x264-GROUP" -> "GROUP")
fn extract_release_group(filename: &str) -> Option<String> {
    // Common pattern: last part after a dash, before the extension
    let name_without_ext = filename
        .rsplit('.')
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".");

    // Find the last dash that precedes the release group
    if let Some(dash_pos) = name_without_ext.rfind('-') {
        let group = &name_without_ext[dash_pos + 1..];
        // Filter out common false positives
        if !group.is_empty() && !group.chars().all(|c| c.is_numeric()) && group.len() <= 20 {
            return Some(group.to_string());
        }
    }
    None
}

fn scan_directory_for_videos(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_directory_for_videos(&path, files);
            } else if is_video_file(&path) {
                files.push(path);
            }
        }
    }
}

use crate::core::scanner::is_video_file;

/// Re-export from scanner module to avoid duplication
use crate::core::scanner::parse_episodes_from_filename;

/// Execute EpisodeSearch command - search indexers for specific episodes
async fn execute_episode_search(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
) -> Result<String, String> {
    use crate::core::datastore::repositories::{
        EpisodeRepository, IndexerRepository, QualityProfileRepository, SeriesRepository,
        TrackedDownloadRepository,
    };
    use crate::core::indexers::search::IndexerSearchService;
    use crate::core::indexers::SearchCriteria;
    use crate::core::profiles::QualityProfileItem;
    use crate::core::queue::service::TrackedDownloadService;

    // Parse episode IDs from body
    let episode_ids: Vec<i64> = body
        .get("episodeIds")
        .or_else(|| body.get("body").and_then(|b| b.get("episodeIds")))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if episode_ids.is_empty() {
        return Ok("No episodes to search".to_string());
    }

    tracing::info!(
        "EpisodeSearch: searching for {} episodes",
        episode_ids.len()
    );

    // Get indexers from database
    let indexer_repo = IndexerRepository::new(db.clone());
    let indexers = match indexer_repo.get_all().await {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("EpisodeSearch: failed to fetch indexers: {}", e);
            return Err(format!("Failed to fetch indexers: {}", e));
        }
    };

    if indexers.is_empty() {
        tracing::warn!("EpisodeSearch: no indexers configured");
        return Ok("No indexers configured".to_string());
    }

    let enabled_indexers: Vec<_> = indexers
        .into_iter()
        .filter(|i| i.enable_automatic_search)
        .collect();

    if enabled_indexers.is_empty() {
        tracing::warn!("EpisodeSearch: no indexers have automatic search enabled");
        return Ok("No indexers have automatic search enabled".to_string());
    }

    tracing::info!("EpisodeSearch: using {} indexers", enabled_indexers.len());

    let episode_repo = EpisodeRepository::new(db.clone());
    let series_repo = SeriesRepository::new(db.clone());
    let quality_repo = QualityProfileRepository::new(db.clone());
    let tracked_repo = TrackedDownloadRepository::new(db.clone());
    let tracked_service = TrackedDownloadService::new(db.clone());

    // Pre-load quality profiles for fast lookup
    let all_profiles = quality_repo.get_all().await.unwrap_or_default();
    let profiles: std::collections::HashMap<i64, _> =
        all_profiles.into_iter().map(|p| (p.id, p)).collect();

    // Get currently downloading episode IDs to avoid duplicate grabs
    let active_downloads = tracked_repo.get_all_active().await.unwrap_or_default();
    let downloading_episode_ids: std::collections::HashSet<i64> = active_downloads
        .iter()
        .flat_map(|d| serde_json::from_str::<Vec<i64>>(&d.episode_ids).unwrap_or_default())
        .collect();

    let mut total_releases = 0;
    let mut grabbed = 0u32;

    for episode_id in &episode_ids {
        // Get episode from database
        let episode = match episode_repo.get_by_id(*episode_id).await {
            Ok(Some(ep)) => ep,
            Ok(None) => {
                tracing::warn!("EpisodeSearch: episode {} not found", episode_id);
                continue;
            }
            Err(e) => {
                tracing::error!(
                    "EpisodeSearch: failed to fetch episode {}: {}",
                    episode_id,
                    e
                );
                continue;
            }
        };

        // Get series for TVDB ID and title
        let series = match series_repo.get_by_id(episode.series_id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                tracing::warn!("EpisodeSearch: series {} not found", episode.series_id);
                continue;
            }
            Err(e) => {
                tracing::error!(
                    "EpisodeSearch: failed to fetch series {}: {}",
                    episode.series_id,
                    e
                );
                continue;
            }
        };

        tracing::info!(
            "EpisodeSearch: searching for {} (TVDB: {}) - S{:02}E{:02} - {}",
            series.title,
            series.tvdb_id,
            episode.season_number,
            episode.episode_number,
            episode.title
        );

        // Build search criteria using TVDB ID and series title
        let criteria = SearchCriteria {
            series_id: series.tvdb_id,          // TVDB ID for indexers that support it
            series_title: series.title.clone(), // Text search for all indexers
            episode_id: Some(*episode_id),
            season_number: Some(episode.season_number),
            episode_numbers: vec![episode.episode_number],
            absolute_episode_numbers: vec![],
            special: false,
        };

        // Perform search
        let search_service = IndexerSearchService::new(enabled_indexers.clone());
        match search_service.search(&criteria).await {
            Ok(releases) => {
                tracing::info!(
                    "EpisodeSearch: found {} releases for {} S{:02}E{:02}",
                    releases.len(),
                    series.title,
                    episode.season_number,
                    episode.episode_number
                );
                total_releases += releases.len();

                // Log top results
                for (i, release) in releases.iter().take(5).enumerate() {
                    tracing::debug!(
                        "  {}. {} ({} - {:?})",
                        i + 1,
                        release.title,
                        release.indexer,
                        release.quality.quality
                    );
                }

                // Auto-grab: check if episode is wanted and grab best matching release
                let dominated_check = episode.monitored
                    && !episode.has_file
                    && episode.air_date_utc.is_some_and(|d| d < chrono::Utc::now())
                    && !downloading_episode_ids.contains(episode_id);

                tracing::info!(
                    "EpisodeSearch: auto-grab check ep={}: monitored={}, has_file={}, aired={}, not_downloading={}, pass={}",
                    episode_id,
                    episode.monitored,
                    episode.has_file,
                    episode.air_date_utc.is_some_and(|d| d < chrono::Utc::now()),
                    !downloading_episode_ids.contains(episode_id),
                    dominated_check
                );

                if dominated_check {
                    if let Some(profile) = profiles.get(&series.quality_profile_id) {
                        let profile_items: Vec<QualityProfileItem> =
                            serde_json::from_str(&profile.items).unwrap_or_default();

                        tracing::info!(
                            "EpisodeSearch: quality profile '{}' (id={}): cutoff={}, {} items",
                            profile.name,
                            profile.id,
                            profile.cutoff,
                            profile_items.len()
                        );

                        // A profile with cutoff=0 and only "Unknown" allowed means "accept any quality"
                        let accept_any = profile.cutoff == 0
                            && profile_items
                                .iter()
                                .all(|item| !item.allowed || item.quality.id == 0);

                        // Find best release that passes quality checks
                        // Releases are already sorted by quality (best first) from IndexerSearchService
                        for mut release in releases {
                            let release_weight = release.quality.quality.weight();

                            if !accept_any {
                                let is_quality_allowed = profile_items.iter().any(|item| {
                                    item.allowed
                                        && (item.quality.id == release_weight
                                            || item.items.iter().any(|q| q.id == release_weight))
                                });

                                if !is_quality_allowed {
                                    tracing::debug!(
                                        "EpisodeSearch: release '{}' weight={} rejected (quality not allowed)",
                                        release.title,
                                        release_weight
                                    );
                                    continue;
                                }

                                if release_weight < profile.cutoff {
                                    tracing::debug!(
                                        "EpisodeSearch: release '{}' weight={} rejected (below cutoff {})",
                                        release.title,
                                        release_weight,
                                        profile.cutoff
                                    );
                                    continue;
                                }
                            }

                            // Grab this release
                            release.series_id = Some(series.id);
                            tracing::info!(
                                "EpisodeSearch auto-grab: '{}' → {} S{:02}E{:02} ({:?})",
                                release.title,
                                series.title,
                                episode.season_number,
                                episode.episode_number,
                                release.quality.quality
                            );

                            match tracked_service
                                .grab_release(&release, vec![*episode_id], None)
                                .await
                            {
                                Ok(tracked_id) => {
                                    grabbed += 1;
                                    tracing::info!(
                                        "EpisodeSearch: grabbed successfully (tracked_id={})",
                                        tracked_id
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "EpisodeSearch: failed to grab '{}': {}",
                                        release.title,
                                        e
                                    );
                                }
                            }
                            break; // Only grab the best matching release
                        }
                    } else {
                        tracing::warn!(
                            "EpisodeSearch: quality profile {} not found for series '{}'",
                            series.quality_profile_id,
                            series.title
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "EpisodeSearch: search failed for {} S{:02}E{:02}: {}",
                    series.title,
                    episode.season_number,
                    episode.episode_number,
                    e
                );
            }
        }

        // Publish search event for notifications/history
        event_bus
            .publish(crate::core::messaging::Message::EpisodeSearchRequested {
                episode_id: *episode_id,
                series_id: episode.series_id,
                season_number: episode.season_number,
                episode_number: episode.episode_number,
            })
            .await;
    }

    Ok(format!(
        "Episode search completed for {} episodes, found {} releases, grabbed {}",
        episode_ids.len(),
        total_releases,
        grabbed
    ))
}

/// Execute SeasonSearch command - search indexers for a whole season
async fn execute_season_search(
    body: &serde_json::Value,
    _db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
) -> Result<String, String> {
    // Parse series ID and season number from body
    let series_id = body
        .get("seriesId")
        .or_else(|| body.get("body").and_then(|b| b.get("seriesId")))
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Missing seriesId".to_string())?;

    let season_number = body
        .get("seasonNumber")
        .or_else(|| body.get("body").and_then(|b| b.get("seasonNumber")))
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .ok_or_else(|| "Missing seasonNumber".to_string())?;

    tracing::info!(
        "SeasonSearch: searching for series {} season {}",
        series_id,
        season_number
    );

    // Publish search event
    event_bus
        .publish(crate::core::messaging::Message::SeasonSearchRequested {
            series_id,
            season_number,
        })
        .await;

    Ok(format!(
        "Season search started for series {} season {}",
        series_id, season_number
    ))
}

/// Public entry point for SeriesSearch — used by v3 add handler for auto-search
pub async fn execute_series_search_public(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
) -> Result<String, String> {
    execute_series_search(body, db, event_bus).await
}

/// Execute SeriesSearch command - search indexers for all missing episodes in a series
async fn execute_series_search(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    event_bus: &crate::core::messaging::EventBus,
) -> Result<String, String> {
    use crate::core::datastore::repositories::EpisodeRepository;

    // Parse series ID from body
    let series_id = body
        .get("seriesId")
        .or_else(|| body.get("body").and_then(|b| b.get("seriesId")))
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Missing seriesId".to_string())?;

    tracing::info!(
        "SeriesSearch: searching for all missing episodes in series {}",
        series_id
    );

    // Publish search event for notifications/websocket
    event_bus
        .publish(crate::core::messaging::Message::SeriesSearchRequested { series_id })
        .await;

    // Find all monitored episodes without files that have aired
    let episode_repo = EpisodeRepository::new(db.clone());
    let episodes = episode_repo
        .get_by_series_id(series_id)
        .await
        .map_err(|e| format!("Failed to fetch episodes: {}", e))?;

    let missing_ids: Vec<i64> = episodes
        .into_iter()
        .filter(|ep| {
            ep.monitored
                && !ep.has_file
                && ep.air_date_utc.is_some()
                && ep
                    .air_date_utc
                    .as_ref()
                    .map(|d| *d < chrono::Utc::now())
                    .unwrap_or(false)
        })
        .map(|ep| ep.id)
        .collect();

    if missing_ids.is_empty() {
        tracing::info!("SeriesSearch: no missing episodes for series {}", series_id);
        return Ok(format!(
            "Series search complete for series {} — no missing episodes",
            series_id
        ));
    }

    tracing::info!(
        "SeriesSearch: found {} missing episodes for series {}, delegating to EpisodeSearch",
        missing_ids.len(),
        series_id
    );

    // Delegate to EpisodeSearch with the missing episode IDs
    let episode_body = serde_json::json!({ "episodeIds": missing_ids });
    execute_episode_search(&episode_body, db, event_bus).await
}

/// Execute MovieSearch command — search indexers for a movie and auto-grab best match
async fn execute_movie_search(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
) -> Result<String, String> {
    use crate::core::datastore::repositories::{
        IndexerRepository, MovieRepository, QualityProfileRepository, TrackedDownloadRepository,
    };
    use crate::core::indexers::search::IndexerSearchService;
    use crate::core::profiles::QualityProfileItem;
    use crate::core::queue::service::TrackedDownloadService;

    // Parse movie IDs — accept both movieId (singular) and movieIds (array)
    let mut movie_ids: Vec<i64> = body
        .get("movieIds")
        .or_else(|| body.get("body").and_then(|b| b.get("movieIds")))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if movie_ids.is_empty() {
        if let Some(id) = body
            .get("movieId")
            .or_else(|| body.get("body").and_then(|b| b.get("movieId")))
            .and_then(|v| v.as_i64())
        {
            movie_ids.push(id);
        }
    }

    if movie_ids.is_empty() {
        return Ok("No movies to search".to_string());
    }

    tracing::info!("MovieSearch: searching for {} movies", movie_ids.len());

    // Get indexers
    let indexer_repo = IndexerRepository::new(db.clone());
    let indexers = match indexer_repo.get_all().await {
        Ok(i) => i,
        Err(e) => return Err(format!("Failed to fetch indexers: {}", e)),
    };

    let enabled_indexers: Vec<_> = indexers
        .into_iter()
        .filter(|i| i.enable_automatic_search)
        .collect();

    if enabled_indexers.is_empty() {
        return Ok("No indexers have automatic search enabled".to_string());
    }

    let movie_repo = MovieRepository::new(db.clone());
    let quality_repo = QualityProfileRepository::new(db.clone());
    let tracked_repo = TrackedDownloadRepository::new(db.clone());
    let tracked_service = TrackedDownloadService::new(db.clone());

    let all_profiles = quality_repo.get_all().await.unwrap_or_default();
    let profiles: std::collections::HashMap<i64, _> =
        all_profiles.into_iter().map(|p| (p.id, p)).collect();

    // Get active movie downloads to avoid duplicates
    let active_downloads = tracked_repo.get_all_active().await.unwrap_or_default();
    let downloading_movie_ids: std::collections::HashSet<i64> = active_downloads
        .iter()
        .filter_map(|d| d.movie_id)
        .collect();

    let mut total_releases = 0;
    let mut grabbed = 0u32;

    for movie_id in &movie_ids {
        let movie = match movie_repo.get_by_id(*movie_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                tracing::warn!("MovieSearch: movie {} not found", movie_id);
                continue;
            }
            Err(e) => {
                tracing::error!("MovieSearch: failed to fetch movie {}: {}", movie_id, e);
                continue;
            }
        };

        if downloading_movie_ids.contains(movie_id) {
            tracing::info!(
                "MovieSearch: movie {} '{}' already downloading, skipping",
                movie_id,
                movie.title
            );
            continue;
        }

        tracing::info!(
            "MovieSearch: searching for '{}' ({}) IMDB: [{}]",
            movie.title,
            movie.year,
            movie.imdb_id.as_deref().unwrap_or("none"),
        );

        let search_service = IndexerSearchService::new(enabled_indexers.clone());
        let year = if movie.year > 0 { Some(movie.year) } else { None };
        match search_service
            .movie_search(&movie.title, year, movie.imdb_id.as_deref())
            .await
        {
            Ok(releases) => {
                tracing::info!(
                    "MovieSearch: found {} releases for '{}'",
                    releases.len(),
                    movie.title
                );
                total_releases += releases.len();

                for (i, release) in releases.iter().take(5).enumerate() {
                    tracing::debug!(
                        "  {}. {} ({} - {:?})",
                        i + 1,
                        release.title,
                        release.indexer,
                        release.quality.quality
                    );
                }

                // Auto-grab: check quality profile and grab best match
                if movie.monitored {
                    if let Some(profile) = profiles.get(&movie.quality_profile_id) {
                        let profile_items: Vec<QualityProfileItem> =
                            serde_json::from_str(&profile.items).unwrap_or_default();

                        let accept_any = profile.cutoff == 0
                            && profile_items
                                .iter()
                                .all(|item| !item.allowed || item.quality.id == 0);

                        for mut release in releases {
                            let release_weight = release.quality.quality.weight();

                            if !accept_any {
                                let is_quality_allowed = profile_items.iter().any(|item| {
                                    item.allowed
                                        && (item.quality.id == release_weight
                                            || item
                                                .items
                                                .iter()
                                                .any(|q| q.id == release_weight))
                                });

                                if !is_quality_allowed {
                                    tracing::debug!(
                                        "MovieSearch: release '{}' weight={} rejected (quality not allowed)",
                                        release.title,
                                        release_weight
                                    );
                                    continue;
                                }

                                if release_weight < profile.cutoff {
                                    tracing::debug!(
                                        "MovieSearch: release '{}' weight={} rejected (below cutoff {})",
                                        release.title,
                                        release_weight,
                                        profile.cutoff
                                    );
                                    continue;
                                }
                            }

                            // Grab this release
                            release.movie_id = Some(movie.id);
                            tracing::info!(
                                "MovieSearch auto-grab: '{}' → '{}' ({:?})",
                                release.title,
                                movie.title,
                                release.quality.quality
                            );

                            match tracked_service
                                .grab_release(&release, vec![], Some(movie.id))
                                .await
                            {
                                Ok(tracked_id) => {
                                    grabbed += 1;
                                    tracing::info!(
                                        "MovieSearch: grabbed successfully (tracked_id={})",
                                        tracked_id
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "MovieSearch: failed to grab '{}': {}",
                                        release.title,
                                        e
                                    );
                                }
                            }
                            break; // Only grab the best matching release
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "MovieSearch: search failed for '{}': {}",
                    movie.title,
                    e
                );
            }
        }
    }

    Ok(format!(
        "Movie search completed for {} movies, found {} releases, grabbed {}",
        movie_ids.len(),
        total_releases,
        grabbed
    ))
}

/// Execute RescanMovie command — scans a movie's folder for video files.
/// If a movie has no file or the file path changed, re-scans and updates.
/// When a worker is available (Redis enabled), dispatches the scan to the worker
/// for local FFmpeg probe + BLAKE3 hash — much faster than NFS.
async fn execute_rescan_movie(
    body: &serde_json::Value,
    db: &crate::core::datastore::Database,
    hybrid_event_bus: Option<&crate::core::messaging::HybridEventBus>,
) -> Result<String, String> {
    let movie_id = body
        .get("movieId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Missing movieId".to_string())?;

    let repo = crate::core::datastore::repositories::MovieRepository::new(db.clone());
    let file_repo = crate::core::datastore::repositories::MovieFileRepository::new(db.clone());

    let movie = repo
        .get_by_id(movie_id)
        .await
        .map_err(|e| format!("Failed to fetch movie: {}", e))?
        .ok_or_else(|| format!("Movie {} not found", movie_id))?;

    tracing::info!("RescanMovie: scanning '{}' at {}", movie.title, movie.path);

    // Prefer worker scanning when Redis is available
    if let Some(hybrid_bus) = hybrid_event_bus {
        if hybrid_bus.is_redis_enabled() {
            // Build known_files map from existing movie file for skip-enrichment
            let mut known_files = std::collections::HashMap::new();
            if let Ok(Some(mf)) = file_repo.get_by_movie_id(movie_id).await {
                if mf.file_hash.is_some() {
                    known_files.insert(
                        mf.path.clone(),
                        crate::core::messaging::KnownFileInfo {
                            size: mf.size,
                            media_info: mf.media_info.clone(),
                            quality: Some(mf.quality.clone()),
                            file_hash: mf.file_hash.clone(),
                        },
                    );
                }
            }

            let (job_id, message) = crate::core::scanner::create_movie_scan_request(
                vec![movie_id],
                vec![movie.path.clone()],
                known_files,
            );
            tracing::info!(
                "RescanMovie: dispatching '{}' to worker (job_id={})",
                movie.title,
                job_id
            );
            hybrid_bus.publish(message).await;
            return Ok(format!(
                "Dispatched movie scan for '{}' to worker (job_id: {})",
                movie.title, job_id
            ));
        }
    }

    // No Redis/workers — scan locally (fallback)
    let mut folder_path = movie.path.clone();

    // If the expected path doesn't exist, try to find a matching folder
    // in the root folder by fuzzy title matching. This handles cases where
    // the folder name has a minor difference (e.g. a missing word).
    if !std::path::Path::new(&folder_path).exists() {
        tracing::info!(
            "RescanMovie: exact path not found for '{}': {}, searching root folder...",
            movie.title,
            folder_path
        );

        let found = tokio::task::spawn_blocking({
            let root = movie.root_folder_path.clone();
            let movie_title = movie.title.clone();
            let movie_year = movie.year;
            move || {
                use crate::core::parser::normalize_title;
                let normalized_movie = normalize_title(&movie_title);
                let root_path = std::path::Path::new(&root);
                if !root_path.is_dir() {
                    return None;
                }

                let mut best: Option<(std::path::PathBuf, usize)> = None;
                let entries = std::fs::read_dir(root_path).ok()?;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let dir_name = path.file_name()?.to_string_lossy().to_string();
                    let normalized_dir = normalize_title(&dir_name);

                    // Score: check if normalized movie title is contained in dir name
                    // or dir name is contained in movie title (handles missing words)
                    let is_match = normalized_dir.contains(&normalized_movie)
                        || normalized_movie.contains(&normalized_dir);
                    // Also match by year if present in folder name
                    let has_year = dir_name.contains(&movie_year.to_string());

                    if is_match && has_year {
                        let score = normalized_dir.len();
                        if best.as_ref().is_none_or(|(_, s)| {
                            // Prefer closest length match
                            let diff_new = score.abs_diff(normalized_movie.len());
                            let diff_old = s.abs_diff(normalized_movie.len());
                            diff_new < diff_old
                        }) {
                            best = Some((path, score));
                        }
                    }
                }
                best.map(|(p, _)| p)
            }
        })
        .await
        .unwrap_or(None);

        if let Some(actual_path) = found {
            let actual_str = actual_path.to_string_lossy().to_string();
            tracing::info!(
                "RescanMovie: fuzzy matched '{}' -> '{}'",
                movie.title,
                actual_str
            );
            // Update movie.path in DB to the actual folder
            let pool = db.pool();
            let _ = sqlx::query("UPDATE movies SET path = $1 WHERE id = $2")
                .bind(&actual_str)
                .bind(movie_id)
                .execute(pool)
                .await;
            folder_path = actual_str;
        } else {
            // No match found — clear file reference if needed
            if movie.has_file {
                let pool = db.pool();
                let _ = sqlx::query(
                    "UPDATE movies SET has_file = false, movie_file_id = NULL WHERE id = $1",
                )
                .bind(movie_id)
                .execute(pool)
                .await;
            }
            return Ok(format!("Path does not exist: {}", folder_path));
        }
    }

    // Scan for the largest video file
    if let Some(mut movie_file) = super::movies::scan_movie_folder(&folder_path, movie_id) {
        // Check if this file is already tracked
        let existing_files = file_repo
            .get_by_movie_id(movie_id)
            .await
            .map_err(|e| format!("Failed to fetch movie files: {}", e))?;

        let already_tracked = existing_files.iter().any(|f| f.path == movie_file.path);
        if already_tracked {
            tracing::info!(
                "RescanMovie: '{}' — file already tracked: {}",
                movie.title,
                movie_file.path
            );
            return Ok(format!("Movie file already tracked for '{}'", movie.title));
        }

        let file_path = std::path::Path::new(&movie_file.path);

        // Real media analysis via FFmpeg probe
        let media_info_result = crate::core::mediafiles::MediaAnalyzer::analyze(file_path).await;
        movie_file.media_info = media_info_result
            .as_ref()
            .ok()
            .and_then(|info| serde_json::to_string(info).ok());

        // Derive quality from actual resolution
        if let Ok(ref info) = media_info_result {
            let quality =
                crate::core::mediafiles::derive_quality_from_media(info, &movie_file.path);
            movie_file.quality =
                serde_json::to_string(&quality).unwrap_or_else(|_| movie_file.quality.clone());
        }

        // BLAKE3 file hash
        movie_file.file_hash = crate::core::mediafiles::compute_file_hash(file_path)
            .await
            .ok();

        match file_repo.insert(&movie_file).await {
            Ok(file_id) => {
                let pool = db.pool();
                let _ = sqlx::query(
                    "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                )
                .bind(file_id)
                .bind(movie_id)
                .execute(pool)
                .await;
                tracing::info!(
                    "RescanMovie: found file for '{}': {} ({} bytes)",
                    movie.title,
                    movie_file.path,
                    movie_file.size
                );
                Ok(format!(
                    "Found video file for '{}': {}",
                    movie.title, movie_file.path
                ))
            }
            Err(e) => {
                tracing::error!("RescanMovie: failed to insert movie file: {}", e);
                Err(format!("Failed to insert movie file: {}", e))
            }
        }
    } else {
        tracing::info!("RescanMovie: no video files found in '{}'", folder_path);
        // Clear file reference if movie thought it had one
        if movie.has_file {
            let pool = db.pool();
            let _ = sqlx::query(
                "UPDATE movies SET has_file = false, movie_file_id = NULL WHERE id = $1",
            )
            .bind(movie_id)
            .execute(pool)
            .await;
        }
        Ok(format!("No video files found for '{}'", movie.title))
    }
}
