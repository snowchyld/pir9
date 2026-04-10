//! Scan result consumer service
//!
//! This service runs on the server and processes scan results from distributed workers.
//! It updates the database with discovered files and links them to episodes/movies.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
use crate::core::messaging::{
    EventBus, HybridEventBus, ImportFileResult, ImportFileSpec, Message, ScanType, ScannedFile,
};

/// Tracks pending scan jobs and their results
#[derive(Debug, Default)]
pub struct PendingScanJobs {
    /// Maps job_id -> pending job state
    jobs: HashMap<String, PendingJob>,
    /// Maps job_id -> pending download import (Phase 2→3→4 tracking)
    download_imports: HashMap<String, PendingDownloadImport>,
    /// Maps per-file import_job_id -> pending movie file import (Phase 2→3→4 for movies)
    movie_file_imports: HashMap<String, PendingMovieFileImport>,
    /// Tracks overall progress of per-file download imports (keyed by scan job_id)
    download_job_trackers: HashMap<String, DownloadJobTracker>,
    /// Maps per-file import_job_id → original scan job_id for tracker lookup
    import_to_scan_job: HashMap<String, String>,
    /// Tracks which (job_id, entity_id) pairs have received at least one file
    /// via streaming. Used to distinguish "worker found 0 files" (needs fallback)
    /// from "streaming completion signal after files were already sent".
    streamed_entities: HashSet<(String, i64)>,
    /// Per-file enrichment jobs: maps probe/hash job_id → file enrichment state.
    /// When both probe and hash results arrive, the file is ready for processing.
    file_jobs: HashMap<String, PendingFileJob>,
    /// Maps probe_job_id → file_key and hash_job_id → file_key for lookup
    enrichment_job_to_file: HashMap<String, String>,
    /// Pending rename jobs: maps job_id → episode_file_ids (for DB update on result)
    pending_renames: HashMap<String, Vec<i64>>,
}

#[derive(Debug)]
struct PendingJob {
    scan_type: ScanType,
    /// series_ids for RescanSeries, movie_ids for RescanMovie
    entity_ids: Vec<i64>,
    results_received: usize,
    completed: bool,
    /// Which worker is handling this job (set on first ScanResult)
    worker_id: Option<String>,
    /// When the job was registered
    started_at: chrono::DateTime<chrono::Utc>,
    /// Live progress from worker enrichment pipeline
    progress: Option<ScanProgressInfo>,
    /// Download ID from the download client (set for queue imports so the
    /// queue API can match progress to the right queue item)
    download_id: Option<String>,
    /// Total per-file enrichment jobs dispatched (probe+hash pairs count as 1 file)
    enrichment_total: usize,
    /// Per-file enrichments completed (both probe and hash done)
    enrichment_completed: usize,
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
    /// Series ID from the download tracker (known at dispatch time)
    series_id: Option<i64>,
    /// Mapping: dest_path → import details for DB insert
    file_mappings: HashMap<PathBuf, ImportMapping>,
    /// Manual episode overrides from import preview UI: source_file → [(season, episode)]
    overrides: HashMap<String, Vec<(i32, i32)>>,
    /// Pre-resolved episodes from the import handler (season, episode) pairs.
    /// Used as fallback when filename has no S##E## pattern and no overrides are provided.
    pre_resolved_episodes: Vec<(i32, i32)>,
    /// Source file paths to force-reimport even if identical (same size as existing)
    force_reimport: std::collections::HashSet<String>,
    /// Source file paths to skip during import (user chose "Do not import")
    skip_files: std::collections::HashSet<String>,
    /// When true, bypass the same-size skip entirely (user-initiated imports)
    force_import_all: bool,
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

/// Per-file data needed to insert movie_file records after the worker moves the file.
/// Stored in Phase 2 (scan result) and consumed in Phase 4 (import result).
#[derive(Debug, Clone)]
struct PendingMovieFileImport {
    movie_id: i64,
    movie_path: String,
    movie_title: String,
    media_info: Option<String>,
    quality: String,
    file_hash: Option<String>,
    release_group: Option<String>,
    scene_name: String,
    source_path: PathBuf,
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

/// Tracks a single file waiting for probe + hash enrichment results.
/// Created when the server receives an unenriched file from discovery and
/// dispatches individual ProbeFileRequest + HashFileRequest to workers.
#[derive(Debug, Clone)]
struct PendingFileJob {
    /// Parent scan job ID (for completion tracking)
    parent_job_id: String,
    /// Entity ID (series_id or movie_id)
    entity_id: i64,
    /// Scan type (for routing to the right processor)
    scan_type: ScanType,
    /// The discovered file (unenriched)
    file: ScannedFile,
    /// Probe job ID dispatched for this file
    probe_job_id: String,
    /// Hash job ID dispatched for this file
    hash_job_id: String,
    /// Probe result (None = not yet received)
    probe_result: Option<(Option<String>, Option<String>)>,
    /// Hash result (None = not yet received)
    hash_result: Option<Option<String>>,
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
    /// Manual episode overrides from import preview UI: source_file → [(season, episode)]
    pub overrides: std::collections::HashMap<String, Vec<(i32, i32)>>,
    /// Source file paths to force-reimport even if identical (same size as existing)
    pub force_reimport: std::collections::HashSet<String>,
    /// Source file paths to skip during import (user chose "Do not import")
    pub skip_files: std::collections::HashSet<String>,
    /// When true, bypass the same-size skip entirely (user-initiated imports)
    pub force_import_all: bool,
}

/// Progress info from a worker's scan enrichment pipeline
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressInfo {
    /// Current stage: "scanning", "probing", "hashing", "copying"
    pub stage: String,
    /// File currently being processed
    pub current_file: Option<String>,
    /// Total number of files to process
    pub files_total: usize,
    /// Number of files fully processed so far
    pub files_processed: usize,
    /// Overall percent complete (0.0-100.0, supports 0.1% granularity)
    pub percent: f32,
    /// Detail string: "1080p x265 HDR10" or "unchanged"
    pub detail: Option<String>,
    /// Bytes copied so far (only during "copying" stage)
    pub bytes_copied: Option<u64>,
    /// Total bytes to copy (only during "copying" stage)
    pub bytes_total: Option<u64>,
}

/// Info about a currently running scan job, exposed to the API layer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningJobInfo {
    pub job_id: String,
    pub scan_type: ScanType,
    pub entity_ids: Vec<i64>,
    pub results_received: usize,
    pub worker_id: Option<String>,
    pub started_at: Option<String>,
    pub progress: Option<ScanProgressInfo>,
}

/// Service that consumes scan results from workers and updates the database
#[derive(Debug)]
pub struct ScanResultConsumer {
    db: Database,
    event_bus: HybridEventBus,
    /// WebSocket event bus — when set, scan progress is forwarded here so
    /// the frontend receives real-time updates via the WebSocket channel.
    ws_event_bus: Option<EventBus>,
    pending_jobs: Arc<RwLock<PendingScanJobs>>,
    media_config: MediaConfig,
    /// Tracked downloads stores for removing records after import
    tracked: Option<Arc<crate::core::queue::TrackedDownloads>>,
}

impl ScanResultConsumer {
    /// Create a new scan result consumer
    pub fn new(db: Database, event_bus: HybridEventBus) -> Self {
        Self {
            db,
            event_bus,
            ws_event_bus: None,
            pending_jobs: Arc::new(RwLock::new(PendingScanJobs::default())),
            media_config: MediaConfig::default(),
            tracked: None,
        }
    }

