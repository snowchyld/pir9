#![allow(dead_code, unused_imports)]
//! Worker module for distributed file scanning
//!
//! The worker runs on machines with direct access to media files (e.g., on a NAS).
//! It subscribes to Redis for scan requests, executes scans locally, and publishes
//! results back to the server.
//!
//! This eliminates network I/O for file scanning operations when the worker runs
//! on the same machine as the files.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};

use crate::core::messaging::{KnownFileInfo, Message, ScannedFile, ScanType};
use crate::core::scanner;
use std::collections::HashMap;

/// Get a human-readable name for a message type (for logging)
fn message_type_name(message: &Message) -> &'static str {
    match message {
        Message::CommandStarted { .. } => "CommandStarted",
        Message::CommandUpdated { .. } => "CommandUpdated",
        Message::CommandCompleted { .. } => "CommandCompleted",
        Message::CommandFailed { .. } => "CommandFailed",
        Message::SeriesAdded { .. } => "SeriesAdded",
        Message::SeriesUpdated { .. } => "SeriesUpdated",
        Message::SeriesDeleted { .. } => "SeriesDeleted",
        Message::SeriesRefreshed { .. } => "SeriesRefreshed",
        Message::SeriesScanned { .. } => "SeriesScanned",
        Message::EpisodeAdded { .. } => "EpisodeAdded",
        Message::EpisodeUpdated { .. } => "EpisodeUpdated",
        Message::EpisodeFileImported { .. } => "EpisodeFileImported",
        Message::EpisodeFileDeleted { .. } => "EpisodeFileDeleted",
        Message::EpisodeSearchRequested { .. } => "EpisodeSearchRequested",
        Message::SeasonSearchRequested { .. } => "SeasonSearchRequested",
        Message::SeriesSearchRequested { .. } => "SeriesSearchRequested",
        Message::ReleaseGrabbed { .. } => "ReleaseGrabbed",
        Message::DownloadStarted { .. } => "DownloadStarted",
        Message::DownloadCompleted { .. } => "DownloadCompleted",
        Message::DownloadFailed { .. } => "DownloadFailed",
        Message::QueueUpdated => "QueueUpdated",
        Message::HealthCheckChanged => "HealthCheckChanged",
        Message::ConfigUpdated => "ConfigUpdated",
        Message::NotificationSent { .. } => "NotificationSent",
        Message::ScanRequest { .. } => "ScanRequest",
        Message::ScanProgress { .. } => "ScanProgress",
        Message::ScanResult { .. } => "ScanResult",
        Message::WorkerOnline { .. } => "WorkerOnline",
        Message::WorkerOffline { .. } => "WorkerOffline",
        Message::WorkerHeartbeat { .. } => "WorkerHeartbeat",
        Message::MovieAdded { .. } => "MovieAdded",
        Message::MovieUpdated { .. } => "MovieUpdated",
        Message::MovieDeleted { .. } => "MovieDeleted",
        Message::MovieRefreshed { .. } => "MovieRefreshed",
        Message::MovieFileImported { .. } => "MovieFileImported",
        Message::MovieFileDeleted { .. } => "MovieFileDeleted",
        Message::ImportFilesRequest { .. } => "ImportFilesRequest",
        Message::ImportFilesResult { .. } => "ImportFilesResult",
        Message::DeletePathsRequest { .. } => "DeletePathsRequest",
        Message::DeletePathsResult { .. } => "DeletePathsResult",
    }
}

/// Worker runner that handles scan requests from Redis
pub struct WorkerRunner {
    /// Worker's unique ID
    worker_id: String,
    /// Paths this worker is responsible for scanning
    worker_paths: Vec<PathBuf>,
    /// Redis URL for event bus
    redis_url: String,
    /// Redis connection for job claiming (distributed lock)
    #[cfg(feature = "redis-events")]
    redis_conn: tokio::sync::Mutex<Option<redis::aio::ConnectionManager>>,
    /// Limits the worker to one scan job at a time. When busy, incoming scan
    /// requests are skipped (not claimed) so the other worker can pick them up.
    /// ImportFilesRequest is NOT gated — it's part of the active scan workflow.
    scan_semaphore: Arc<tokio::sync::Semaphore>,
    /// Statistics: number of scans completed
    scans_completed: std::sync::atomic::AtomicU64,
    /// Statistics: total files found
    files_found: std::sync::atomic::AtomicU64,
    /// When the worker started
    start_time: std::time::Instant,
}

impl WorkerRunner {
    /// Create a new worker runner
    pub async fn new(redis_url: &str, worker_id: &str, worker_paths: Vec<String>) -> Result<Self> {
        let paths: Vec<PathBuf> = worker_paths.iter().map(PathBuf::from).collect();

        // Validate paths exist
        for path in &paths {
            if !path.exists() {
                warn!("Worker path does not exist: {:?}", path);
            }
        }

        Ok(Self {
            worker_id: worker_id.to_string(),
            worker_paths: paths,
            redis_url: redis_url.to_string(),
            #[cfg(feature = "redis-events")]
            redis_conn: tokio::sync::Mutex::new(None),
            scan_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            scans_completed: std::sync::atomic::AtomicU64::new(0),
            files_found: std::sync::atomic::AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        })
    }

