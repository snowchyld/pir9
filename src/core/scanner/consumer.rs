//! Scan result consumer service
//!
//! This service runs on the server and processes scan results from distributed workers.
//! It updates the database with discovered files and links them to episodes/movies.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::core::configuration::MediaConfig;
use crate::core::datastore::models::{EpisodeDbModel, EpisodeFileDbModel, SeriesDbModel};
use crate::core::datastore::repositories::{
    EpisodeFileRepository, EpisodeRepository, HistoryRepository, MovieFileRepository,
    MovieRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::mediafiles::{compute_file_hash, derive_quality_from_media, MediaAnalyzer};
use crate::core::messaging::{HybridEventBus, ImportFileSpec, Message, ScanType, ScannedFile};

/// Tracks pending scan jobs and their results
#[derive(Debug, Default)]
pub struct PendingScanJobs {
    /// Maps job_id -> pending job state
    jobs: HashMap<String, PendingJob>,
    /// Maps job_id -> pending download import (Phase 2→3→4 tracking)
    download_imports: HashMap<String, PendingDownloadImport>,
    /// Tracks overall progress of per-file download imports (keyed by scan job_id)
    download_job_trackers: HashMap<String, DownloadJobTracker>,
    /// Maps per-file import_job_id → original scan job_id for tracker lookup
    import_to_scan_job: HashMap<String, String>,
}

#[derive(Debug)]
struct PendingJob {
    scan_type: ScanType,
    /// series_ids for RescanSeries, movie_ids for RescanMovie
    entity_ids: Vec<i64>,
    results_received: usize,
    completed: bool,
}

/// Metadata needed to complete a download import after the worker moves files.
/// Stored when the scan result arrives (Phase 2) and consumed when ImportFilesResult
/// arrives (Phase 4).
#[derive(Debug)]
struct PendingDownloadImport {
    /// Download client info for cleanup
    download_id: String,
    download_client_id: i64,
    download_title: String,
    /// Mapping: dest_path → import details for DB insert
    file_mappings: HashMap<PathBuf, ImportMapping>,
}

/// Per-file data needed to insert episode_file records after the worker moves the file
#[derive(Debug, Clone)]
struct ImportMapping {
    series_id: i64,
    season_number: i32,
    episode_ids: Vec<i64>,
    /// Enrichment from Phase 1 scan
    media_info: Option<String>,
    quality: String,
    file_hash: Option<String>,
    release_group: Option<String>,
    scene_name: String,
    source_path: PathBuf,
    /// Parsed quality model JSON for history recording
    parsed_quality_json: String,
}

/// Tracks overall progress of a download import across multiple per-file rounds.
/// Created when the scan is registered and consumed when all files are imported.
#[derive(Debug)]
struct DownloadJobTracker {
    download_id: String,
    download_client_id: i64,
    download_title: String,
    /// Number of ImportFilesRequests dispatched (one per matched file)
    files_dispatched: usize,
    /// Number of ImportFilesResults received back from worker
    files_completed: usize,
    /// Files successfully imported (DB records created)
    files_imported: usize,
    /// Episodes linked to imported files
    episodes_linked: usize,
    /// Set when the worker sends an empty ScanResult (no more files coming)
    scan_finished: bool,
}

/// Info about a download to import, passed from command.rs to the consumer
#[derive(Debug, Clone)]
pub struct DownloadImportInfo {
    pub download_id: String,
    pub download_client_id: i64,
    pub download_client_name: String,
    pub title: String,
    pub output_path: PathBuf,
    pub parsed_info: Option<crate::core::parser::ParsedEpisodeInfo>,
    pub series: Option<SeriesDbModel>,
    pub episodes: Vec<EpisodeDbModel>,
}

/// Service that consumes scan results from workers and updates the database
#[derive(Debug)]
pub struct ScanResultConsumer {
    db: Database,
    event_bus: HybridEventBus,
    pending_jobs: Arc<RwLock<PendingScanJobs>>,
    media_config: MediaConfig,
}

impl ScanResultConsumer {
    /// Create a new scan result consumer
    pub fn new(db: Database, event_bus: HybridEventBus) -> Self {
        Self {
            db,
            event_bus,
            pending_jobs: Arc::new(RwLock::new(PendingScanJobs::default())),
            media_config: MediaConfig::default(),
        }
    }

    /// Set media config for episode naming during download imports
    pub fn set_media_config(&mut self, config: MediaConfig) {
        self.media_config = config;
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

    /// Register a pending download import (called from command.rs when dispatching to worker)
    pub async fn register_download_import(
        &self,
        job_id: &str,
        imports: Vec<DownloadImportInfo>,
    ) {
        let mut jobs = self.pending_jobs.write().await;
        // Store each download import keyed by job_id + index for multi-download batches
        // For now we key by job_id and store one at a time (imports dispatched per-download)
        for import_info in &imports {
            let key = format!("{}:{}", job_id, import_info.download_id);
            jobs.download_imports.insert(
                key,
                PendingDownloadImport {
                    download_id: import_info.download_id.clone(),
                    download_client_id: import_info.download_client_id,
                    download_title: import_info.title.clone(),
                    file_mappings: HashMap::new(),
                },
            );
        }

        // Create a job tracker for per-file import progress
        if let Some(first) = imports.first() {
            jobs.download_job_trackers.insert(
                job_id.to_string(),
                DownloadJobTracker {
                    download_id: first.download_id.clone(),
                    download_client_id: first.download_client_id,
                    download_title: first.title.clone(),
                    files_dispatched: 0,
                    files_completed: 0,
                    files_imported: 0,
                    episodes_linked: 0,
                    scan_finished: false,
                },
            );
        }

        debug!("Registered download imports for scan job: {}", job_id);
    }

    /// Start the consumer loop
    ///
    /// This subscribes to the event bus and processes incoming ScanResult and
    /// ImportFilesResult messages.
    /// It runs until the event bus is closed or an error occurs.
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting scan result consumer");

        let mut receiver = self.event_bus.subscribe();

        loop {
            match receiver.recv().await {
                Ok(message) => {
                    match message {
                        Message::ScanResult {
                            job_id,
                            series_id,
                            worker_id,
                            files_found,
                            errors,
                        } => {
                            // Check scan type to route to the right processor
                            let scan_type = self.get_job_scan_type(&job_id).await;
                            match scan_type {
                                Some(ScanType::RescanMovie) => {
                                    self.handle_movie_scan_result(
                                        &job_id,
                                        series_id,
                                        &worker_id,
                                        files_found,
                                        errors,
                                    )
                                    .await;
                                }
                                Some(ScanType::DownloadedEpisodesScan) => {
                                    self.handle_download_scan_result(
                                        &job_id,
                                        &worker_id,
                                        files_found,
                                        errors,
                                    )
                                    .await;
                                }
                                Some(ScanType::RescanPodcast) => {
                                    info!("Received podcast scan result for job {} — not yet implemented", job_id);
                                    self.mark_job_result_received(&job_id).await;
                                }
                                Some(ScanType::RescanMusic) => {
                                    info!("Received music scan result for job {} — not yet implemented", job_id);
                                    self.mark_job_result_received(&job_id).await;
                                }
                                _ => {
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
                        Message::ImportFilesResult {
                            job_id,
                            worker_id,
                            results,
                        } => {
                            self.handle_import_files_result(&job_id, &worker_id, results)
                                .await;
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }
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

    /// Handle download scan result (Phase 2): match file → episodes, compute path, dispatch move.
    ///
    /// Called once per file in streaming mode (worker sends per-file ScanResults).
    /// When files_found is empty, this signals scan completion for cleanup tracking.
    ///
    /// For each file:
    /// 1. Parse filename to extract season/episode numbers
    /// 2. Match against series/episodes in the database
    /// 3. Compute destination path using the naming engine
    /// 4. Send a per-file ImportFilesRequest to the worker
    /// 5. Store mapping data for Phase 4 (DB insert after move)
    async fn handle_download_scan_result(
        &self,
        job_id: &str,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        for err in &errors {
            warn!("Worker {} download scan error: {}", worker_id, err);
        }

        // Empty result = scan completion signal from worker
        if files_found.is_empty() {
            info!(
                "[worker:{}] Scan complete for job {}",
                worker_id, job_id
            );
            {
                let mut jobs = self.pending_jobs.write().await;
                if let Some(tracker) = jobs.download_job_trackers.get_mut(job_id) {
                    tracker.scan_finished = true;
                }
            }
            self.try_download_cleanup(job_id).await;
            self.mark_job_result_received(job_id).await;
            return;
        }

        info!(
            "[worker:{}] Scan result: job_id={}, {} file(s)",
            worker_id, job_id, files_found.len()
        );

        // Look up which download imports are associated with this job
        let download_keys: Vec<String> = {
            let jobs = self.pending_jobs.read().await;
            jobs.download_imports
                .keys()
                .filter(|k| k.starts_with(&format!("{}:", job_id)))
                .cloned()
                .collect()
        };

        if download_keys.is_empty() {
            warn!(
                "No download import info found for job_id={}, skipping",
                job_id
            );
            return;
        }

        // Get download info from the registered import
        let mut download_title = String::new();
        if let Some(key) = download_keys.first() {
            let jobs = self.pending_jobs.read().await;
            if let Some(pending) = jobs.download_imports.get(key) {
                download_title = pending.download_title.clone();
            }
        }

        // Process each file (typically 1 in per-file streaming mode)
        for file in &files_found {
            let filename = &file.filename;

            // Parse episode info from the filename
            let parsed_eps = crate::core::scanner::parse_episodes_from_filename(filename);
            if parsed_eps.is_empty() {
                debug!("[worker:{}] skipping unmatched file '{}'", worker_id, filename);
                continue;
            }

            // Try to match to a series using the full filename as a parsed title
            let parsed_info = crate::core::parser::parse_title(filename);
            let parsed_info = match parsed_info {
                Some(pi) => pi,
                None => {
                    debug!(
                        "[worker:{}] could not parse title from '{}'",
                        worker_id, filename
                    );
                    continue;
                }
            };

            // Match series
            let series = match crate::core::download::import::match_series_standalone(
                &self.db,
                &parsed_info,
            )
            .await
            {
                Ok(Some(s)) => s,
                _ => {
                    info!(
                        "[worker:{}] no series match for '{}'",
                        worker_id, filename
                    );
                    continue;
                }
            };

            // Match episodes
            let episodes = match crate::core::download::import::match_episodes_standalone(
                &self.db,
                &series,
                &parsed_info,
            )
            .await
            {
                Ok(eps) if !eps.is_empty() => {
                    let ep_list: String = eps
                        .iter()
                        .map(|e| format!("S{:02}E{:02}", e.season_number, e.episode_number))
                        .collect::<Vec<_>>()
                        .join("+");
                    info!(
                        "[worker:{}] '{}' → '{}' {} (parsed {:?})",
                        worker_id, filename, series.title, ep_list, parsed_eps
                    );
                    eps
                }
                _ => {
                    info!(
                        "[worker:{}] '{}' → '{}' — no episode match (parsed {:?})",
                        worker_id, filename, series.title, parsed_eps
                    );
                    continue;
                }
            };

            let season_number = episodes.first().map(|e| e.season_number).unwrap_or(1);

            // Compute destination path (pure computation, no I/O)
            let dest_path = crate::core::download::import::compute_destination_path(
                &self.media_config,
                &series,
                season_number,
                filename,
                &episodes,
                &parsed_info,
            );

            let episode_ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();

            // Use worker-enriched quality or fall back to parsed quality
            let quality_str = file
                .quality
                .clone()
                .unwrap_or_else(|| {
                    serde_json::to_string(&parsed_info.quality).unwrap_or_default()
                });

            // Generate a unique import_job_id for this single file
            let import_job_id = uuid::Uuid::new_v4().to_string();

            // Store per-file mapping for Phase 4
            let mut file_mappings = HashMap::new();
            file_mappings.insert(
                dest_path.clone(),
                ImportMapping {
                    series_id: series.id,
                    season_number,
                    episode_ids,
                    media_info: file.media_info.clone(),
                    quality: quality_str,
                    file_hash: file.file_hash.clone(),
                    release_group: file.release_group.clone(),
                    scene_name: download_title.clone(),
                    source_path: file.path.clone(),
                    parsed_quality_json: serde_json::to_string(&parsed_info.quality)
                        .unwrap_or_default(),
                },
            );

            {
                let mut jobs = self.pending_jobs.write().await;
                // Store per-file import state
                jobs.download_imports.insert(
                    import_job_id.clone(),
                    PendingDownloadImport {
                        download_id: String::new(), // cleanup handled by tracker
                        download_client_id: 0,
                        download_title: download_title.clone(),
                        file_mappings,
                    },
                );
                // Map import_job_id back to scan job for tracker updates
                jobs.import_to_scan_job
                    .insert(import_job_id.clone(), job_id.to_string());
                // Increment dispatched counter
                if let Some(tracker) = jobs.download_job_trackers.get_mut(job_id) {
                    tracker.files_dispatched += 1;
                }
            }

            info!(
                "[worker:{}] '{}': dispatching move {} → {}",
                worker_id,
                filename,
                file.path.display(),
                dest_path.display(),
            );

            // Phase 3: Send per-file ImportFilesRequest to worker immediately
            self.event_bus
                .publish(Message::ImportFilesRequest {
                    job_id: import_job_id,
                    files: vec![ImportFileSpec {
                        source_path: file.path.clone(),
                        dest_path,
                    }],
                })
                .await;
        }
    }

    /// Handle import files result (Phase 4): insert DB records, link episodes.
    ///
    /// Called per-file when the worker confirms a file move is complete. Creates an
    /// EpisodeFileDbModel, links episodes, records history, and updates the download
    /// job tracker. Download cleanup happens when all files are processed.
    async fn handle_import_files_result(
        &self,
        job_id: &str,
        worker_id: &str,
        results: Vec<crate::core::messaging::ImportFileResult>,
    ) {
        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        info!(
            "[worker:{}] File move result: job_id={}, {} ok, {} failed",
            worker_id, job_id, succeeded, failed
        );

        // Look up pending import state for this per-file import
        let pending = {
            let mut jobs = self.pending_jobs.write().await;
            jobs.download_imports.remove(job_id)
        };

        let pending = match pending {
            Some(p) => p,
            None => {
                warn!(
                    "No pending import state found for import job_id={}, skipping DB insert",
                    job_id
                );
                return;
            }
        };

        let episode_file_repo = EpisodeFileRepository::new(self.db.clone());
        let episode_repo = EpisodeRepository::new(self.db.clone());
        let history_repo = HistoryRepository::new(self.db.clone());

        let mut total_imported = 0;
        let mut total_episodes_linked = 0;

        for result in &results {
            if !result.success {
                error!(
                    "Worker failed to move file {} -> {}: {}",
                    result.source_path.display(),
                    result.dest_path.display(),
                    result.error.as_deref().unwrap_or("unknown error")
                );
                continue;
            }

            // Look up the mapping for this dest_path
            let mapping = match pending.file_mappings.get(&result.dest_path) {
                Some(m) => m,
                None => {
                    warn!(
                        "No mapping found for moved file: {}",
                        result.dest_path.display()
                    );
                    continue;
                }
            };

            let dest_path_str = result.dest_path.to_string_lossy().to_string();

            // Compute relative path
            let series_repo = SeriesRepository::new(self.db.clone());
            let series_path = match series_repo.get_by_id(mapping.series_id).await {
                Ok(Some(s)) => s.path,
                _ => String::new(),
            };

            let relative_path = result
                .dest_path
                .strip_prefix(&series_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    result
                        .dest_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                });

            let episode_file = EpisodeFileDbModel {
                id: 0,
                series_id: mapping.series_id,
                season_number: mapping.season_number,
                relative_path,
                path: dest_path_str.clone(),
                size: result.file_size,
                date_added: Utc::now(),
                scene_name: Some(mapping.scene_name.clone()),
                release_group: mapping.release_group.clone(),
                quality: mapping.quality.clone(),
                languages: r#"[{"id":1,"name":"English"}]"#.to_string(),
                media_info: mapping.media_info.clone(),
                original_file_path: Some(mapping.source_path.to_string_lossy().to_string()),
                file_hash: mapping.file_hash.clone(),
            };

            match episode_file_repo.insert(&episode_file).await {
                Ok(file_id) => {
                    total_imported += 1;

                    // Link episodes to the file
                    for episode_id in &mapping.episode_ids {
                        if let Ok(Some(mut ep)) = episode_repo.get_by_id(*episode_id).await {
                            ep.has_file = true;
                            ep.episode_file_id = Some(file_id);
                            if episode_repo.update(&ep).await.is_ok() {
                                total_episodes_linked += 1;
                            }
                        }
                    }

                    // Record history
                    let download_id = {
                        let jobs = self.pending_jobs.read().await;
                        jobs.import_to_scan_job
                            .get(job_id)
                            .and_then(|scan_id| jobs.download_job_trackers.get(scan_id))
                            .map(|t| t.download_id.clone())
                            .unwrap_or_default()
                    };

                    for episode_id in &mapping.episode_ids {
                        let history = crate::core::datastore::models::HistoryDbModel {
                            id: 0,
                            series_id: Some(mapping.series_id),
                            episode_id: Some(*episode_id),
                            movie_id: None,
                            source_title: mapping.scene_name.clone(),
                            quality: mapping.parsed_quality_json.clone(),
                            languages: "[]".to_string(),
                            custom_formats: "[]".to_string(),
                            custom_format_score: 0,
                            quality_cutoff_not_met: false,
                            date: Utc::now(),
                            download_id: Some(download_id.clone()),
                            event_type: 3, // DownloadImported
                            data: "{}".to_string(),
                        };
                        let _ = history_repo.insert(&history).await;
                    }

                    info!(
                        "[worker:{}] Imported {} (file_id={}, {} episodes)",
                        worker_id,
                        dest_path_str,
                        file_id,
                        mapping.episode_ids.len()
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to insert episode file record for {}: {}",
                        dest_path_str, e
                    );
                }
            }
        }

        // Update the download job tracker with per-file results
        let scan_job_id = {
            let mut jobs = self.pending_jobs.write().await;
            let scan_job_id = jobs.import_to_scan_job.remove(job_id).clone();
            if let Some(ref sjid) = scan_job_id {
                if let Some(tracker) = jobs.download_job_trackers.get_mut(sjid.as_str()) {
                    tracker.files_completed += 1;
                    tracker.files_imported += total_imported;
                    tracker.episodes_linked += total_episodes_linked;
                }
            }
            scan_job_id
        };

        // Check if all files are done and we can finalize
        if let Some(sjid) = scan_job_id {
            self.try_download_cleanup(&sjid).await;
        }
    }

    /// Check if a download import is fully complete (all files processed + scan finished).
    /// If so, clean up the download from the client and emit final log.
    async fn try_download_cleanup(&self, scan_job_id: &str) {
        let should_cleanup = {
            let jobs = self.pending_jobs.read().await;
            if let Some(tracker) = jobs.download_job_trackers.get(scan_job_id) {
                tracker.scan_finished
                    && tracker.files_completed == tracker.files_dispatched
                    && tracker.files_dispatched > 0
            } else {
                false
            }
        };

        if !should_cleanup {
            return;
        }

        // Remove tracker and perform cleanup
        let tracker = {
            let mut jobs = self.pending_jobs.write().await;
            jobs.download_job_trackers.remove(scan_job_id)
        };

        let tracker = match tracker {
            Some(t) => t,
            None => return,
        };

        info!(
            "Download import complete '{}': {} files imported, {} episodes linked",
            tracker.download_title, tracker.files_imported, tracker.episodes_linked
        );

        // Clean up download from download client
        if tracker.files_imported > 0 && tracker.download_client_id > 0 {
            use crate::core::datastore::repositories::DownloadClientRepository;
            use crate::core::download::clients::create_client_from_model;

            let client_repo = DownloadClientRepository::new(self.db.clone());
            if let Ok(Some(client_model)) =
                client_repo.get_by_id(tracker.download_client_id).await
            {
                if let Ok(client) = create_client_from_model(&client_model) {
                    match client.remove(&tracker.download_id, false).await {
                        Ok(()) => {
                            info!(
                                "Download import: cleaned up '{}' from {}",
                                tracker.download_title, client_model.name
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Download import: cleanup failed for '{}': {}",
                                tracker.download_title, e
                            );
                        }
                    }
                }
            }
        }

        crate::core::logging::log_info(
            "DownloadImported",
            &format!(
                "Worker import '{}': {} files imported, {} episodes linked",
                tracker.download_title, tracker.files_imported, tracker.episodes_linked
            ),
        )
        .await;
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

/// Create a scan request for podcast libraries (stub — not yet implemented)
pub fn create_podcast_scan_request(
    podcast_ids: Vec<i64>,
    paths: Vec<String>,
) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanPodcast,
        series_ids: podcast_ids,
        paths,
    };

    (job_id, message)
}

/// Create a scan request for music libraries (stub — not yet implemented)
pub fn create_music_scan_request(music_ids: Vec<i64>, paths: Vec<String>) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanMusic,
        series_ids: music_ids,
        paths,
    };

    (job_id, message)
}