    /// Set media config for episode naming during download imports
    pub fn set_media_config(&mut self, config: MediaConfig) {
        self.media_config = config;
    }

    /// Set the WebSocket event bus for forwarding scan progress to frontend clients
    pub fn set_ws_event_bus(&mut self, bus: EventBus) {
        self.ws_event_bus = Some(bus);
    }

    /// Set the tracked downloads store for removing records after import
    pub fn set_tracked(&mut self, tracked: Arc<crate::core::queue::TrackedDownloads>) {
        self.tracked = Some(tracked);
    }

    /// Get info about currently running scan jobs (for the UI).
    /// Also cleans up completed jobs to prevent unbounded HashMap growth.
    pub async fn get_running_jobs(&self) -> Vec<RunningJobInfo> {
        let mut jobs = self.pending_jobs.write().await;
        // Clean up completed jobs
        jobs.jobs.retain(|_, job| !job.completed);
        jobs.jobs
            .iter()
            .map(|(id, job)| RunningJobInfo {
                job_id: id.clone(),
                scan_type: job.scan_type,
                entity_ids: job.entity_ids.clone(),
                results_received: job.results_received,
                worker_id: job.worker_id.clone(),
                started_at: Some(job.started_at.to_rfc3339()),
                progress: job.progress.clone(),
            })
            .collect()
    }

    /// Get import progress for all active download imports, keyed by download_id.
    /// Used by the queue API to show real-time import status on the frontend.
    pub async fn get_import_progress_by_download_id(&self) -> HashMap<String, ScanProgressInfo> {
        let jobs = self.pending_jobs.read().await;
        let mut result = HashMap::new();

        for (job_id, job) in &jobs.jobs {
            if job.completed {
                continue;
            }
            if let Some(ref progress) = job.progress {
                // First check if the job itself has a download_id (set via set_job_download_id)
                if let Some(ref dl_id) = job.download_id {
                    result.insert(dl_id.clone(), progress.clone());
                }
                // Also check if this job has a download tracker (series download imports)
                else if let Some(tracker) = jobs.download_job_trackers.get(job_id) {
                    result.insert(tracker.download_id.clone(), progress.clone());
                }
            }
        }

        result
    }

    /// Cancel a running scan job. Marks it as completed so incoming results are ignored.
    pub async fn cancel_job(&self, job_id: &str) -> bool {
        let mut jobs = self.pending_jobs.write().await;
        if let Some(job) = jobs.jobs.get_mut(job_id) {
            job.completed = true;
            info!("Cancelled scan job: {} (type={:?})", job_id, job.scan_type);
            true
        } else {
            false
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
                worker_id: None,
                started_at: Utc::now(),
                progress: None,
                download_id: None,
                enrichment_total: 0,
                enrichment_completed: 0,
            },
        );
        debug!("Registered scan job: {} (type={:?})", job_id, scan_type);
    }

    /// Associate a download_id with a pending job so the queue API can
    /// map import progress to the correct queue item.
    pub async fn set_job_download_id(&self, job_id: &str, download_id: &str) {
        let mut jobs = self.pending_jobs.write().await;
        if let Some(job) = jobs.jobs.get_mut(job_id) {
            job.download_id = Some(download_id.to_string());
        }
    }

    /// Get the scan type for a pending job
    async fn get_job_scan_type(&self, job_id: &str) -> Option<ScanType> {
        let jobs = self.pending_jobs.read().await;
        jobs.jobs.get(job_id).map(|j| j.scan_type.clone())
    }