    /// Get worker paths as strings
    fn paths_as_strings(&self) -> Vec<String> {
        self.worker_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    }

    /// Get current statistics
    fn get_stats(&self) -> (u64, u64, u64) {
        use std::sync::atomic::Ordering;
        (
            self.scans_completed.load(Ordering::Relaxed),
            self.files_found.load(Ordering::Relaxed),
            self.start_time.elapsed().as_secs(),
        )
    }

    /// Increment scan statistics
    fn record_scan(&self, files_count: u64) {
        use std::sync::atomic::Ordering;
        self.scans_completed.fetch_add(1, Ordering::Relaxed);
        self.files_found.fetch_add(files_count, Ordering::Relaxed);
    }

    /// Run the worker loop
    ///
    /// This blocks until a shutdown signal is received.
    #[cfg(feature = "redis-events")]
    pub async fn run(self) -> Result<()> {
        use crate::core::messaging::HybridEventBus;
        use crate::core::scanner::registry::HEARTBEAT_INTERVAL;
        use tokio::signal;

        // Wrap in Arc so spawned background tasks can access worker state
        let this = Arc::new(self);

        info!(
            "Starting worker {} with paths: {:?}",
            this.worker_id, this.worker_paths
        );

        // Connect to Redis
        let event_bus = HybridEventBus::new_redis(&this.redis_url)
            .await
            .context("Failed to connect to Redis")?;

        let event_bus = Arc::new(event_bus);

        // Create a separate Redis connection for job claiming (SET NX)
        {
            let client = redis::Client::open(this.redis_url.as_str())
                .context("Failed to create Redis client for job claiming")?;
            let conn = redis::aio::ConnectionManager::new(client)
                .await
                .context("Failed to connect to Redis for job claiming")?;
            *this.redis_conn.lock().await = Some(conn);
        }

        // Start the Redis subscriber in background to receive messages from other instances
        let event_bus_clone = event_bus.clone();
        tokio::spawn(async move {
            if let Err(e) = event_bus_clone.start_redis_subscriber().await {
                error!("Redis subscriber error: {}", e);
            }
        });

        // Give the subscriber a moment to connect
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Announce that we're online
        event_bus
            .publish(Message::WorkerOnline {
                worker_id: this.worker_id.clone(),
                paths: this.paths_as_strings(),
            })
            .await;

        info!("Worker online, waiting for scan requests...");

        // Subscribe to events
        let mut receiver = event_bus.subscribe();

        // Create heartbeat interval — Skip missed ticks so heartbeats don't burst
        // after a long blocking operation (e.g., 100s file copy)
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Handle shutdown signal
        let shutdown = async {
            let ctrl_c = signal::ctrl_c();
            #[cfg(unix)]
            let terminate = async {
                signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("Failed to install SIGTERM handler")
                    .recv()
                    .await;
            };
            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => {}
                _ = terminate => {}
            }
        };

        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!("Shutdown signal received");
                    break;
                }
                _ = heartbeat_interval.tick() => {
                    // Send heartbeat
                    let (scans, files, uptime) = this.get_stats();
                    trace!("Sending heartbeat: scans={}, files={}, uptime={}s", scans, files, uptime);
                    event_bus.publish(Message::WorkerHeartbeat {
                        worker_id: this.worker_id.clone(),
                        paths: this.paths_as_strings(),
                        scans_completed: scans,
                        files_found: files,
                        uptime_seconds: uptime,
                    }).await;
                }
                result = receiver.recv() => {
                    match result {
                        Ok(message) => {
                            this.handle_message(message, &event_bus).await;
                        }
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                            // Reconnect logic could go here
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }

        // Announce that we're going offline
        event_bus
            .publish(Message::WorkerOffline {
                worker_id: this.worker_id.clone(),
            })
            .await;

        info!("Worker shutdown complete");
        Ok(())
    }

    /// Fallback when redis-events feature is not enabled
    #[cfg(not(feature = "redis-events"))]
    pub async fn run(self) -> Result<()> {
        anyhow::bail!("Worker mode requires the 'redis-events' feature (enabled by default). Was this built with --no-default-features?")
    }

    /// Handle an incoming message
    #[cfg(feature = "redis-events")]
    async fn handle_message(
        self: &Arc<Self>,
        message: Message,
        event_bus: &Arc<crate::core::messaging::HybridEventBus>,
    ) {
        match &message {
            Message::ScanRequest {
                job_id,
                scan_type,
                series_ids,
                paths,
                known_files,
            } => {
                info!(
                    "Received scan request: job_id={}, type={:?}, series={:?}, paths={:?}, known_files={}",
                    job_id, scan_type, series_ids, paths, known_files.len()
                );

                // Pair series_ids with paths (1:1 aligned), filter to paths we handle, own the data
                let relevant: Vec<(i64, String)> = series_ids
                    .iter()
                    .zip(paths.iter())
                    .filter(|(_, p)| self.handles_path(p))
                    .map(|(&sid, p)| (sid, p.clone()))
                    .collect();

                if relevant.is_empty() {
                    debug!("Scan request not for our paths, ignoring");
                    return;
                }

                // Only accept one scan at a time — if busy, skip so the other worker can claim it
                let permit = match self.scan_semaphore.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        info!("Worker busy with another scan, skipping job {}", job_id);
                        return;
                    }
                };

                // Try to claim this job — only one worker should process each scan
                if !self.try_claim_job(job_id).await {
                    return;
                }

                if *scan_type == ScanType::DownloadedEpisodesScan {
                    // Per-file streaming: spawn enrichment as background task so the
                    // main event loop stays free to process ImportFilesRequests concurrently.
                    // This means file1 can be moved to library while file50 is still being probed.
                    let this = Arc::clone(self);
                    let event_bus = Arc::clone(event_bus);
                    let job_id = job_id.clone();

                    tokio::spawn(async move {
                        // Hold the permit for the duration of the scan — dropped when this
                        // block exits, allowing the worker to accept the next job.
                        let _permit = permit;
                        let mut total_files: u64 = 0;
                        let scan_start = std::time::Instant::now();

                        for (series_id, path_str) in &relevant {
                            let path = PathBuf::from(path_str);
                            if !path.exists() {
                                warn!("[scan][download] Path does not exist, skipping: {}", path_str);
                                continue;
                            }

                            info!("[scan][download] Scanning {}", path_str);
                            let mut files = scanner::scan_download_directory(&path);
                            let file_count = files.len();
                            info!(
                                "[scan][download] Found {} video file(s) in {}",
                                file_count, path_str
                            );

                            publish_progress(&event_bus, &job_id, &this.worker_id, "scanning", None, file_count, 0, 0, None).await;

                            let dl_total_steps = file_count * 2;
                            let mut dl_completed_steps: usize = 0;

                            for (idx, file) in files.iter_mut().enumerate() {
                                // Probe stage
                                let probe_pct = if dl_total_steps > 0 { (dl_completed_steps * 100 / dl_total_steps).min(100) as u8 } else { 0 };
                                publish_progress(&event_bus, &job_id, &this.worker_id, "probing", Some(&file.filename), file_count, idx, probe_pct, None).await;

                                let probe_detail = probe_scanned_file(file, (idx + 1, file_count)).await;
                                dl_completed_steps += 1;

                                let after_probe_pct = if dl_total_steps > 0 { (dl_completed_steps * 100 / dl_total_steps).min(100) as u8 } else { 50 };
                                publish_progress(&event_bus, &job_id, &this.worker_id, "probing", Some(&file.filename), file_count, idx, after_probe_pct, probe_detail).await;

                                // Hash stage
                                publish_progress(&event_bus, &job_id, &this.worker_id, "hashing", Some(&file.filename), file_count, idx, after_probe_pct, None).await;
                                hash_scanned_file(file, (idx + 1, file_count)).await;
                                dl_completed_steps += 1;

                                let after_hash_pct = if dl_total_steps > 0 { (dl_completed_steps * 100 / dl_total_steps).min(100) as u8 } else { 100 };
                                publish_progress(&event_bus, &job_id, &this.worker_id, "hashing", Some(&file.filename), file_count, idx + 1, after_hash_pct, None).await;

                                // Publish per-file result immediately — server can start
                                // matching and dispatching file moves right away
                                event_bus
                                    .publish(Message::ScanResult {
                                        job_id: job_id.clone(),
                                        series_id: *series_id,
                                        worker_id: this.worker_id.clone(),
                                        files_found: vec![file.clone()],
                                        errors: vec![],
                                    })
                                    .await;
                            }

                            total_files += file_count as u64;
                        }

                        let elapsed = scan_start.elapsed();
                        info!(
                            "[scan][download] Streaming scan complete — {} file(s) in {:.1}s",
                            total_files,
                            elapsed.as_secs_f64()
                        );

                        // Signal scan completion with empty result so server knows
                        // no more files are coming and can finalize cleanup
                        event_bus
                            .publish(Message::ScanResult {
                                job_id: job_id.clone(),
                                series_id: 0,
                                worker_id: this.worker_id.clone(),
                                files_found: vec![],
                                errors: vec![],
                            })
                            .await;

                        this.record_scan(total_files);
                    });
                } else {
                    // Batch mode for non-download scans (RescanSeries, RescanMovie, etc.)
                    // Hold permit until scan completes — dropped at end of block
                    let _permit = permit;
                    let relevant_refs: Vec<(i64, &String)> =
                        relevant.iter().map(|(id, p)| (*id, p)).collect();
                    let result = self.execute_scan(job_id, scan_type, &relevant_refs, known_files, event_bus).await;
                    for scan_result in result {
                        event_bus.publish(scan_result).await;
                    }
                }
            }
            // Ignore our own heartbeats and status messages
            Message::WorkerHeartbeat { worker_id, .. } if worker_id == &self.worker_id => {
                // Our own heartbeat echoed back, ignore silently
            }
            Message::WorkerOnline { worker_id, .. } if worker_id == &self.worker_id => {
                // Our own online announcement, ignore silently
            }
            Message::WorkerOffline { worker_id } if worker_id == &self.worker_id => {
                // Our own offline announcement, ignore silently
            }
            // Log other worker events at trace level
            Message::WorkerHeartbeat { worker_id, .. } => {
                trace!("Other worker heartbeat: {}", worker_id);
            }
            Message::WorkerOnline { worker_id, paths } => {
                info!("Worker came online: {} with paths {:?}", worker_id, paths);
            }
            Message::WorkerOffline { worker_id } => {
                info!("Worker went offline: {}", worker_id);
            }
            // File move requests from server (download import Phase 3)
            Message::ImportFilesRequest { job_id, files } => {
                let total = files.len();
                info!("[import] Starting import job {} — {} file(s)", job_id, total);

                // Check if any source paths are on our volumes
                let any_ours = files.iter().any(|f| self.handles_path(&f.source_path.to_string_lossy()));
                if !any_ours {
                    debug!("Import files request not for our paths, ignoring");
                    return;
                }

                // Claim the import job so only one worker moves the files
                if !self.try_claim_job(job_id).await {
                    return;
                }

                let mut results = Vec::new();
                let started = std::time::Instant::now();

                for (idx, spec) in files.iter().enumerate() {
                    let media_type = infer_media_type(&spec.dest_path);
                    let filename = spec.source_path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy();

                    info!(
                        "[import][{}] ({}/{}) {} — {} -> {}",
                        media_type,
                        idx + 1,
                        total,
                        filename,
                        spec.source_path.display(),
                        spec.dest_path.display()
                    );

                    let file_start = std::time::Instant::now();
                    let result = match move_file(&spec.source_path, &spec.dest_path) {
                        Ok(mr) => {
                            let elapsed = file_start.elapsed();
                            info!(
                                "[{}][{}] ({}/{}) {} — {} in {:.1}s",
                                mr.method,
                                media_type,
                                idx + 1,
                                total,
                                filename,
                                format_size(mr.size as u64),
                                elapsed.as_secs_f64()
                            );
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: true,
                                file_size: mr.size,
                                error: None,
                            }
                        }
                        Err(e) => {
                            error!(
                                "[error][{}] ({}/{}) {} — failed: {}",
                                media_type,
                                idx + 1,
                                total,
                                filename,
                                e
                            );
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: false,
                                file_size: 0,
                                error: Some(e.to_string()),
                            }
                        }
                    };
                    results.push(result);
                }

                let succeeded = results.iter().filter(|r| r.success).count();
                let total_bytes: i64 = results.iter().map(|r| r.file_size).sum();
                let elapsed = started.elapsed();
                info!(
                    "[import] Job {} complete — {}/{} succeeded, {} total in {:.1}s",
                    job_id,
                    succeeded,
                    total,
                    format_size(total_bytes as u64),
                    elapsed.as_secs_f64()
                );
                self.record_scan(succeeded as u64);

                event_bus
                    .publish(Message::ImportFilesResult {
                        job_id: job_id.clone(),
                        worker_id: self.worker_id.clone(),
                        results,
                    })
                    .await;
            }
            // File delete requests from server
            Message::DeletePathsRequest {
                job_id,
                paths,
                recursive,
            } => {
                let total = paths.len();
                info!("[delete] Starting delete job {} — {} path(s), recursive={}", job_id, total, recursive);

                // Check if any paths are on our volumes
                let any_ours = paths.iter().any(|p| self.handles_path(p));
                if !any_ours {
                    debug!("Delete paths request not for our paths, ignoring");
                    return;
                }

                // Claim the delete job so only one worker executes it
                if !self.try_claim_job(job_id).await {
                    return;
                }

                let mut results = Vec::new();
                for (idx, path_str) in paths.iter().enumerate() {
                    let path = std::path::Path::new(path_str.as_str());
                    let media_type = infer_media_type(path);
                    let name = path.file_name().unwrap_or_default().to_string_lossy();

                    let result = if path.is_dir() && *recursive {
                        std::fs::remove_dir_all(path)
                    } else if path.is_file() {
                        std::fs::remove_file(path)
                    } else if !path.exists() {
                        Ok(()) // Already gone
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Path is a directory but recursive=false",
                        ))
                    };

                    match result {
                        Ok(()) => {
                            info!("[delete][{}] ({}/{}) {} — {}", media_type, idx + 1, total, name, path_str);
                            results.push((path_str.clone(), true, None));
                        }
                        Err(e) => {
                            error!("[delete][{}] ({}/{}) {} — failed: {}", media_type, idx + 1, total, name, e);
                            results.push((path_str.clone(), false, Some(e.to_string())));
                        }
                    }
                }

                event_bus
                    .publish(Message::DeletePathsResult {
                        job_id: job_id.clone(),
                        worker_id: self.worker_id.clone(),
                        results,
                    })
                    .await;
            }
            // Scan results, progress updates, and import results are for the server, not workers
            Message::ScanResult { .. } | Message::ScanProgress { .. } | Message::ImportFilesResult { .. } | Message::DeletePathsResult { .. } => {
                // Ignore - these are for the server to process
            }
            // Log other message types at trace level
            other => {
                trace!("Ignoring message: {}", message_type_name(other));
            }
        }
    }

    /// Check if this worker handles the given path
    fn handles_path(&self, path: &str) -> bool {
        let path = PathBuf::from(path);
        self.worker_paths.iter().any(|wp| {
            // Check if the path starts with or equals one of our worker paths
            path.starts_with(wp) || wp.starts_with(&path)
        })
    }

    /// Try to claim a job using Redis SET NX (distributed lock).
    /// Returns true if this worker claimed the job, false if another worker already has it.
    /// The lock auto-expires after 1 hour to prevent stale locks from dead workers.
    #[cfg(feature = "redis-events")]
    async fn try_claim_job(&self, job_id: &str) -> bool {
        let mut guard = self.redis_conn.lock().await;
        let conn = match guard.as_mut() {
            Some(c) => c,
            None => {
                warn!("No Redis connection for job claiming, proceeding without lock");
                return true;
            }
        };

        let key = format!("pir9:job:{}", job_id);
        let result: redis::RedisResult<bool> = redis::cmd("SET")
            .arg(&key)
            .arg(&self.worker_id)
            .arg("NX") // Only set if not exists
            .arg("EX")
            .arg(3600) // Auto-expire after 1 hour
            .query_async(conn)
            .await;

        match result {
            Ok(true) => {
                info!(
                    "Claimed job {} (worker={})",
                    job_id, self.worker_id
                );
                true
            }
            Ok(false) => {
                info!(
                    "Job {} already claimed by another worker, skipping",
                    job_id
                );
                false
            }
            Err(e) => {
                warn!("Failed to claim job {} via Redis: {}, proceeding anyway", job_id, e);
                true // Fail open — better to double-scan than miss a scan
            }
        }
    }

    /// Execute a scan request and return results
    ///
    /// Each entry in `series_paths` is a `(series_id, path)` pair — the dispatcher
    /// keeps these aligned so the worker can tag each result with the correct series.
    /// For `RescanMovie`, series_id is actually movie_id (reused field).
    ///
    /// `known_files` contains DB metadata from the server. Files whose path+size match
    /// a known entry skip FFmpeg probe + BLAKE3 hash — a massive speedup for unchanged libraries.
    #[cfg(feature = "redis-events")]
    async fn execute_scan(
        &self,
        job_id: &str,
        scan_type: &ScanType,
        series_paths: &[(i64, &String)],
        known_files: &HashMap<String, KnownFileInfo>,
        event_bus: &crate::core::messaging::HybridEventBus,
    ) -> Vec<Message> {
        let mut results = Vec::new();
        let mut total_files_found: u64 = 0;

        match scan_type {
            ScanType::RescanSeries => {
                let mut skipped_count: usize = 0;
                for &(series_id, path_str) in series_paths {
                    let path = PathBuf::from(path_str);
                    let media_type = infer_media_type(&path);

                    if !path.exists() {
                        warn!("[scan][{}] Path does not exist: {}", media_type, path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![format!("Path does not exist: {}", path_str)],
                        });
                        continue;
                    }

                    let scan_start = std::time::Instant::now();
                    info!("[scan][{}] Scanning {} (id={})", media_type, path_str, series_id);
                    let mut files = scanner::scan_series_directory(&path);
                    let file_count = files.len();
                    info!("[scan][{}] Found {} video file(s) in {}", media_type, file_count, path_str);

                    // Publish initial scanning progress
                    publish_progress(event_bus, job_id, &self.worker_id, "scanning", None, file_count, 0, 0, None).await;

                    // Each file = 2 steps (probe + hash); skipped files count as 2 steps instantly
                    let total_steps = file_count * 2;
                    let mut completed_steps: usize = 0;

                    // Enrich each file with FFmpeg probe + BLAKE3 hash (LOCAL disk = fast)
                    // Skip files whose path+size match known DB records (unchanged files)
                    for (idx, file) in files.iter_mut().enumerate() {
                        let path_key = file.path.to_string_lossy().to_string();
                        if let Some(known) = known_files.get(&path_key) {
                            if known.size == file.size && known.file_hash.is_some() {
                                file.media_info = known.media_info.clone();
                                file.quality = known.quality.clone();
                                file.file_hash = known.file_hash.clone();
                                info!(
                                    "[skip] ({}/{}) {} — unchanged ({})",
                                    idx + 1, file_count, file.filename, format_size(file.size as u64)
                                );
                                skipped_count += 1;
                                completed_steps += 2;
                                let pct = if total_steps > 0 { (completed_steps * 100 / total_steps).min(100) as u8 } else { 100 };
                                publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), file_count, idx + 1, pct, Some("unchanged".to_string())).await;
                                continue;
                            }
                        }

                        // Probe stage
                        let probe_pct = if total_steps > 0 { (completed_steps * 100 / total_steps).min(100) as u8 } else { 0 };
                        publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), file_count, idx, probe_pct, None).await;

                        let probe_detail = probe_scanned_file(file, (idx + 1, file_count)).await;
                        completed_steps += 1;

                        let after_probe_pct = if total_steps > 0 { (completed_steps * 100 / total_steps).min(100) as u8 } else { 50 };
                        publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), file_count, idx, after_probe_pct, probe_detail).await;

                        // Hash stage
                        publish_progress(event_bus, job_id, &self.worker_id, "hashing", Some(&file.filename), file_count, idx, after_probe_pct, None).await;

                        hash_scanned_file(file, (idx + 1, file_count)).await;
                        completed_steps += 1;

                        let after_hash_pct = if total_steps > 0 { (completed_steps * 100 / total_steps).min(100) as u8 } else { 100 };
                        publish_progress(event_bus, job_id, &self.worker_id, "hashing", Some(&file.filename), file_count, idx + 1, after_hash_pct, None).await;
                    }

                    let elapsed = scan_start.elapsed();
                    total_files_found += file_count as u64;
                    info!(
                        "[scan][{}] Complete — {} file(s) ({} skipped, {} enriched) in {:.1}s (id={})",
                        media_type, file_count, skipped_count, file_count - skipped_count,
                        elapsed.as_secs_f64(), series_id
                    );

                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id,
                        worker_id: self.worker_id.clone(),
                        files_found: files,
                        errors: vec![],
                    });
                }
            }
            ScanType::RescanMovie => {
                for &(movie_id, path_str) in series_paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        warn!("[scan][movie] Path does not exist: {}", path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: movie_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![format!("Path does not exist: {}", path_str)],
                        });
                        continue;
                    }

                    let scan_start = std::time::Instant::now();
                    info!("[scan][movie] Scanning {} (id={})", path_str, movie_id);
                    if let Some(mut file) = scanner::scan_movie_directory(&path) {
                        // Check known_files for skip-enrichment
                        let path_key = file.path.to_string_lossy().to_string();
                        if let Some(known) = known_files.get(&path_key) {
                            if known.size == file.size && known.file_hash.is_some() {
                                file.media_info = known.media_info.clone();
                                file.quality = known.quality.clone();
                                file.file_hash = known.file_hash.clone();
                                let elapsed = scan_start.elapsed();
                                info!(
                                    "[skip][movie] {} — unchanged ({}) in {:.1}s",
                                    file.filename, format_size(file.size as u64), elapsed.as_secs_f64()
                                );
                                publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), 1, 1, 100, Some("unchanged".to_string())).await;
                                total_files_found += 1;
                                results.push(Message::ScanResult {
                                    job_id: job_id.to_string(),
                                    series_id: movie_id,
                                    worker_id: self.worker_id.clone(),
                                    files_found: vec![file],
                                    errors: vec![],
                                });
                                continue;
                            }
                        }

                        // Probe stage
                        publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), 1, 0, 0, None).await;
                        let probe_detail = probe_scanned_file(&mut file, (1, 1)).await;
                        publish_progress(event_bus, job_id, &self.worker_id, "probing", Some(&file.filename), 1, 0, 50, probe_detail).await;

                        // Hash stage
                        publish_progress(event_bus, job_id, &self.worker_id, "hashing", Some(&file.filename), 1, 0, 50, None).await;
                        hash_scanned_file(&mut file, (1, 1)).await;
                        publish_progress(event_bus, job_id, &self.worker_id, "hashing", Some(&file.filename), 1, 1, 100, None).await;

                        total_files_found += 1;
                        let elapsed = scan_start.elapsed();
                        info!(
                            "[scan][movie] Complete — {} ({}, hash={}) in {:.1}s",
                            file.filename,
                            format_size(file.size as u64),
                            file.file_hash.as_deref().unwrap_or("none"),
                            elapsed.as_secs_f64()
                        );
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: movie_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![file],
                            errors: vec![],
                        });
                    } else {
                        info!("[scan][movie] No video files found in {}", path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: movie_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![],
                        });
                    }
                }
            }
            ScanType::RescanPodcast => {
                for &(podcast_id, path_str) in series_paths {
                    info!(
                        "[scan][podcast] Podcast scanning not yet implemented (id={}, path={})",
                        podcast_id, path_str
                    );
                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id: podcast_id,
                        worker_id: self.worker_id.clone(),
                        files_found: vec![],
                        errors: vec!["Podcast scanning not yet implemented".to_string()],
                    });
                }
            }
            ScanType::RescanMusic => {
                for &(music_id, path_str) in series_paths {
                    info!(
                        "[scan][music] Music scanning not yet implemented (id={}, path={})",
                        music_id, path_str
                    );
                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id: music_id,
                        worker_id: self.worker_id.clone(),
                        files_found: vec![],
                        errors: vec!["Music scanning not yet implemented".to_string()],
                    });
                }
            }
            ScanType::DownloadedEpisodesScan => {
                for &(series_id, path_str) in series_paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        warn!("[scan][download] Path does not exist, skipping: {}", path_str);
                        continue;
                    }

                    let scan_start = std::time::Instant::now();
                    info!("[scan][download] Scanning {}", path_str);
                    let mut files = scanner::scan_download_directory(&path);
                    let file_count = files.len();
                    info!("[scan][download] Found {} video file(s) in {}", file_count, path_str);

                    for (idx, file) in files.iter_mut().enumerate() {
                        enrich_scanned_file(file, (idx + 1, file_count)).await;
                    }

                    let elapsed = scan_start.elapsed();
                    total_files_found += file_count as u64;
                    info!(
                        "[scan][download] Complete — {} file(s) enriched in {:.1}s",
                        file_count, elapsed.as_secs_f64()
                    );

                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id,
                        worker_id: self.worker_id.clone(),
                        files_found: files,
                        errors: vec![],
                    });
                }
            }
        }

        // Record statistics
        self.record_scan(total_files_found);

        results
    }
}

