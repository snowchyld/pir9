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

use crate::core::messaging::ImportFileResult;

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
        Message::ProbeFileRequest { .. } => "ProbeFileRequest",
        Message::ProbeFileResult { .. } => "ProbeFileResult",
        Message::HashFileRequest { .. } => "HashFileRequest",
        Message::HashFileResult { .. } => "HashFileResult",
        Message::DeletePathsRequest { .. } => "DeletePathsRequest",
        Message::DeletePathsResult { .. } => "DeletePathsResult",
        Message::RenameFilesRequest { .. } => "RenameFilesRequest",
        Message::RenameFilesResult { .. } => "RenameFilesResult",
        Message::PodcastAdded { .. } => "PodcastAdded",
        Message::PodcastUpdated { .. } => "PodcastUpdated",
        Message::PodcastDeleted { .. } => "PodcastDeleted",
        Message::PodcastRefreshed { .. } => "PodcastRefreshed",
        Message::ArtistAdded { .. } => "ArtistAdded",
        Message::ArtistUpdated { .. } => "ArtistUpdated",
        Message::ArtistDeleted { .. } => "ArtistDeleted",
        Message::ArtistRefreshed { .. } => "ArtistRefreshed",
        Message::TrackFileImported { .. } => "TrackFileImported",
        Message::TrackFileDeleted { .. } => "TrackFileDeleted",
    }
}

/// Worker runner that handles scan requests from Redis Streams
pub struct WorkerRunner {
    /// Worker's unique ID
    worker_id: String,
    /// Paths this worker is responsible for scanning
    worker_paths: Vec<PathBuf>,
    /// Redis URL for event bus
    redis_url: String,
    /// Dedicated Redis connection for stream operations (XREADGROUP BLOCK + XACK)
    #[cfg(feature = "redis-events")]
    stream_conn: tokio::sync::Mutex<Option<redis::aio::ConnectionManager>>,
    /// Single permit — worker accepts only ONE job at a time (discovery, probe, hash,
    /// import, or delete). If busy, incoming requests are not dequeued from the stream.
    job_semaphore: Arc<tokio::sync::Semaphore>,
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
            stream_conn: tokio::sync::Mutex::new(None),
            job_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
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

        // Ensure streams and consumer groups exist
        event_bus
            .ensure_streams()
            .await
            .context("Failed to initialize Redis streams")?;

        // Create a dedicated Redis connection for stream operations (XREADGROUP BLOCK + XACK)
        {
            let client = redis::Client::open(this.redis_url.as_str())
                .context("Failed to create Redis client for stream")?;
            let conn = redis::aio::ConnectionManager::new(client)
                .await
                .context("Failed to connect to Redis for stream")?;
            *this.stream_conn.lock().await = Some(conn);
        }

        // Recover any stale jobs from crashed workers on startup
        this.recover_stale_jobs().await;

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

