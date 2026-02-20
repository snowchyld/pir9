//! Scan result consumer service
//!
//! This service runs on the server and processes scan results from distributed workers.
//! It updates the database with discovered files and links them to episodes/movies.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::datastore::models::EpisodeFileDbModel;
use crate::core::datastore::repositories::{
    EpisodeFileRepository, EpisodeRepository, MovieFileRepository, MovieRepository,
    SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::mediafiles::{compute_file_hash, derive_quality_from_media, MediaAnalyzer};
use crate::core::messaging::{HybridEventBus, Message, ScanType, ScannedFile};

/// Tracks pending scan jobs and their results
#[derive(Debug, Default)]
pub struct PendingScanJobs {
    /// Maps job_id -> pending job state
    jobs: HashMap<String, PendingJob>,
}

#[derive(Debug)]
struct PendingJob {
    scan_type: ScanType,
    /// series_ids for RescanSeries, movie_ids for RescanMovie
    entity_ids: Vec<i64>,
    results_received: usize,
    completed: bool,
}

/// Service that consumes scan results from workers and updates the database
pub struct ScanResultConsumer {
    db: Database,
    event_bus: HybridEventBus,
    pending_jobs: Arc<RwLock<PendingScanJobs>>,
}

impl ScanResultConsumer {
    /// Create a new scan result consumer
    pub fn new(db: Database, event_bus: HybridEventBus) -> Self {
        Self {
            db,
            event_bus,
            pending_jobs: Arc::new(RwLock::new(PendingScanJobs::default())),
        }
    }

    /// Register a pending scan job with its scan type
    pub async fn register_job(&self, job_id: &str, scan_type: ScanType, entity_ids: Vec<i64>) {
        let mut jobs = self.pending_jobs.write().await;
        jobs.jobs.insert(
            job_id.to_string(),
            PendingJob {
                scan_type,
                entity_ids,
                results_received: 0,
                completed: false,
            },
        );
        debug!("Registered scan job: {} (type={:?})", job_id, scan_type);
    }

    /// Get the scan type for a pending job
    async fn get_job_scan_type(&self, job_id: &str) -> Option<ScanType> {
        let jobs = self.pending_jobs.read().await;
        jobs.jobs.get(job_id).map(|j| j.scan_type.clone())
    }

    /// Start the consumer loop
    ///
    /// This subscribes to the event bus and processes incoming ScanResult messages.
    /// It runs until the event bus is closed or an error occurs.
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting scan result consumer");

        let mut receiver = self.event_bus.subscribe();

        loop {
            match receiver.recv().await {
                Ok(message) => {
                    if let Message::ScanResult {
                        job_id,
                        series_id,
                        worker_id,
                        files_found,
                        errors,
                    } = message
                    {
                        // Check scan type to route to the right processor
                        let scan_type = self.get_job_scan_type(&job_id).await;
                        match scan_type {
                            Some(ScanType::RescanMovie) => {
                                self.handle_movie_scan_result(
                                    &job_id,
                                    series_id, // actually movie_id
                                    &worker_id,
                                    files_found,
                                    errors,
                                )
                                .await;
                            }
                            _ => {
                                // Default: series scan (RescanSeries, DownloadedEpisodesScan, or unknown)
                                self.handle_scan_result(
                                    &job_id,
                                    series_id,
                                    &worker_id,
                                    files_found,
                                    errors,
                                )
                                .await;
                            }
                        }
                    }
                    // Ignore other message types
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Scan result consumer lagged by {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Event bus closed, shutting down scan result consumer");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a series scan result from a worker
    async fn handle_scan_result(
        &self,
        job_id: &str,
        series_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        info!(
            "Received scan result: job_id={}, series_id={}, worker={}, files={}, errors={}",
            job_id,
            series_id,
            worker_id,
            files_found.len(),
            errors.len()
        );

        // Log any errors from the worker
        for error in &errors {
            warn!(
                "Worker {} reported error for job {}: {}",
                worker_id, job_id, error
            );
        }

        // If worker returned 0 files, try local fallback scan (server may have
        // updated parser patterns that the worker doesn't have yet)
        let files_to_process = if files_found.is_empty() {
            self.try_local_fallback_scan(series_id).await
        } else {
            files_found
        };

        // Process the files
        if let Err(e) = self
            .process_scanned_files(series_id, files_to_process)
            .await
        {
            error!(
                "Failed to process scanned files for series {}: {}",
                series_id, e
            );
        }

        // Update job tracking
        self.mark_job_result_received(job_id).await;
    }

    /// Handle a movie scan result from a worker
    async fn handle_movie_scan_result(
        &self,
        job_id: &str,
        movie_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        info!(
            "Received movie scan result: job_id={}, movie_id={}, worker={}, files={}, errors={}",
            job_id,
            movie_id,
            worker_id,
            files_found.len(),
            errors.len()
        );

        for error in &errors {
            warn!(
                "Worker {} reported error for movie job {}: {}",
                worker_id, job_id, error
            );
        }

        if let Some(file) = files_found.into_iter().next() {
            if let Err(e) = self.process_movie_scan_result(movie_id, file).await {
                error!(
                    "Failed to process movie scan result for movie {}: {}",
                    movie_id, e
                );
            }
        } else {
            info!("No video files found for movie {} by worker", movie_id);
            // Clear file reference if movie thought it had one
            let movie_repo = MovieRepository::new(self.db.clone());
            if let Ok(Some(movie)) = movie_repo.get_by_id(movie_id).await {
                if movie.has_file {
                    let pool = self.db.pool();
                    let _ = sqlx::query(
                        "UPDATE movies SET has_file = false, movie_file_id = NULL WHERE id = $1",
                    )
                    .bind(movie_id)
                    .execute(pool)
                    .await;
                }
            }
        }

        self.mark_job_result_received(job_id).await;
    }

    /// Process a single movie file from a worker scan result
    async fn process_movie_scan_result(&self, movie_id: i64, file: ScannedFile) -> Result<()> {
        let movie_repo = MovieRepository::new(self.db.clone());
        let file_repo = MovieFileRepository::new(self.db.clone());

        let movie = movie_repo
            .get_by_id(movie_id)
            .await
            .context("Failed to fetch movie")?
            .ok_or_else(|| anyhow::anyhow!("Movie {} not found", movie_id))?;

        let file_path_str = file.path.to_string_lossy().to_string();

        // Check if file already tracked
        let existing_files = file_repo.get_by_movie_id(movie_id).await?;
        if existing_files.iter().any(|f| f.path == file_path_str) {
            info!(
                "Movie file already tracked for '{}': {}",
                movie.title, file_path_str
            );
            return Ok(());
        }

        let relative_path = file
            .path
            .strip_prefix(&movie.path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.filename.clone());

        // Use worker-provided enrichment or fall back to local analysis
        let (media_info, quality_str, file_hash) = if file.media_info.is_some() {
            // Worker already enriched — use directly
            let quality = file.quality.unwrap_or_else(|| {
                serde_json::to_string(&serde_json::json!({
                    "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                    "revision": {"version": 1, "real": 0, "isRepack": false}
                }))
                .expect("static JSON")
            });
            (file.media_info, quality, file.file_hash)
        } else {
            // No enrichment — run locally (fallback for non-worker scans)
            let file_path = std::path::Path::new(&file_path_str);
            let media_info_result = MediaAnalyzer::analyze(file_path).await;
            let media_info = media_info_result
                .as_ref()
                .ok()
                .and_then(|info| serde_json::to_string(info).ok());
            let quality = match &media_info_result {
                Ok(info) => {
                    serde_json::to_string(&derive_quality_from_media(info, &file.filename))
                        .unwrap_or_default()
                }
                Err(_) => serde_json::to_string(&serde_json::json!({
                    "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                    "revision": {"version": 1, "real": 0, "isRepack": false}
                }))
                .expect("static JSON"),
            };
            let file_hash = compute_file_hash(file_path).await.ok();
            (media_info, quality, file_hash)
        };

        use crate::core::datastore::models::MovieFileDbModel;
        let movie_file = MovieFileDbModel {
            id: 0,
            movie_id,
            relative_path,
            path: file_path_str.clone(),
            size: file.size,
            date_added: Utc::now(),
            scene_name: Some(file.filename.clone()),
            release_group: file.release_group.clone(),
            quality: quality_str,
            languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
            media_info,
            original_file_path: Some(file_path_str.clone()),
            edition: None,
            file_hash,
        };

        match file_repo.insert(&movie_file).await {
            Ok(file_id) => {
                let pool = self.db.pool();
                let _ = sqlx::query(
                    "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                )
                .bind(file_id)
                .bind(movie_id)
                .execute(pool)
                .await;
                info!(
                    "Movie scan: found file for '{}': {} ({} bytes)",
                    movie.title, file_path_str, file.size
                );
            }
            Err(e) => {
                warn!(
                    "Failed to insert movie file for '{}': {}",
                    movie.title, e
                );
            }
        }

        Ok(())
    }

    /// Mark a job result as received in the pending jobs tracker
    async fn mark_job_result_received(&self, job_id: &str) {
        let mut jobs = self.pending_jobs.write().await;
        if let Some(job) = jobs.jobs.get_mut(job_id) {
            job.results_received += 1;
            debug!("Job {} received result {}", job_id, job.results_received);
        }
    }

    /// When the worker returns 0 files, check if the path is accessible locally
    /// and re-scan with the server's (potentially newer) scanner code.
    async fn try_local_fallback_scan(&self, series_id: i64) -> Vec<ScannedFile> {
        let series_repo = SeriesRepository::new(self.db.clone());
        let series = match series_repo.get_by_id(series_id).await {
            Ok(Some(s)) => s,
            _ => return Vec::new(),
        };

        let series_path = std::path::Path::new(&series.path);
        if !series_path.exists() {
            return Vec::new();
        }

        let files = super::scan_series_directory(series_path);
        if !files.is_empty() {
            info!(
                "Local fallback scan found {} files for '{}' (worker returned 0)",
                files.len(),
                series.title
            );
        }
        files
    }

    /// Process scanned files and update the database.
    ///
    /// If the worker already enriched files with media_info/quality/file_hash,
    /// those values are used directly — no local FFmpeg/hash I/O needed.
    async fn process_scanned_files(&self, series_id: i64, files: Vec<ScannedFile>) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }

        let series_repo = SeriesRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());

        // Get series info
        let series = series_repo
            .get_by_id(series_id)
            .await
            .context("Failed to fetch series")?
            .ok_or_else(|| anyhow::anyhow!("Series {} not found", series_id))?;

        // Get episodes for this series
        let episodes = episode_repo
            .get_by_series_id(series_id)
            .await
            .context("Failed to fetch episodes")?;

        let mut new_files = 0;
        let mut matched_episodes = 0;

        for file in files {
            let file_path_str = file.path.to_string_lossy().to_string();

            // Skip if we don't have episode info
            if file.episode_numbers.is_empty() {
                debug!("Skipping file without episode info: {}", file.filename);
                continue;
            }

            let season_number = file.season_number.unwrap_or(1);

            // Check if file already exists in database
            let existing = episode_file_repo
                .get_by_path(&file_path_str)
                .await
                .ok()
                .flatten();

            let episode_file_id = if let Some(ef) = existing {
                ef.id
            } else {
                // Calculate relative path
                let relative_path = file
                    .path
                    .strip_prefix(&series.path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file.filename.clone());

                let languages_json = serde_json::json!([{
                    "id": 1,
                    "name": "English"
                }]);

                // Use worker-provided enrichment or fall back to local analysis
                let (media_info, quality_json, file_hash) = if file.media_info.is_some() {
                    // Worker already enriched — use directly (no local disk I/O)
                    let quality: serde_json::Value = file
                        .quality
                        .as_deref()
                        .and_then(|q| serde_json::from_str(q).ok())
                        .unwrap_or_else(|| {
                            serde_json::json!({
                                "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                                "revision": {"version": 1, "real": 0, "isRepack": false}
                            })
                        });
                    info!(
                        "Using worker-enriched data for {} (hash={})",
                        file.filename,
                        file.file_hash.as_deref().unwrap_or("none")
                    );
                    (file.media_info.clone(), quality, file.file_hash.clone())
                } else {
                    // No enrichment — FFmpeg probe + hash locally (fallback for local scans)
                    let media_info_result =
                        MediaAnalyzer::analyze(std::path::Path::new(&file_path_str)).await;
                    let media_info = media_info_result
                        .as_ref()
                        .ok()
                        .and_then(|info| serde_json::to_string(info).ok());

                    let quality_json = match &media_info_result {
                        Ok(info) => derive_quality_from_media(info, &file.filename),
                        Err(e) => {
                            debug!(
                                "Media probe failed for {}, using SDTV default: {}",
                                file_path_str, e
                            );
                            serde_json::json!({
                                "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                                "revision": {"version": 1, "real": 0, "isRepack": false}
                            })
                        }
                    };

                    let file_hash = compute_file_hash(std::path::Path::new(&file_path_str))
                        .await
                        .ok();

                    (media_info, quality_json, file_hash)
                };

                let episode_file = EpisodeFileDbModel {
                    id: 0,
                    series_id,
                    season_number,
                    relative_path,
                    path: file_path_str.clone(),
                    size: file.size,
                    date_added: Utc::now(),
                    scene_name: Some(file.filename.clone()),
                    release_group: file.release_group.clone(),
                    quality: quality_json.to_string(),
                    languages: languages_json.to_string(),
                    media_info,
                    original_file_path: Some(file_path_str.clone()),
                    file_hash,
                };

                match episode_file_repo.insert(&episode_file).await {
                    Ok(id) => {
                        new_files += 1;
                        debug!("Created episode file: id={}, path={}", id, file_path_str);
                        id
                    }
                    Err(e) => {
                        warn!("Failed to create episode file for {}: {}", file_path_str, e);
                        continue;
                    }
                }
            };

            // Link episodes to file
            for episode_num in &file.episode_numbers {
                if let Some(mut ep) = episodes
                    .iter()
                    .find(|e| e.season_number == season_number && e.episode_number == *episode_num)
                    .cloned()
                {
                    if !ep.has_file || ep.episode_file_id != Some(episode_file_id) {
                        ep.has_file = true;
                        ep.episode_file_id = Some(episode_file_id);
                        if episode_repo.update(&ep).await.is_ok() {
                            matched_episodes += 1;
                            debug!(
                                "Linked S{:02}E{:02} to file {}",
                                season_number, episode_num, episode_file_id
                            );
                        }
                    }
                }
            }

            // Log multi-episode files
            if file.episode_numbers.len() > 1 {
                let ep_list: Vec<String> = file
                    .episode_numbers
                    .iter()
                    .map(|e| format!("E{:02}", e))
                    .collect();
                info!(
                    "Multi-episode file: {} -> S{:02}{}",
                    file.filename,
                    season_number,
                    ep_list.join("")
                );
            }
        }

        info!(
            "Processed scan for series {}: {} new files, {} episodes linked",
            series.title, new_files, matched_episodes
        );

        // Publish series scanned event
        self.event_bus
            .publish(Message::SeriesScanned {
                series_id,
                title: series.title,
                files_found: new_files,
                episodes_matched: matched_episodes,
            })
            .await;

        Ok(())
    }
}

/// Create a scan request message for series
pub fn create_scan_request(series_ids: Vec<i64>, paths: Vec<String>) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanSeries,
        series_ids: series_ids.clone(),
        paths,
    };

    (job_id, message)
}

/// Create a scan request message for movies
pub fn create_movie_scan_request(movie_ids: Vec<i64>, paths: Vec<String>) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanMovie,
        series_ids: movie_ids.clone(), // reused field for movie IDs
        paths,
    };

    (job_id, message)
}