/// Result of a file import operation
struct MoveResult {
    size: i64,
    method: &'static str, // "copy"
}

/// Copy a file from source to dest, preserving the original.
///
/// Always copies — source files are never moved or deleted. The download
/// client manages source cleanup via its own retention/seeding rules.
fn move_file(source: &std::path::Path, dest: &std::path::Path) -> std::io::Result<MoveResult> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let source_size = std::fs::metadata(source)?.len();
    copy_with_progress(source, dest, source_size)?;
    let size = std::fs::metadata(dest)?.len() as i64;
    Ok(MoveResult { size, method: "copy" })
}

/// Copy a file with periodic progress logging (every 10% or 500MB, whichever comes first)
fn copy_with_progress(
    source: &std::path::Path,
    dest: &std::path::Path,
    total_size: u64,
) -> std::io::Result<()> {
    use std::io::{Read, Write};

    let src_file = std::fs::File::open(source)?;
    let dst_file = std::fs::File::create(dest)?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, src_file); // 1MB buffer
    let mut writer = std::io::BufWriter::with_capacity(1 << 20, dst_file);

    let mut copied: u64 = 0;
    let mut last_logged_pct: u64 = 0;
    let mut buf = vec![0u8; 1 << 20]; // 1MB chunks
    let src_name = source.file_name().unwrap_or_default().to_string_lossy();

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        copied += n as u64;

        if total_size > 0 {
            let pct = (copied * 100) / total_size;
            // Log every 10%
            if pct >= last_logged_pct + 10 {
                last_logged_pct = pct - (pct % 10);
                info!(
                    "[copy] {} — {}% ({}/{})",
                    src_name,
                    last_logged_pct,
                    format_size(copied),
                    format_size(total_size)
                );
            }
        }
    }

    writer.flush()?;
    Ok(())
}