        // Poll interval for stream read. Non-blocking XREADGROUP + interval timer
        // avoids MultiplexedConnection desync from BLOCK commands.
        let mut stream_poll = tokio::time::interval(tokio::time::Duration::from_secs(1));
        stream_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Periodic stale job recovery (every 60s)
        let mut autoclaim_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        autoclaim_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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
                // Periodic XAUTOCLAIM to recover stale jobs from crashed workers
                _ = autoclaim_interval.tick() => {
                    if this.job_semaphore.available_permits() > 0 {
                        this.recover_stale_jobs().await;
                    }
                }
                // Stream-based job dequeue — only read when idle (semaphore available)
                _ = stream_poll.tick() => {
                    if this.job_semaphore.available_permits() > 0 {
                        if let Some((stream_id, message)) = this.read_job_from_stream().await {
                            this.handle_stream_job(stream_id, message, &event_bus).await;
                        }
                    }
                }
                // Pub/sub for ephemeral broadcast events (heartbeats, worker online/offline)
                result = receiver.recv() => {
                    match result {
                        Ok(message) => {
                            this.handle_pubsub_message(message, &event_bus).await;
                        }
                        Err(e) => {
                            error!("Error receiving pub/sub message: {}", e);
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

    /// Handle a job message dequeued from the Redis job stream.
    /// The stream_id is ACK'd after successful processing.
    /// If the worker can't handle the job (wrong path), it re-enqueues and ACKs.
    #[cfg(feature = "redis-events")]
    async fn handle_stream_job(
        self: &Arc<Self>,
        stream_id: String,
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

                let relevant: Vec<(i64, String)> = series_ids
                    .iter()
                    .zip(paths.iter())
                    .filter(|(_, p)| self.handles_path(p))
                    .map(|(&sid, p)| (sid, p.clone()))
                    .collect();

                if relevant.is_empty() {
                    debug!("Scan request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                // ACK the stream entry — we've claimed this job
                self.ack_job(&stream_id).await;

                let this = Arc::clone(self);
                let event_bus = Arc::clone(event_bus);
                let job_id = job_id.clone();
                let scan_type = *scan_type;

                tokio::spawn(async move {
                    let mut total_files: u64 = 0;
                    let scan_start = std::time::Instant::now();

                    for (entity_id, path_str) in &relevant {
                        let path = PathBuf::from(path_str);
                        if !path.exists() {
                            warn!("[discover] Path does not exist: {}", path_str);
                            event_bus.publish(Message::ScanResult {
                                job_id: job_id.clone(),
                                series_id: *entity_id,
                                worker_id: this.worker_id.clone(),
                                files_found: vec![],
                                errors: vec![format!("Path does not exist: {}", path_str)],
                            }).await;
                            continue;
                        }

                        let media_type = infer_media_type(&path);
                        let files: Vec<ScannedFile> = match scan_type {
                            ScanType::RescanSeries => {
                                info!("[discover][{}] Listing {} (id={})", media_type, path_str, entity_id);
                                scanner::scan_series_directory(&path)
                            }
                            ScanType::RescanMovie => {
                                info!("[discover][movie] Listing {} (id={})", path_str, entity_id);
                                scanner::scan_movie_directory(&path)
                                    .into_iter()
                                    .collect()
                            }
                            ScanType::DownloadedEpisodesScan | ScanType::DownloadedMovieScan => {
                                info!("[discover][download] Listing {}", path_str);
                                scanner::scan_download_directory(&path)
                            }
                            _ => vec![],
                        };

                        let file_count = files.len();
                        total_files += file_count as u64;
                        info!("[discover] {} — {} file(s) (id={})", media_type, file_count, entity_id);

                        event_bus.publish(Message::ScanResult {
                            job_id: job_id.clone(),
                            series_id: *entity_id,
                            worker_id: this.worker_id.clone(),
                            files_found: files,
                            errors: vec![],
                        }).await;
                    }

                    let elapsed = scan_start.elapsed();
                    info!("[discover] Job complete — {} file(s) in {:.1}s", total_files, elapsed.as_secs_f64());
                    this.record_scan(total_files);
                });
            }
            Message::ProbeFileRequest {
                job_id,
                parent_job_id,
                file_path,
                entity_id,
                scan_type,
            } => {
                if !self.handles_path(file_path) {
                    debug!("Probe request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                self.ack_job(&stream_id).await;

                let this = Arc::clone(self);
                let event_bus = Arc::clone(event_bus);
                let job_id = job_id.clone();
                let parent_job_id = parent_job_id.clone();
                let file_path = file_path.clone();
                let entity_id = *entity_id;
                let _scan_type = *scan_type;

                tokio::spawn(async move {
                    let filename = std::path::Path::new(&file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    info!("[probe] {} (entity={})", filename, entity_id);
                    let path = std::path::Path::new(&file_path);

                    let (media_info, quality) = if path.exists() {
                        let media_info_result = crate::core::mediafiles::MediaAnalyzer::analyze(path).await;
                        let media_info = media_info_result
                            .as_ref()
                            .ok()
                            .and_then(|info| serde_json::to_string(info).ok());
                        let quality = match &media_info_result {
                            Ok(info) => {
                                let q = crate::core::mediafiles::derive_quality_from_media(info, &filename);
                                serde_json::to_string(&q).ok()
                            }
                            Err(_) => None,
                        };
                        (media_info, quality)
                    } else {
                        warn!("[probe] File not found: {}", file_path);
                        (None, None)
                    };

                    event_bus.publish(Message::ProbeFileResult {
                        job_id,
                        parent_job_id,
                        file_path,
                        entity_id,
                        worker_id: this.worker_id.clone(),
                        media_info,
                        quality,
                    }).await;
                });
            }
            Message::HashFileRequest {
                job_id,
                parent_job_id,
                file_path,
                entity_id,
            } => {
                if !self.handles_path(file_path) {
                    debug!("Hash request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                self.ack_job(&stream_id).await;

                let this = Arc::clone(self);
                let event_bus = Arc::clone(event_bus);
                let job_id = job_id.clone();
                let parent_job_id = parent_job_id.clone();
                let file_path = file_path.clone();
                let entity_id = *entity_id;

                tokio::spawn(async move {
                    let filename = std::path::Path::new(&file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    info!("[hash] {} (entity={})", filename, entity_id);
                    let path = std::path::Path::new(&file_path);

                    let file_hash = if path.exists() {
                        crate::core::mediafiles::compute_file_hash(path).await.ok()
                    } else {
                        warn!("[hash] File not found: {}", file_path);
                        None
                    };

                    event_bus.publish(Message::HashFileResult {
                        job_id,
                        parent_job_id,
                        file_path,
                        entity_id,
                        worker_id: this.worker_id.clone(),
                        file_hash,
                    }).await;
                });
            }
            Message::ImportFilesRequest { job_id, files } => {
                let total = files.len();
                info!("[import] Starting import job {} — {} file(s)", job_id, total);

                let any_ours = files.iter().any(|f| self.handles_path(&f.source_path.to_string_lossy()));
                if !any_ours {
                    debug!("Import files request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                self.ack_job(&stream_id).await;

                let mut results = Vec::new();
                let started = std::time::Instant::now();

                publish_progress(event_bus, job_id, &self.worker_id, "copying", None, total, 0, 0.0, None).await;

                for (idx, spec) in files.iter().enumerate() {
                    let media_type = infer_media_type(&spec.dest_path);
                    let filename = spec.source_path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    info!(
                        "[import][{}] ({}/{}) {} — {} -> {}",
                        media_type, idx + 1, total, filename,
                        spec.source_path.display(), spec.dest_path.display()
                    );

                    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<(u64, u64)>();
                    let progress_job_id = job_id.to_string();
                    let progress_worker_id = self.worker_id.clone();
                    let progress_filename = filename.clone();
                    let progress_bus = Arc::clone(event_bus);
                    let progress_file_idx = idx;
                    let progress_total = total;

                    let progress_handle = tokio::spawn(async move {
                        let mut last_reported_pct: f32 = -1.0;
                        let mut disconnected = false;
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            let mut latest: Option<(u64, u64)> = None;
                            loop {
                                match progress_rx.try_recv() {
                                    Ok(val) => latest = Some(val),
                                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                        disconnected = true;
                                        break;
                                    }
                                }
                            }
                            if let Some((bytes_copied, bytes_total_size)) = latest {
                                let file_pct = if bytes_total_size > 0 {
                                    bytes_copied as f32 * 100.0 / bytes_total_size as f32
                                } else {
                                    0.0
                                };
                                let overall_pct = (progress_file_idx as f32 + file_pct / 100.0)
                                    * 100.0 / progress_total as f32;
                                if (overall_pct - last_reported_pct).abs() >= 0.1 {
                                    last_reported_pct = overall_pct;
                                    publish_progress_bytes(
                                        &progress_bus, &progress_job_id, &progress_worker_id,
                                        "copying", Some(&progress_filename), progress_total,
                                        progress_file_idx, overall_pct.min(100.0), None,
                                        Some(bytes_copied), Some(bytes_total_size),
                                    ).await;
                                }
                            }
                            if disconnected { break; }
                        }
                    });

                    let src = spec.source_path.clone();
                    let dst = spec.dest_path.clone();
                    let file_start = std::time::Instant::now();
                    let copy_result = tokio::task::spawn_blocking(move || {
                        move_file(&src, &dst, Some(progress_tx))
                    }).await;

                    let _ = progress_handle.await;

                    let result = match copy_result {
                        Ok(Ok(mr)) => {
                            let elapsed = file_start.elapsed();
                            info!(
                                "[{}][{}] ({}/{}) {} — {} in {:.1}s",
                                mr.method, media_type, idx + 1, total, filename,
                                format_size(mr.size as u64), elapsed.as_secs_f64()
                            );
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: true, file_size: mr.size, error: None,
                            }
                        }
                        Ok(Err(e)) => {
                            error!("[error][{}] ({}/{}) {} — copy failed: {}", media_type, idx + 1, total, filename, e);
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: false, file_size: 0, error: Some(e.to_string()),
                            }
                        }
                        Err(e) => {
                            error!("[error][{}] ({}/{}) {} — spawn_blocking panicked: {}", media_type, idx + 1, total, filename, e);
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: false, file_size: 0, error: Some(format!("internal error: {}", e)),
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
                    job_id, succeeded, total, format_size(total_bytes as u64), elapsed.as_secs_f64()
                );
                self.record_scan(succeeded as u64);

                event_bus.publish(Message::ImportFilesResult {
                    job_id: job_id.clone(),
                    worker_id: self.worker_id.clone(),
                    results,
                }).await;
            }
            Message::DeletePathsRequest { job_id, paths, recursive } => {
                let total = paths.len();
                info!("[delete] Starting delete job {} — {} path(s), recursive={}", job_id, total, recursive);

                let any_ours = paths.iter().any(|p| self.handles_path(p));
                if !any_ours {
                    debug!("Delete paths request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                self.ack_job(&stream_id).await;

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
                        Ok(())
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "Path is a directory but recursive=false"))
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

                event_bus.publish(Message::DeletePathsResult {
                    job_id: job_id.clone(),
                    worker_id: self.worker_id.clone(),
                    results,
                }).await;
            }
            Message::RenameFilesRequest { job_id, files, episode_file_ids: _ } => {
                let total = files.len();
                info!("[rename] Starting rename job {} — {} file(s)", job_id, total);

                let any_ours = files.iter().any(|f| self.handles_path(&f.source_path.to_string_lossy()));
                if !any_ours {
                    debug!("Rename request not for our paths, re-enqueuing");
                    event_bus.enqueue_job(message).await;
                    self.ack_job(&stream_id).await;
                    return;
                }

                self.ack_job(&stream_id).await;

                let mut results = Vec::new();
                for (idx, spec) in files.iter().enumerate() {
                    let src = &spec.source_path;
                    let dst = &spec.dest_path;
                    let name = src.file_name().unwrap_or_default().to_string_lossy();

                    // Ensure target directory exists
                    if let Some(parent) = dst.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            error!("[rename] ({}/{}) Failed to create dir for {}: {}", idx + 1, total, name, e);
                            results.push(ImportFileResult {
                                source_path: src.clone(),
                                dest_path: dst.clone(),
                                success: false,
                                file_size: 0,
                                error: Some(format!("Failed to create directory: {}", e)),
                            });
                            continue;
                        }
                    }

                    if !src.exists() && dst.exists() {
                        // Already renamed (idempotent)
                        let size = std::fs::metadata(dst).map(|m| m.len() as i64).unwrap_or(0);
                        info!("[rename] ({}/{}) {} — already at destination", idx + 1, total, name);
                        results.push(ImportFileResult {
                            source_path: src.clone(),
                            dest_path: dst.clone(),
                            success: true,
                            file_size: size,
                            error: None,
                        });
                        continue;
                    }

                    match std::fs::rename(src, dst) {
                        Ok(()) => {
                            let size = std::fs::metadata(dst).map(|m| m.len() as i64).unwrap_or(0);
                            info!("[rename] ({}/{}) {} → {}", idx + 1, total, name, dst.display());
                            results.push(ImportFileResult {
                                source_path: src.clone(),
                                dest_path: dst.clone(),
                                success: true,
                                file_size: size,
                                error: None,
                            });
                        }
                        Err(e) => {
                            error!("[rename] ({}/{}) {} — failed: {}", idx + 1, total, name, e);
                            results.push(ImportFileResult {
                                source_path: src.clone(),
                                dest_path: dst.clone(),
                                success: false,
                                file_size: 0,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }

                event_bus.publish(Message::RenameFilesResult {
                    job_id: job_id.clone(),
                    worker_id: self.worker_id.clone(),
                    results,
                }).await;
            }
            other => {
                // Unexpected message type from job stream — ACK and discard
                warn!("Unexpected message type from job stream: {}", message_type_name(other));
                self.ack_job(&stream_id).await;
            }
        }
    }

    /// Handle ephemeral pub/sub messages (heartbeats, worker online/offline)
    #[cfg(feature = "redis-events")]
    async fn handle_pubsub_message(
        self: &Arc<Self>,
        message: Message,
        _event_bus: &Arc<crate::core::messaging::HybridEventBus>,
    ) {
        match &message {
            // Ignore our own heartbeats and status messages
            Message::WorkerHeartbeat { worker_id, .. } if worker_id == &self.worker_id => {}
            Message::WorkerOnline { worker_id, .. } if worker_id == &self.worker_id => {}
            Message::WorkerOffline { worker_id } if worker_id == &self.worker_id => {}
            // Log other worker events
            Message::WorkerHeartbeat { worker_id, .. } => {
                trace!("Other worker heartbeat: {}", worker_id);
            }
            Message::WorkerOnline { worker_id, paths } => {
                info!("Worker came online: {} with paths {:?}", worker_id, paths);
            }
            Message::WorkerOffline { worker_id } => {
                info!("Worker went offline: {}", worker_id);
            }
            // Ignore server-bound messages
            Message::ScanResult { .. } | Message::ScanProgress { .. }
            | Message::ImportFilesResult { .. } | Message::DeletePathsResult { .. }
            | Message::ProbeFileResult { .. } | Message::HashFileResult { .. } => {}
            other => {
                trace!("Ignoring pub/sub message: {}", message_type_name(other));
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

    /// Read a job from the Redis job stream (XREADGROUP).
    /// Uses the dedicated stream_conn. Returns (stream_entry_id, message) or None.
    ///
    /// First drains the consumer's PEL (pending entries from prior runs or
    /// XAUTOCLAIM), then reads new entries with `>`. This ensures entries
    /// aren't stuck in the PEL forever after worker restarts.
    #[cfg(feature = "redis-events")]
    async fn read_job_from_stream(&self) -> Option<(String, Message)> {
        use crate::core::messaging::redis_bus::RedisEventBus;
        use crate::core::messaging::{REDIS_JOB_STREAM, REDIS_WORKER_GROUP};

        let mut guard = self.stream_conn.lock().await;
        let conn = match guard.as_mut() {
            Some(c) => c,
            None => {
                warn!("No Redis stream connection for XREADGROUP");
                return None;
            }
        };

        // First: drain any entries already in our PEL (from prior runs, restarts, or XAUTOCLAIM).
        // XREADGROUP with "0" returns pending entries without blocking.
        let pending = RedisEventBus::read_pending_entries(
            conn,
            REDIS_JOB_STREAM,
            REDIS_WORKER_GROUP,
            &self.worker_id,
            1,
        )
        .await;

        if let Some(entry) = pending.into_iter().next() {
            debug!("Processing pending entry from PEL: {}", entry.0);
            return Some(entry);
        }

        // PEL is empty — poll for new entries (non-blocking)
        let entries = RedisEventBus::read_stream_entries(
            conn,
            REDIS_JOB_STREAM,
            REDIS_WORKER_GROUP,
            &self.worker_id,
            1, // one job at a time
        )
        .await;

        entries.into_iter().next()
    }

    /// ACK a job stream entry after processing
    #[cfg(feature = "redis-events")]
    async fn ack_job(&self, stream_id: &str) {
        use crate::core::messaging::redis_bus::RedisEventBus;
        use crate::core::messaging::{REDIS_JOB_STREAM, REDIS_WORKER_GROUP};

        let mut guard = self.stream_conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            RedisEventBus::ack_stream_entry(conn, REDIS_JOB_STREAM, REDIS_WORKER_GROUP, stream_id)
                .await;
        }
    }

    /// Recover stale jobs from crashed workers via XAUTOCLAIM.
    ///
    /// XAUTOCLAIM transfers entries idle > threshold from dead consumers
    /// to this consumer's PEL. The `read_job_from_stream()` method drains
    /// the PEL before reading new entries, so reclaimed jobs will be
    /// picked up on the next poll cycle automatically.
    #[cfg(feature = "redis-events")]
    async fn recover_stale_jobs(&self) {
        use crate::core::messaging::redis_bus::RedisEventBus;
        use crate::core::messaging::{REDIS_JOB_STREAM, REDIS_WORKER_GROUP};

        let mut guard = self.stream_conn.lock().await;
        let conn = match guard.as_mut() {
            Some(c) => c,
            None => return,
        };

        let entries = RedisEventBus::autoclaim_stale(
            conn,
            REDIS_JOB_STREAM,
            REDIS_WORKER_GROUP,
            &self.worker_id,
            300_000, // 5 minute idle threshold
        )
        .await;

        if !entries.is_empty() {
            info!(
                "XAUTOCLAIM recovered {} stale job(s) — will process from PEL on next poll",
                entries.len()
            );
        }
        // Entries are now in our PEL. read_job_from_stream() checks PEL first
        // (using "0"), so they'll be picked up and processed normally.
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
///
/// Sends `(bytes_copied, total_size)` through the optional channel after each 1MB chunk.
fn move_file(
    source: &std::path::Path,
    dest: &std::path::Path,
    progress_tx: Option<std::sync::mpsc::Sender<(u64, u64)>>,
) -> std::io::Result<MoveResult> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let source_size = std::fs::metadata(source)?.len();
    copy_with_progress(source, dest, source_size, progress_tx)?;
    let size = std::fs::metadata(dest)?.len() as i64;
    Ok(MoveResult { size, method: "copy" })
}

/// Copy a file with periodic progress logging and byte-level progress reporting.
///
/// Sends `(bytes_copied, total_size)` through the channel after each 1MB chunk,
/// letting the async receiver calculate percentage and publish to Redis.
fn copy_with_progress(
    source: &std::path::Path,
    dest: &std::path::Path,
    total_size: u64,
    progress_tx: Option<std::sync::mpsc::Sender<(u64, u64)>>,
) -> std::io::Result<()> {
    use std::io::{Read, Write};

    let src_file = std::fs::File::open(source)?;
    let dst_file = std::fs::File::create(dest)?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, src_file); // 1MB buffer
    let mut writer = std::io::BufWriter::with_capacity(1 << 20, dst_file);

    let mut copied: u64 = 0;
    let mut last_logged_pct: f32 = -1.0;
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
            let pct = copied as f64 * 100.0 / total_size as f64;
            // Send bytes to channel on every chunk (non-blocking — drops if receiver is behind)
            if let Some(ref tx) = progress_tx {
                let _ = tx.send((copied, total_size));
            }
            // Log every 10%
            if pct as f32 >= last_logged_pct + 10.0 {
                last_logged_pct = (pct as f32 / 10.0).floor() * 10.0;
                info!(
                    "[copy] {} — {:.0}% ({}/{})",
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
    percent: f32,
    detail: Option<String>,
) {
    publish_progress_bytes(
        event_bus,
        job_id,
        worker_id,
        stage,
        current_file,
        files_total,
        files_processed,
        percent,
        detail,
        None,
        None,
    )
    .await;
}

/// Publish a scan progress update with optional byte-level copy progress
#[cfg(feature = "redis-events")]
async fn publish_progress_bytes(
    event_bus: &crate::core::messaging::HybridEventBus,
    job_id: &str,
    worker_id: &str,
    stage: &str,
    current_file: Option<&str>,
    files_total: usize,
    files_processed: usize,
    percent: f32,
    detail: Option<String>,
    bytes_copied: Option<u64>,
    bytes_total: Option<u64>,
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
            bytes_copied,
            bytes_total,
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
            stream_conn: tokio::sync::Mutex::new(None),
            job_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
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