    /// Register a pending download import (called from command.rs when dispatching to worker)
    pub async fn register_download_import(&self, job_id: &str, imports: Vec<DownloadImportInfo>) {
        let mut jobs = self.pending_jobs.write().await;
        // Store each download import keyed by job_id + index for multi-download batches
        // For now we key by job_id and store one at a time (imports dispatched per-download)
        for import_info in &imports {
            let key = format!("{}:{}", job_id, import_info.download_id);
            let pre_resolved: Vec<(i32, i32)> = import_info
                .episodes
                .iter()
                .map(|ep| (ep.season_number, ep.episode_number))
                .collect();
            jobs.download_imports.insert(
                key,
                PendingDownloadImport {
                    download_id: import_info.download_id.clone(),
                    download_client_id: import_info.download_client_id,
                    download_title: import_info.title.clone(),
                    series_id: import_info.series.as_ref().map(|s| s.id),
                    file_mappings: HashMap::new(),
                    overrides: import_info.overrides.clone(),
                    pre_resolved_episodes: pre_resolved,
                    force_reimport: import_info.force_reimport.clone(),
                    skip_files: import_info.skip_files.clone(),
                    force_import_all: import_info.force_import_all,
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

    /// Register a movie download import for the worker pipeline.
    /// Creates a download job tracker so the consumer knows to compute a destination
    /// path, dispatch a file move, and create movie_file records when results arrive.
    pub async fn register_movie_download_import(
        &self,
        job_id: &str,
        movie_id: i64,
        movie_title: String,
        download_id: String,
        download_client_id: i64,
        download_title: String,
    ) {
        let mut jobs = self.pending_jobs.write().await;
        jobs.download_job_trackers.insert(
            job_id.to_string(),
            DownloadJobTracker {
                download_id,
                download_client_id,
                download_title: download_title.clone(),
                files_dispatched: 0,
                files_completed: 0,
                files_imported: 0,
                episodes_linked: 0,
                scan_finished: false,
            },
        );
        debug!(
            "Registered movie download import for scan job: {} (movie_id={}, title='{}')",
            job_id, movie_id, movie_title
        );
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
                            // Track which worker is handling this job
                            self.set_job_worker_id(&job_id, &worker_id).await;

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
                                Some(ScanType::DownloadedMovieScan) => {
                                    self.handle_movie_download_scan_result(
                                        &job_id,
                                        series_id, // movie_id reused in series_id field
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
                                Some(ScanType::RescanAudiobook) => {
                                    info!("Received audiobook scan result for job {} — not yet implemented", job_id);
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
                        Message::ScanProgress {
                            job_id,
                            worker_id,
                            stage,
                            current_file,
                            files_total,
                            files_processed,
                            percent,
                            detail,
                            bytes_copied,
                            bytes_total,
                            ..
                        } => {
                            // Look up entity context from pending job
                            let (entity_ids, job_scan_type) = {
                                let mut jobs = self.pending_jobs.write().await;
                                let ctx = if let Some(job) = jobs.jobs.get_mut(&job_id) {
                                    if !job.completed {
                                        if job.worker_id.is_none() {
                                            job.worker_id = Some(worker_id.clone());
                                        }
                                        job.progress = Some(ScanProgressInfo {
                                            stage: stage.clone(),
                                            current_file: current_file.clone(),
                                            files_total,
                                            files_processed,
                                            percent,
                                            detail: detail.clone(),
                                            bytes_copied,
                                            bytes_total,
                                        });
                                    }
                                    (job.entity_ids.clone(), Some(job.scan_type))
                                } else {
                                    (vec![], None)
                                };
                                ctx
                            };

                            // Forward enriched progress to WebSocket
                            if let Some(ref ws_bus) = self.ws_event_bus {
                                ws_bus
                                    .publish(Message::ScanProgress {
                                        job_id,
                                        worker_id,
                                        stage,
                                        current_file,
                                        files_total,
                                        files_processed,
                                        percent,
                                        detail,
                                        entity_ids,
                                        scan_type: job_scan_type,
                                        bytes_copied,
                                        bytes_total,
                                    })
                                    .await;
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
                        Message::ProbeFileResult {
                            job_id,
                            parent_job_id: _,
                            file_path,
                            entity_id,
                            worker_id,
                            media_info,
                            quality,
                        } => {
                            self.handle_probe_file_result(
                                &job_id, &file_path, entity_id, &worker_id, media_info, quality,
                            )
                            .await;
                        }
                        Message::HashFileResult {
                            job_id,
                            parent_job_id: _,
                            file_path,
                            entity_id,
                            worker_id,
                            file_hash,
                        } => {
                            self.handle_hash_file_result(
                                &job_id, &file_path, entity_id, &worker_id, file_hash,
                            )
                            .await;
                        }
                        Message::RenameFilesResult {
                            job_id,
                            worker_id,
                            results,
                        } => {
                            self.handle_rename_files_result(&job_id, &worker_id, results)
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

    /// Handle a series scan result from discovery phase.
    ///
    /// The worker returns a list of unenriched files (no media_info, quality, or hash).
    /// For each file:
    /// - Check known DB records: if path+size match and hash exists, process immediately
    /// - Otherwise: dispatch individual ProbeFileRequest + HashFileRequest jobs
    ///
    /// Empty files_found = worker found 0 files for this entity. Try local fallback
    /// and mark the entity result as received.
    async fn handle_scan_result(
        &self,
        job_id: &str,
        series_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        for error in &errors {
            warn!(
                "Worker {} reported error for job {}: {}",
                worker_id, job_id, error
            );
        }

        if files_found.is_empty() {
            // No files found — try local fallback, then mark received
            info!(
                "Discovery: 0 files for series {} (job={})",
                series_id, job_id
            );
            let fallback = self.try_local_fallback_scan(series_id).await;
            if let Err(e) = self.process_scanned_files(series_id, fallback).await {
                error!(
                    "Failed to process fallback scan for series {}: {}",
                    series_id, e
                );
            }
            self.mark_job_result_received(job_id).await;
            return;
        }

        info!(
            "Discovery: {} files for series {} (job={}), dispatching enrichment",
            files_found.len(),
            series_id,
            job_id
        );

        // Load known file data from DB for skip-enrichment
        let known_files = self.load_known_series_files(series_id).await;

        let mut enriched_files = Vec::new();
        let mut files_needing_enrichment = Vec::new();

        for file in files_found {
            let path_key = file.path.to_string_lossy().to_string();
            if let Some(known) = known_files.get(&path_key) {
                if known.size == file.size && known.file_hash.is_some() {
                    // File unchanged — use existing enrichment data
                    let mut enriched = file;
                    enriched.media_info = known.media_info.clone();
                    enriched.quality = known.quality.clone();
                    enriched.file_hash = known.file_hash.clone();
                    enriched_files.push(enriched);
                    continue;
                }
            }
            files_needing_enrichment.push(file);
        }

        // Process already-enriched files immediately
        if !enriched_files.is_empty() {
            let count = enriched_files.len();
            debug!(
                "Processing {} unchanged files for series {}",
                count, series_id
            );
            if let Err(e) = self.process_scanned_files(series_id, enriched_files).await {
                error!(
                    "Failed to process enriched files for series {}: {}",
                    series_id, e
                );
            }
        }

        if files_needing_enrichment.is_empty() {
            // All files were known — done with this entity
            self.mark_job_result_received(job_id).await;
            return;
        }

        // Dispatch individual probe + hash jobs for files needing enrichment
        self.dispatch_enrichment_jobs(
            job_id,
            series_id,
            ScanType::RescanSeries,
            files_needing_enrichment,
        )
        .await;
    }

    /// Handle a movie scan result from discovery phase.
    ///
    /// Same pattern as series: check known files, dispatch enrichment for new ones.
    async fn handle_movie_scan_result(
        &self,
        job_id: &str,
        movie_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        for error in &errors {
            warn!(
                "Worker {} reported error for movie job {}: {}",
                worker_id, job_id, error
            );
        }

        if files_found.is_empty() {
            // No file found — clear has_file if movie thought it had one
            info!("Discovery: 0 files for movie {} (job={})", movie_id, job_id);
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
            self.mark_job_result_received(job_id).await;
            return;
        }

        info!(
            "Discovery: {} files for movie {} (job={}), dispatching enrichment",
            files_found.len(),
            movie_id,
            job_id
        );

        // Load known file data from DB for skip-enrichment
        let known_files = self.load_known_movie_files(movie_id).await;

        let mut enriched_files = Vec::new();
        let mut files_needing_enrichment = Vec::new();

        for file in files_found {
            let path_key = file.path.to_string_lossy().to_string();
            if let Some(known) = known_files.get(&path_key) {
                if known.size == file.size && known.file_hash.is_some() {
                    let mut enriched = file;
                    enriched.media_info = known.media_info.clone();
                    enriched.quality = known.quality.clone();
                    enriched.file_hash = known.file_hash.clone();
                    enriched_files.push(enriched);
                    continue;
                }
            }
            files_needing_enrichment.push(file);
        }

        // Process already-enriched files immediately
        for file in enriched_files {
            if let Err(e) = self.process_movie_scan_result(movie_id, file).await {
                error!(
                    "Failed to process enriched movie file for movie {}: {}",
                    movie_id, e
                );
            }
        }

        if files_needing_enrichment.is_empty() {
            self.mark_job_result_received(job_id).await;
            return;
        }

        // Dispatch individual probe + hash jobs
        self.dispatch_enrichment_jobs(
            job_id,
            movie_id,
            ScanType::RescanMovie,
            files_needing_enrichment,
        )
        .await;
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
        if let Some(existing) = existing_files.iter().find(|f| f.path == file_path_str) {
            // File already tracked — ensure movie record is linked
            if !movie.has_file || movie.movie_file_id != Some(existing.id) {
                let pool = self.db.pool();
                let _ = sqlx::query(
                    "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                )
                .bind(existing.id)
                .bind(movie_id)
                .execute(pool)
                .await;
                info!(
                    "Movie file already tracked for '{}', linked has_file: {}",
                    movie.title, file_path_str
                );
            }
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
                Ok(info) => serde_json::to_string(&derive_quality_from_media(info, &file.filename))
                    .unwrap_or_default(),
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
            quality: quality_str.into(),
            languages: r#"[{"id":1,"name":"English"}]"#.to_string().into(),
            media_info: media_info.map(Into::into),
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

                // Publish per-file event so the frontend updates immediately
                self.event_bus
                    .publish(Message::MovieFileImported {
                        movie_file_id: file_id,
                        movie_id,
                    })
                    .await;
            }
            Err(e) => {
                warn!("Failed to insert movie file for '{}': {}", movie.title, e);
            }
        }

        Ok(())
    }

    /// Handle movie download scan result (Phase 2): compute destination, dispatch file move.
    ///
    /// Called per-file in streaming mode. The worker probed + hashed the file in the
    /// download directory. We compute the destination in the movie's library folder and
    /// send an ImportFilesRequest so the worker moves it. Phase 4 (handle_import_files_result)
    /// creates the movie_file DB record once the move completes.
    async fn handle_movie_download_scan_result(
        &self,
        job_id: &str,
        movie_id: i64,
        worker_id: &str,
        files_found: Vec<ScannedFile>,
        errors: Vec<String>,
    ) {
        for err in &errors {
            warn!("Worker {} movie download scan error: {}", worker_id, err);
        }

        // Empty result = scan completion signal from worker
        if files_found.is_empty() {
            info!(
                "[worker:{}] Movie download scan complete for job {}",
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

        // Take the largest video file (movies are single-file)
        let file = match files_found.into_iter().max_by_key(|f| f.size) {
            Some(f) => f,
            None => return,
        };

        // Look up the movie for its library path
        let movie_repo = MovieRepository::new(self.db.clone());
        let movie = match movie_repo.get_by_id(movie_id).await {
            Ok(Some(m)) => m,
            _ => {
                error!(
                    "Movie {} not found for download import job {}",
                    movie_id, job_id
                );
                self.mark_job_result_received(job_id).await;
                return;
            }
        };

        // Compute destination: movie library path + original filename
        let dest_path = PathBuf::from(&movie.path).join(&file.filename);

        // Get download title from the job tracker
        let download_title = {
            let jobs = self.pending_jobs.read().await;
            jobs.download_job_trackers
                .get(job_id)
                .map(|t| t.download_title.clone())
                .unwrap_or_default()
        };

        let quality_str = file.quality.clone().unwrap_or_else(|| {
            serde_json::to_string(&serde_json::json!({
                "quality": {"id": 1, "name": "SDTV", "source": "unknown", "resolution": 0},
                "revision": {"version": 1, "real": 0, "isRepack": false}
            }))
            .expect("static JSON")
        });

        let import_job_id = uuid::Uuid::new_v4().to_string();

        // Store pending movie import for Phase 4
        {
            let mut jobs = self.pending_jobs.write().await;
            jobs.movie_file_imports.insert(
                import_job_id.clone(),
                PendingMovieFileImport {
                    movie_id,
                    movie_path: movie.path.clone(),
                    movie_title: movie.title.clone(),
                    media_info: file.media_info.clone(),
                    quality: quality_str,
                    file_hash: file.file_hash.clone(),
                    release_group: file.release_group.clone(),
                    scene_name: download_title,
                    source_path: file.path.clone(),
                },
            );
            jobs.import_to_scan_job
                .insert(import_job_id.clone(), job_id.to_string());
            if let Some(tracker) = jobs.download_job_trackers.get_mut(job_id) {
                tracker.files_dispatched += 1;
            }
        }

        info!(
            "[worker:{}] Movie '{}': dispatching move {} → {}",
            worker_id,
            movie.title,
            file.path.display(),
            dest_path.display(),
        );

        // Phase 3: dispatch file move to worker (durable queue)
        self.event_bus
            .enqueue_job(Message::ImportFilesRequest {
                job_id: import_job_id,
                files: vec![ImportFileSpec {
                    source_path: file.path.clone(),
                    dest_path,
                }],
            })
            .await;

        // Discovery complete — mark scan finished so cleanup fires after import
        {
            let mut jobs = self.pending_jobs.write().await;
            if let Some(tracker) = jobs.download_job_trackers.get_mut(job_id) {
                tracker.scan_finished = true;
            }
        }
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
            info!("[worker:{}] Scan complete for job {}", worker_id, job_id);
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
            worker_id,
            job_id,
            files_found.len()
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
            self.mark_job_result_received(job_id).await;
            return;
        }

        // Get download info from the registered import
        let mut download_title = String::new();
        let file_overrides: HashMap<String, Vec<(i32, i32)>>;
        let known_series_id: Option<i64>;
        let pre_resolved_episodes: Vec<(i32, i32)>;
        let force_reimport: std::collections::HashSet<String>;
        let skip_files: std::collections::HashSet<String>;
        let force_import_all: bool;
        if let Some(key) = download_keys.first() {
            let jobs = self.pending_jobs.read().await;
            if let Some(pending) = jobs.download_imports.get(key) {
                download_title = pending.download_title.clone();
                file_overrides = pending.overrides.clone();
                known_series_id = pending.series_id;
                pre_resolved_episodes = pending.pre_resolved_episodes.clone();
                force_reimport = pending.force_reimport.clone();
                skip_files = pending.skip_files.clone();
                force_import_all = pending.force_import_all;
            } else {
                file_overrides = HashMap::new();
                known_series_id = None;
                pre_resolved_episodes = Vec::new();
                force_reimport = std::collections::HashSet::new();
                skip_files = std::collections::HashSet::new();
                force_import_all = false;
            }
        } else {
            file_overrides = HashMap::new();
            known_series_id = None;
            pre_resolved_episodes = Vec::new();
            force_reimport = std::collections::HashSet::new();
            skip_files = std::collections::HashSet::new();
            force_import_all = false;
        }

        // Process each file (typically 1 in per-file streaming mode)
        for file in &files_found {
            let filename = &file.filename;

            // User explicitly chose "Do not import" for this file
            if skip_files.contains(filename) || skip_files.iter().any(|p| p.ends_with(filename)) {
                info!(
                    "[worker:{}] skipping '{}' — user chose Do not import",
                    worker_id, filename,
                );
                continue;
            }

            // Parse episode info from the filename, falling back to manual overrides
            let mut parsed_eps = crate::core::scanner::parse_episodes_from_filename(filename);
            if parsed_eps.is_empty() {
                // Check manual overrides from import preview UI.
                // Override keys come from the download client file list (relative paths
                // like "folder/file.mkv") while filename is the basename only ("file.mkv").
                // Try exact match first, then match by basename of the override key.
                let override_match = file_overrides.get(filename).or_else(|| {
                    file_overrides.iter().find_map(|(key, pairs)| {
                        let key_basename = std::path::Path::new(key)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(key);
                        if key_basename == filename {
                            Some(pairs)
                        } else {
                            None
                        }
                    })
                });
                if let Some(pairs) = override_match {
                    parsed_eps = pairs.clone();
                    info!(
                        "[worker:{}] manual override for '{}' → {:?}",
                        worker_id, filename, pairs
                    );
                } else if !pre_resolved_episodes.is_empty() {
                    // Fallback: use pre-resolved episodes from the import handler.
                    // This covers files without S##E## patterns (e.g., movie-style names)
                    // where the user previously assigned episodes via the import preview UI.
                    parsed_eps = pre_resolved_episodes.clone();
                    info!(
                        "[worker:{}] using pre-resolved episodes for '{}' → {:?}",
                        worker_id, filename, pre_resolved_episodes
                    );
                } else {
                    debug!(
                        "[worker:{}] skipping unmatched file '{}'",
                        worker_id, filename
                    );
                    continue;
                }
            }

            // Try to match to a series and episodes.
            // Priority 1: use known_series_id from the tracked download (most reliable —
            //   the download was already matched to a series at import-queue time)
            // Priority 2: parse filename → match series → match episodes (for untracked imports)
            // Priority 3: skip
            //
            // known_series_id is preferred because individual files inside a torrent
            // may have localized or alternate titles (e.g., Italian dub titles) that
            // don't match the series title in the library.
            let parsed_info = crate::core::parser::parse_title(filename);

            let (series, episodes, parsed_info) = if let Some(sid) = known_series_id {
                // Best path: we know the series from the tracked download
                let series_repo = SeriesRepository::new(self.db.clone());
                let s = match series_repo.get_by_id(sid).await {
                    Ok(Some(s)) => s,
                    _ => {
                        debug!(
                            "[worker:{}] could not look up known series_id={} for '{}'",
                            worker_id, sid, filename
                        );
                        continue;
                    }
                };

                // Look up episodes by parsed season/episode numbers
                let episode_repo = EpisodeRepository::new(self.db.clone());
                let mut eps = Vec::new();
                for &(season, episode) in &parsed_eps {
                    if let Ok(Some(ep)) = episode_repo
                        .get_by_series_season_episode(sid, season, episode)
                        .await
                    {
                        eps.push(ep);
                    }
                }

                if eps.is_empty() {
                    info!(
                        "[worker:{}] '{}' → '{}' — no episode match for S/E {:?}",
                        worker_id, filename, s.title, parsed_eps
                    );
                    continue;
                }

                let ep_list: String = eps
                    .iter()
                    .map(|e| format!("S{:02}E{:02}", e.season_number, e.episode_number))
                    .collect::<Vec<_>>()
                    .join("+");
                info!(
                    "[worker:{}] '{}' → '{}' {} (known series)",
                    worker_id, filename, s.title, ep_list
                );

                // Build ParsedEpisodeInfo — quality comes from worker probe, not parser
                let first_season = parsed_eps.first().map(|&(s, _)| s).unwrap_or(1);
                let ep_numbers: Vec<i32> = parsed_eps.iter().map(|&(_, e)| e).collect();
                let pi = parsed_info.unwrap_or_else(|| crate::core::parser::ParsedEpisodeInfo {
                    series_title: s.title.clone(),
                    season_number: Some(first_season),
                    episode_numbers: ep_numbers.clone(),
                    ..Default::default()
                });
                // Ensure episode_numbers are set even if parse_title extracted different ones
                let pi = crate::core::parser::ParsedEpisodeInfo {
                    episode_numbers: ep_numbers,
                    season_number: Some(first_season),
                    ..pi
                };
                (s, eps, pi)
            } else if let Some(pi) = parsed_info {
                // Fallback: no known series — try to match from filename
                let s = match crate::core::download::import::match_series_standalone(&self.db, &pi)
                    .await
                {
                    Ok(Some(s)) => s,
                    _ => {
                        info!("[worker:{}] no series match for '{}'", worker_id, filename);
                        continue;
                    }
                };

                let eps = match crate::core::download::import::match_episodes_standalone(
                    &self.db, &s, &pi,
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
                            worker_id, filename, s.title, ep_list, parsed_eps
                        );
                        eps
                    }
                    _ => {
                        info!(
                            "[worker:{}] '{}' → '{}' — no episode match (parsed {:?})",
                            worker_id, filename, s.title, parsed_eps
                        );
                        continue;
                    }
                };
                (s, eps, pi)
            } else {
                debug!(
                    "[worker:{}] could not parse title from '{}' and no known series",
                    worker_id, filename
                );
                continue;
            };

            // Same-size skip: if ALL matched episodes already have files with the
            // same size, the source file is likely identical — skip to avoid redundant
            // processing during automatic imports (RSS sync, download completed).
            // Bypassed entirely for user-initiated imports (force_import_all=true)
            // and per-file force_reimport overrides.
            if !force_import_all {
                let is_force_reimport = force_reimport.contains(filename)
                    || force_reimport.iter().any(|p| p.ends_with(filename));
                if !is_force_reimport {
                    let episode_file_repo =
                        crate::core::datastore::repositories::EpisodeFileRepository::new(
                            self.db.clone(),
                        );
                    let ep_files: Vec<_> = episode_file_repo
                        .get_by_series_id(series.id)
                        .await
                        .unwrap_or_default();
                    let ep_file_map: HashMap<i64, &_> =
                        ep_files.iter().map(|f| (f.id, f)).collect();
                    let all_same = episodes.iter().all(|ep| {
                        ep.episode_file_id
                            .and_then(|fid| ep_file_map.get(&fid))
                            .map(|existing| {
                                let size_match = existing.size == file.size;
                                if size_match {
                                    match (&existing.file_hash, &file.file_hash) {
                                        (Some(eh), Some(fh))
                                            if !eh.is_empty() && !fh.is_empty() =>
                                        {
                                            eh == fh
                                        }
                                        _ => true,
                                    }
                                } else {
                                    false
                                }
                            })
                            .unwrap_or(false)
                    });
                    if all_same {
                        info!(
                            "[worker:{}] skipping '{}' — identical to existing file(s) \
                             (source_size={}, existing_size={}, hash={})",
                            worker_id,
                            filename,
                            file.size,
                            episodes
                                .first()
                                .and_then(|ep| ep.episode_file_id)
                                .and_then(|fid| ep_file_map.get(&fid))
                                .map(|f| f.size)
                                .unwrap_or(-1),
                            file.file_hash.as_deref().unwrap_or("none"),
                        );
                        continue;
                    }
                }
            }

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
                .unwrap_or_else(|| serde_json::to_string(&parsed_info.quality).unwrap_or_default());

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
                        series_id: known_series_id,
                        file_mappings,
                        overrides: HashMap::new(),
                        pre_resolved_episodes: Vec::new(),
                        force_reimport: force_reimport.clone(),
                        skip_files: skip_files.clone(),
                        force_import_all,
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

            // Phase 3: Send per-file ImportFilesRequest to worker (durable queue)
            self.event_bus
                .enqueue_job(Message::ImportFilesRequest {
                    job_id: import_job_id,
                    files: vec![ImportFileSpec {
                        source_path: file.path.clone(),
                        dest_path,
                    }],
                })
                .await;
        }

        // Discovery phase complete — all files from this ScanResult have been
        // processed and imports dispatched. Mark scan_finished so try_download_cleanup()
        // can fire once all ImportFilesResults arrive.
        let files_dispatched = {
            let mut jobs = self.pending_jobs.write().await;
            if let Some(tracker) = jobs.download_job_trackers.get_mut(job_id) {
                tracker.scan_finished = true;
                tracker.files_dispatched
            } else {
                0
            }
        };

        // If no files were dispatched (all skipped/unmatched), mark job complete now
        if files_dispatched == 0 {
            info!(
                "[worker:{}] No importable files for download job {} — marking complete",
                worker_id, job_id
            );
            self.try_download_cleanup(job_id).await;
            self.mark_job_result_received(job_id).await;
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

        // Check if this is a movie import first
        let movie_pending = {
            let mut jobs = self.pending_jobs.write().await;
            jobs.movie_file_imports.remove(job_id)
        };
        if let Some(movie_import) = movie_pending {
            self.handle_movie_import_result(job_id, worker_id, results, movie_import)
                .await;
            return;
        }

        // Look up pending import state for this per-file import (series)
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
                quality: mapping.quality.clone().into(),
                languages: r#"[{"id":1,"name":"English"}]"#.to_string().into(),
                media_info: mapping.media_info.clone().map(Into::into),
                original_file_path: Some(mapping.source_path.to_string_lossy().to_string()),
                file_hash: mapping.file_hash.clone(),
            };

            match episode_file_repo.insert(&episode_file).await {
                Ok(file_id) => {
                    total_imported += 1;

                    // Batch link episodes to the file
                    if !mapping.episode_ids.is_empty() {
                        let pool = self.db.pool();
                        let linked = sqlx::query(
                            "UPDATE episodes SET has_file = true, episode_file_id = $1 WHERE id = ANY($2)",
                        )
                        .bind(file_id)
                        .bind(&mapping.episode_ids)
                        .execute(pool)
                        .await;
                        if let Ok(result) = linked {
                            total_episodes_linked += result.rows_affected() as usize;
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
                            quality: mapping.parsed_quality_json.clone().into(),
                            languages: "[]".to_string().into(),
                            custom_formats: "[]".to_string().into(),
                            custom_format_score: 0,
                            quality_cutoff_not_met: false,
                            date: Utc::now(),
                            download_id: Some(download_id.clone()),
                            event_type: 3, // DownloadImported
                            data: "{}".to_string().into(),
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

    /// Handle movie import result (Phase 4): create movie_file record after file move.
    async fn handle_movie_import_result(
        &self,
        job_id: &str,
        worker_id: &str,
        results: Vec<crate::core::messaging::ImportFileResult>,
        pending: PendingMovieFileImport,
    ) {
        let file_repo = MovieFileRepository::new(self.db.clone());
        let history_repo = HistoryRepository::new(self.db.clone());

        let mut total_imported = 0;

        for result in &results {
            if !result.success {
                error!(
                    "Worker failed to move movie file {} -> {}: {}",
                    result.source_path.display(),
                    result.dest_path.display(),
                    result.error.as_deref().unwrap_or("unknown error")
                );
                continue;
            }

            let dest_path_str = result.dest_path.to_string_lossy().to_string();

            let relative_path = result
                .dest_path
                .strip_prefix(&pending.movie_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    result
                        .dest_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                });

            use crate::core::datastore::models::MovieFileDbModel;
            let movie_file = MovieFileDbModel {
                id: 0,
                movie_id: pending.movie_id,
                relative_path,
                path: dest_path_str.clone(),
                size: result.file_size,
                date_added: Utc::now(),
                scene_name: Some(pending.scene_name.clone()),
                release_group: pending.release_group.clone(),
                quality: pending.quality.clone().into(),
                languages: r#"[{"id":1,"name":"English"}]"#.to_string().into(),
                media_info: pending.media_info.clone().map(Into::into),
                original_file_path: Some(pending.source_path.to_string_lossy().to_string()),
                edition: None,
                file_hash: pending.file_hash.clone(),
            };

            match file_repo.insert(&movie_file).await {
                Ok(file_id) => {
                    total_imported += 1;

                    // Update movie to point at the new file
                    let pool = self.db.pool();
                    let _ = sqlx::query(
                        "UPDATE movies SET has_file = true, movie_file_id = $1 WHERE id = $2",
                    )
                    .bind(file_id)
                    .bind(pending.movie_id)
                    .execute(pool)
                    .await;

                    // Record history
                    let download_id = {
                        let jobs = self.pending_jobs.read().await;
                        jobs.import_to_scan_job
                            .get(job_id)
                            .and_then(|scan_id| jobs.download_job_trackers.get(scan_id))
                            .map(|t| t.download_id.clone())
                            .unwrap_or_default()
                    };

                    let history = crate::core::datastore::models::HistoryDbModel {
                        id: 0,
                        series_id: None,
                        episode_id: None,
                        movie_id: Some(pending.movie_id),
                        source_title: pending.scene_name.clone(),
                        quality: pending.quality.clone().into(),
                        languages: "[]".to_string().into(),
                        custom_formats: "[]".to_string().into(),
                        custom_format_score: 0,
                        quality_cutoff_not_met: false,
                        date: Utc::now(),
                        download_id: Some(download_id),
                        event_type: 3, // DownloadImported
                        data: "{}".to_string().into(),
                    };
                    let _ = history_repo.insert(&history).await;

                    info!(
                        "[worker:{}] Imported movie '{}' → {} (file_id={})",
                        worker_id, pending.movie_title, dest_path_str, file_id
                    );

                    // Notify frontend
                    self.event_bus
                        .publish(Message::MovieFileImported {
                            movie_file_id: file_id,
                            movie_id: pending.movie_id,
                        })
                        .await;
                }
                Err(e) => {
                    error!(
                        "Failed to insert movie file record for {}: {}",
                        dest_path_str, e
                    );
                }
            }
        }

        // Update the download job tracker
        let scan_job_id = {
            let mut jobs = self.pending_jobs.write().await;
            let scan_job_id = jobs.import_to_scan_job.remove(job_id).clone();
            if let Some(ref sjid) = scan_job_id {
                if let Some(tracker) = jobs.download_job_trackers.get_mut(sjid.as_str()) {
                    tracker.files_completed += 1;
                    tracker.files_imported += total_imported;
                }
            }
            scan_job_id
        };

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

        // Remove the tracked download record after successful import.
        // The download stays in the client (user controls seeding) — we just
        // stop tracking it in pir9.  The history table already has the
        // Grabbed/Imported events for the completed tab.
        if tracker.files_imported > 0 && tracker.download_client_id > 0 {
            if let Some(ref tracked) = self.tracked {
                tracked
                    .remove_by_download_id(tracker.download_client_id, &tracker.download_id)
                    .await;
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

        // Mark the PendingJob as completed so system status stops showing "importing"
        self.mark_job_result_received(scan_job_id).await;
    }

    /// Mark a job result as received in the pending jobs tracker.
    /// Auto-completes the job when all expected results have arrived.
    /// Set the worker_id on a job (captured from first ScanResult)
    async fn set_job_worker_id(&self, job_id: &str, worker_id: &str) {
        let mut jobs = self.pending_jobs.write().await;
        if let Some(job) = jobs.jobs.get_mut(job_id) {
            if job.worker_id.is_none() {
                job.worker_id = Some(worker_id.to_string());
            }
        }
    }

    async fn mark_job_result_received(&self, job_id: &str) {
        let mut jobs = self.pending_jobs.write().await;
        if let Some(job) = jobs.jobs.get_mut(job_id) {
            job.results_received += 1;
            if !job.entity_ids.is_empty() && job.results_received >= job.entity_ids.len() {
                job.completed = true;
                info!(
                    "Scan job {} completed ({}/{} results)",
                    job_id,
                    job.results_received,
                    job.entity_ids.len()
                );
                // Clean up streaming state for this job
                jobs.streamed_entities.retain(|(jid, _)| jid != job_id);
            } else {
                debug!(
                    "Job {} received result {}/{}",
                    job_id,
                    job.results_received,
                    job.entity_ids.len()
                );
            }
        }
    }

    /// Dispatch individual probe + hash jobs for files that need enrichment.
    /// Creates a PendingFileJob for each file and publishes ProbeFileRequest + HashFileRequest.
    async fn dispatch_enrichment_jobs(
        &self,
        parent_job_id: &str,
        entity_id: i64,
        scan_type: ScanType,
        files: Vec<ScannedFile>,
    ) {
        let event_bus = &self.event_bus;

        let file_count = files.len();
        info!(
            "Dispatching {} probe + {} hash jobs for entity {} (parent={})",
            file_count, file_count, entity_id, parent_job_id
        );

        let mut jobs = self.pending_jobs.write().await;

        // Track total enrichment files on the parent job for progress reporting
        if let Some(parent) = jobs.jobs.get_mut(parent_job_id) {
            parent.enrichment_total += file_count;
        }

        for file in files {
            let file_key = format!("{}:{}", parent_job_id, file.path.to_string_lossy());
            let probe_job_id = uuid::Uuid::new_v4().to_string();
            let hash_job_id = uuid::Uuid::new_v4().to_string();
            let file_path_str = file.path.to_string_lossy().to_string();

            // Track the pending enrichment
            jobs.file_jobs.insert(
                file_key.clone(),
                PendingFileJob {
                    parent_job_id: parent_job_id.to_string(),
                    entity_id,
                    scan_type,
                    file: file.clone(),
                    probe_job_id: probe_job_id.clone(),
                    hash_job_id: hash_job_id.clone(),
                    probe_result: None,
                    hash_result: None,
                },
            );

            // Map job IDs to file key for result lookup
            jobs.enrichment_job_to_file
                .insert(probe_job_id.clone(), file_key.clone());
            jobs.enrichment_job_to_file
                .insert(hash_job_id.clone(), file_key.clone());

            // Enqueue probe job (durable Redis list — persists until a worker picks it up)
            event_bus
                .enqueue_job(Message::ProbeFileRequest {
                    job_id: probe_job_id,
                    parent_job_id: parent_job_id.to_string(),
                    file_path: file_path_str.clone(),
                    entity_id,
                    scan_type,
                })
                .await;

            // Enqueue hash job (separate job — worker does one at a time)
            event_bus
                .enqueue_job(Message::HashFileRequest {
                    job_id: hash_job_id,
                    parent_job_id: parent_job_id.to_string(),
                    file_path: file_path_str,
                    entity_id,
                })
                .await;
        }
    }

    /// Handle a probe result for a single file
    async fn handle_probe_file_result(
        &self,
        job_id: &str,
        file_path: &str,
        entity_id: i64,
        worker_id: &str,
        media_info: Option<String>,
        quality: Option<String>,
    ) {
        debug!(
            "[probe] Result: job={}, entity={}, worker={}, file={}",
            job_id,
            entity_id,
            worker_id,
            std::path::Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );

        let ready_file = {
            let mut jobs = self.pending_jobs.write().await;
            let file_key = match jobs.enrichment_job_to_file.get(job_id) {
                Some(k) => k.clone(),
                None => {
                    warn!("Probe result for unknown job {}", job_id);
                    return;
                }
            };

            if let Some(pending) = jobs.file_jobs.get_mut(&file_key) {
                pending.probe_result = Some((media_info, quality));

                // Check if hash is also done
                if pending.hash_result.is_some() {
                    // Both done — remove and return for processing
                    let completed = jobs.file_jobs.remove(&file_key).unwrap();
                    jobs.enrichment_job_to_file.remove(&completed.probe_job_id);
                    jobs.enrichment_job_to_file.remove(&completed.hash_job_id);
                    Some(completed)
                } else {
                    None
                }
            } else {
                warn!("Probe result for unknown file key {}", file_key);
                None
            }
        };

        if let Some(completed) = ready_file {
            self.process_enriched_file(completed).await;
        }
    }

    /// Handle a hash result for a single file
    async fn handle_hash_file_result(
        &self,
        job_id: &str,
        file_path: &str,
        entity_id: i64,
        worker_id: &str,
        file_hash: Option<String>,
    ) {
        debug!(
            "[hash] Result: job={}, entity={}, worker={}, file={}",
            job_id,
            entity_id,
            worker_id,
            std::path::Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );

        let ready_file = {
            let mut jobs = self.pending_jobs.write().await;
            let file_key = match jobs.enrichment_job_to_file.get(job_id) {
                Some(k) => k.clone(),
                None => {
                    warn!("Hash result for unknown job {}", job_id);
                    return;
                }
            };

            if let Some(pending) = jobs.file_jobs.get_mut(&file_key) {
                pending.hash_result = Some(file_hash);

                if pending.probe_result.is_some() {
                    let completed = jobs.file_jobs.remove(&file_key).unwrap();
                    jobs.enrichment_job_to_file.remove(&completed.probe_job_id);
                    jobs.enrichment_job_to_file.remove(&completed.hash_job_id);
                    Some(completed)
                } else {
                    None
                }
            } else {
                warn!("Hash result for unknown file key {}", file_key);
                None
            }
        };

        if let Some(completed) = ready_file {
            self.process_enriched_file(completed).await;
        }
    }

    /// Process a file that has both probe and hash results.
    /// Builds the enriched ScannedFile and routes to the appropriate processor.
    async fn process_enriched_file(&self, completed: PendingFileJob) {
        let (media_info, quality) = completed.probe_result.unwrap_or((None, None));
        let file_hash = completed.hash_result.unwrap_or(None);

        let mut file = completed.file;
        file.media_info = media_info;
        file.quality = quality;
        file.file_hash = file_hash;

        let entity_id = completed.entity_id;
        let parent_job_id = completed.parent_job_id.clone();
        let scan_type = completed.scan_type;
        let filename = file.filename.clone();

        info!(
            "[enriched] {} (entity={}, hash={})",
            filename,
            entity_id,
            file.file_hash.as_deref().unwrap_or("none")
        );

        match scan_type {
            ScanType::RescanSeries => {
                if let Err(e) = self.process_scanned_files(entity_id, vec![file]).await {
                    error!(
                        "Failed to process enriched file for series {}: {}",
                        entity_id, e
                    );
                }
            }
            ScanType::RescanMovie => {
                if let Err(e) = self.process_movie_scan_result(entity_id, file).await {
                    error!(
                        "Failed to process enriched movie file for movie {}: {}",
                        entity_id, e
                    );
                }
            }
            ScanType::DownloadedEpisodesScan | ScanType::DownloadedMovieScan => {
                warn!(
                    "Enriched file for download scan type {:?} reached process_enriched_file — \
                     this is unexpected. File {} will be skipped.",
                    scan_type, filename
                );
            }
            _ => {
                warn!("Unexpected scan type {:?} for enriched file", scan_type);
            }
        }

        // Update enrichment progress on the parent job and forward to WebSocket
        self.update_enrichment_progress(&parent_job_id, &filename, scan_type)
            .await;

        // Check if all enrichment jobs for this parent are done
        self.check_enrichment_complete(&parent_job_id).await;
    }

    /// Update the parent job's progress after a file enrichment completes,
    /// and forward synthesized progress to WebSocket for real-time UI updates.
    async fn update_enrichment_progress(
        &self,
        parent_job_id: &str,
        filename: &str,
        scan_type: ScanType,
    ) {
        let progress_info = {
            let mut jobs = self.pending_jobs.write().await;
            if let Some(job) = jobs.jobs.get_mut(parent_job_id) {
                job.enrichment_completed += 1;
                let total = job.enrichment_total;
                let done = job.enrichment_completed;
                let percent = if total > 0 {
                    (done as f32 / total as f32) * 100.0
                } else {
                    0.0
                };

                let progress = ScanProgressInfo {
                    stage: "enriching".to_string(),
                    current_file: Some(filename.to_string()),
                    files_total: total,
                    files_processed: done,
                    percent,
                    detail: None,
                    bytes_copied: None,
                    bytes_total: None,
                };
                job.progress = Some(progress.clone());

                Some((
                    job.worker_id.clone().unwrap_or_default(),
                    job.entity_ids.clone(),
                    progress,
                ))
            } else {
                None
            }
        };

        // Forward synthesized progress to WebSocket
        if let Some((worker_id, entity_ids, progress)) = progress_info {
            if let Some(ref ws_bus) = self.ws_event_bus {
                ws_bus
                    .publish(Message::ScanProgress {
                        job_id: parent_job_id.to_string(),
                        worker_id,
                        stage: progress.stage.clone(),
                        current_file: progress.current_file.clone(),
                        files_total: progress.files_total,
                        files_processed: progress.files_processed,
                        percent: progress.percent,
                        detail: progress.detail.clone(),
                        entity_ids,
                        scan_type: Some(scan_type),
                        bytes_copied: None,
                        bytes_total: None,
                    })
                    .await;
            }
        }
    }

    /// Check if all file enrichment jobs for a parent scan job are complete.
    /// If so, mark the entity result as received.
    async fn check_enrichment_complete(&self, parent_job_id: &str) {
        let has_pending = {
            let jobs = self.pending_jobs.read().await;
            jobs.file_jobs
                .values()
                .any(|fj| fj.parent_job_id == parent_job_id)
        };

        if !has_pending {
            info!(
                "All enrichment jobs complete for parent job {}",
                parent_job_id
            );
            self.mark_job_result_received(parent_job_id).await;
        }
    }

    /// Load known file data from DB for a series (for skip-enrichment)
    async fn load_known_series_files(
        &self,
        series_id: i64,
    ) -> HashMap<String, crate::core::messaging::KnownFileInfo> {
        let pool = self.db.pool();
        let rows: Vec<(String, i64, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT path, size, media_info, quality, file_hash FROM episode_files WHERE series_id = $1",
            )
            .bind(series_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        rows.into_iter()
            .map(|(path, size, media_info, quality, file_hash)| {
                (
                    path,
                    crate::core::messaging::KnownFileInfo {
                        size,
                        media_info,
                        quality,
                        file_hash,
                    },
                )
            })
            .collect()
    }

    /// Load known file data from DB for a movie (for skip-enrichment)
    async fn load_known_movie_files(
        &self,
        movie_id: i64,
    ) -> HashMap<String, crate::core::messaging::KnownFileInfo> {
        let pool = self.db.pool();
        let rows: Vec<(String, i64, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT path, size, media_info, quality, file_hash FROM movie_files WHERE movie_id = $1",
            )
            .bind(movie_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        rows.into_iter()
            .map(|(path, size, media_info, quality, file_hash)| {
                (
                    path,
                    crate::core::messaging::KnownFileInfo {
                        size,
                        media_info,
                        quality,
                        file_hash,
                    },
                )
            })
            .collect()
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
                    quality: quality_json.to_string().into(),
                    languages: languages_json.to_string().into(),
                    media_info: media_info.map(Into::into),
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
            let mut linked_episode_ids = Vec::new();
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
                            linked_episode_ids.push(ep.id);
                            debug!(
                                "Linked S{:02}E{:02} to file {}",
                                season_number, episode_num, episode_file_id
                            );
                        }
                    }
                }
            }

            // Publish per-file event so the frontend updates incrementally
            if !linked_episode_ids.is_empty() {
                self.event_bus
                    .publish(Message::EpisodeFileImported {
                        episode_file_id,
                        series_id,
                        episode_ids: linked_episode_ids,
                    })
                    .await;
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

    /// Register a pending rename job so the consumer can update DB when the worker finishes.
    pub async fn register_rename_job(&self, job_id: &str, episode_file_ids: Vec<i64>) {
        let mut jobs = self.pending_jobs.write().await;
        jobs.pending_renames
            .insert(job_id.to_string(), episode_file_ids);
    }

    /// Handle rename files result from worker — update episode_file paths in DB.
    async fn handle_rename_files_result(
        &self,
        job_id: &str,
        worker_id: &str,
        results: Vec<ImportFileResult>,
    ) {
        let episode_file_ids = {
            let mut jobs = self.pending_jobs.write().await;
            jobs.pending_renames.remove(job_id).unwrap_or_default()
        };

        let file_repo =
            crate::core::datastore::repositories::EpisodeFileRepository::new(self.db.clone());

        let mut renamed = 0;
        let mut failed = 0;

        for (idx, result) in results.iter().enumerate() {
            if !result.success {
                warn!(
                    "[rename] Worker {} failed to rename {} → {}: {}",
                    worker_id,
                    result.source_path.display(),
                    result.dest_path.display(),
                    result.error.as_deref().unwrap_or("unknown error"),
                );
                failed += 1;
                continue;
            }

            // Update episode_file DB record with new path
            let file_id = episode_file_ids.get(idx).copied().unwrap_or(0);
            if file_id == 0 {
                warn!(
                    "[rename] No episode_file_id for index {} in job {}",
                    idx, job_id
                );
                continue;
            }

            match file_repo.get_by_id(file_id).await {
                Ok(Some(mut ef)) => {
                    let new_path = result.dest_path.to_string_lossy().to_string();
                    // Compute relative path if we can determine the series root
                    let series_repo = crate::core::datastore::repositories::SeriesRepository::new(
                        self.db.clone(),
                    );
                    let relative =
                        if let Ok(Some(series)) = series_repo.get_by_id(ef.series_id).await {
                            std::path::Path::new(&new_path)
                                .strip_prefix(&series.path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| new_path.clone())
                        } else {
                            new_path.clone()
                        };

                    ef.path = new_path;
                    ef.relative_path = relative;
                    if let Err(e) = file_repo.update(&ef).await {
                        warn!(
                            "[rename] DB update failed for episode_file {}: {}",
                            file_id, e
                        );
                        failed += 1;
                    } else {
                        renamed += 1;
                    }
                }
                Ok(None) => {
                    warn!("[rename] episode_file {} not found in DB", file_id);
                    failed += 1;
                }
                Err(e) => {
                    warn!("[rename] Failed to fetch episode_file {}: {}", file_id, e);
                    failed += 1;
                }
            }
        }

        info!(
            "[rename] Job {} complete (worker {}): {} renamed, {} failed",
            job_id, worker_id, renamed, failed,
        );
    }
}

/// Create a scan request message for series.
///
/// `known_files` maps file path → known DB metadata. The worker uses this to skip
/// FFmpeg probe + BLAKE3 hash for files whose size hasn't changed. Pass an empty
/// map for new imports where all files need enrichment.
pub fn create_scan_request(
    series_ids: Vec<i64>,
    paths: Vec<String>,
    known_files: HashMap<String, crate::core::messaging::KnownFileInfo>,
) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanSeries,
        series_ids: series_ids.clone(),
        paths,
        known_files,
    };

    (job_id, message)
}

/// Create a scan request message for movies.
///
/// `known_files` maps file path → known DB metadata for skip-enrichment optimization.
/// Pass an empty map for new imports.
pub fn create_movie_scan_request(
    movie_ids: Vec<i64>,
    paths: Vec<String>,
    known_files: HashMap<String, crate::core::messaging::KnownFileInfo>,
) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanMovie,
        series_ids: movie_ids.clone(), // reused field for movie IDs
        paths,
        known_files,
    };

    (job_id, message)
}

/// Create a scan request for podcast libraries (stub — not yet implemented)
pub fn create_podcast_scan_request(podcast_ids: Vec<i64>, paths: Vec<String>) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanPodcast,
        series_ids: podcast_ids,
        paths,
        known_files: HashMap::new(),
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
        known_files: HashMap::new(),
    };

    (job_id, message)
}

/// Create a scan request for audiobook libraries (stub — not yet implemented)
pub fn create_audiobook_scan_request(
    audiobook_ids: Vec<i64>,
    paths: Vec<String>,
) -> (String, Message) {
    let job_id = uuid::Uuid::new_v4().to_string();

    let message = Message::ScanRequest {
        job_id: job_id.clone(),
        scan_type: ScanType::RescanAudiobook,
        series_ids: audiobook_ids,
        paths,
        known_files: HashMap::new(),
    };

    (job_id, message)
}