/// Format bytes into a human-readable string (e.g., "1.5 GB", "340 MB")
fn format_size(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1_024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Infer media category from a file path based on known library directory names
fn infer_media_type(path: &std::path::Path) -> &'static str {
    let path_str = path.to_string_lossy();
    let lower = path_str.to_lowercase();

    if lower.contains("/movies") || lower.contains("/film") {
        "movie"
    } else if lower.contains("/anime") {
        "anime"
    } else if lower.contains("/shows") || lower.contains("/tv") || lower.contains("/series") {
        "show"
    } else if lower.contains("/podcast") {
        "podcast"
    } else if lower.contains("/music") {
        "music"
    } else {
        "media"
    }
}

/// Publish a scan progress update via the event bus
#[cfg(feature = "redis-events")]
async fn publish_progress(
    event_bus: &crate::core::messaging::HybridEventBus,
    job_id: &str,
    worker_id: &str,
    stage: &str,
    current_file: Option<&str>,
    files_total: usize,
    files_processed: usize,
    percent: u8,
    detail: Option<String>,
) {
    event_bus
        .publish(Message::ScanProgress {
            job_id: job_id.to_string(),
            worker_id: worker_id.to_string(),
            stage: stage.to_string(),
            current_file: current_file.map(|s| s.to_string()),
            files_total,
            files_processed,
            percent,
            detail,
            entity_ids: vec![],
            scan_type: None,
        })
        .await;
}

/// Extract a human-readable media detail string from a ScannedFile's media_info JSON.
/// Returns something like "1080p x265 HDR10" or None if no useful info.
fn extract_media_detail(file: &ScannedFile) -> Option<String> {
    let info_str = file.media_info.as_deref()?;
    let info: serde_json::Value = serde_json::from_str(info_str).ok()?;
    let resolution = info["resolution"].as_str().unwrap_or("?");
    let codec = info["videoCodec"].as_str()
        .or_else(|| info["video_codec"].as_str())
        .unwrap_or("?");
    let mut parts = vec![format!("{}p", resolution), codec.to_string()];
    // HDR detection
    if let Some(ct) = info["videoColourTransfer"].as_str()
        .or_else(|| info["video_colour_transfer"].as_str())
    {
        if ct == "smpte2084" {
            parts.push("HDR10".to_string());
        } else if ct == "arib-std-b67" {
            parts.push("HLG".to_string());
        }
    }
    Some(parts.join(" "))
}

/// Probe a scanned file with FFmpeg (sets media_info + quality).
/// Returns the probe detail string for progress reporting.
async fn probe_scanned_file(file: &mut ScannedFile, progress: (usize, usize)) -> Option<String> {
    let path = std::path::Path::new(&file.path);
    let (current, total) = progress;

    #[cfg(feature = "media-probe")]
    {
        use crate::core::mediafiles::{derive_quality_from_media, MediaAnalyzer};
        let probe_start = std::time::Instant::now();
        info!(
            "[probe] ({}/{}) {} ({})",
            current, total, file.filename, format_size(file.size as u64)
        );
        match MediaAnalyzer::analyze(path).await {
            Ok(info) => {
                let elapsed = probe_start.elapsed();
                let quality = derive_quality_from_media(&info, &file.filename);
                let resolution = info.resolution.as_deref().unwrap_or("?");
                let codec = info.video_codec.as_deref().unwrap_or("?");
                info!(
                    "[probe] ({}/{}) {} — {}p {} in {:.1}s",
                    current, total, file.filename, resolution, codec, elapsed.as_secs_f64()
                );
                file.quality = serde_json::to_string(&quality).ok();
                file.media_info = serde_json::to_string(&info).ok();
                return extract_media_detail(file);
            }
            Err(e) => {
                let elapsed = probe_start.elapsed();
                warn!(
                    "[probe] ({}/{}) {} — failed in {:.1}s: {}",
                    current, total, file.filename, elapsed.as_secs_f64(), e
                );
            }
        }
    }

    #[cfg(not(feature = "media-probe"))]
    {
        let _ = (path, current, total);
    }

    None
}

/// Hash a scanned file with BLAKE3 (sets file_hash).
async fn hash_scanned_file(file: &mut ScannedFile, progress: (usize, usize)) {
    use crate::core::mediafiles::compute_file_hash;

    let path = std::path::Path::new(&file.path);
    let (current, total) = progress;

    let hash_start = std::time::Instant::now();
    info!("[hash] ({}/{}) {}", current, total, file.filename);
    match compute_file_hash(path).await {
        Ok(hash) => {
            let elapsed = hash_start.elapsed();
            let rate = if elapsed.as_secs_f64() > 0.0 {
                format_size((file.size as f64 / elapsed.as_secs_f64()) as u64)
            } else {
                "instant".to_string()
            };
            info!(
                "[hash] ({}/{}) {} — {} in {:.1}s ({}/s)",
                current, total, file.filename, &hash[..12], elapsed.as_secs_f64(), rate
            );
            file.file_hash = Some(hash);
        }
        Err(e) => {
            warn!(
                "[hash] ({}/{}) {} — failed: {}",
                current, total, file.filename, e
            );
        }
    }
}

/// Enrich a scanned file with FFmpeg media info and BLAKE3 content hash.
///
/// Convenience wrapper that calls probe + hash sequentially.
/// Used by code paths that don't need per-stage progress reporting.
async fn enrich_scanned_file(file: &mut ScannedFile, progress: (usize, usize)) {
    probe_scanned_file(file, progress).await;
    hash_scanned_file(file, progress).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_path() {
        let worker = WorkerRunner {
            worker_id: "test".to_string(),
            worker_paths: vec![PathBuf::from("/media/tv"), PathBuf::from("/media/anime")],
            redis_url: "redis://localhost".to_string(),
            #[cfg(feature = "redis-events")]
            redis_conn: tokio::sync::Mutex::new(None),
            scan_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            scans_completed: std::sync::atomic::AtomicU64::new(0),
            files_found: std::sync::atomic::AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        };

        assert!(worker.handles_path("/media/tv"));
        assert!(worker.handles_path("/media/tv/Show Name"));
        assert!(worker.handles_path("/media/anime/Anime Name/Season 1"));
        assert!(!worker.handles_path("/other/path"));
        assert!(!worker.handles_path("/media/movies")); // Different path
    }
}
