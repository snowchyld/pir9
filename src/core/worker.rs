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
use tracing::{debug, error, info, warn};

use crate::core::messaging::{Message, ScannedFile, ScanType};
use crate::core::scanner;

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

        info!(
            "Starting worker {} with paths: {:?}",
            self.worker_id, self.worker_paths
        );

        // Connect to Redis
        let event_bus = HybridEventBus::new_redis(&self.redis_url)
            .await
            .context("Failed to connect to Redis")?;

        let event_bus = Arc::new(event_bus);

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
                worker_id: self.worker_id.clone(),
                paths: self.paths_as_strings(),
            })
            .await;

        info!("Worker online, waiting for scan requests...");

        // Subscribe to events
        let mut receiver = event_bus.subscribe();

        // Create heartbeat interval
        let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

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
                    let (scans, files, uptime) = self.get_stats();
                    debug!("Sending heartbeat: scans={}, files={}, uptime={}s", scans, files, uptime);
                    event_bus.publish(Message::WorkerHeartbeat {
                        worker_id: self.worker_id.clone(),
                        paths: self.paths_as_strings(),
                        scans_completed: scans,
                        files_found: files,
                        uptime_seconds: uptime,
                    }).await;
                }
                result = receiver.recv() => {
                    match result {
                        Ok(message) => {
                            self.handle_message(message, &event_bus).await;
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
                worker_id: self.worker_id.clone(),
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
        &self,
        message: Message,
        event_bus: &crate::core::messaging::HybridEventBus,
    ) {
        match &message {
            Message::ScanRequest {
                job_id,
                scan_type,
                series_ids,
                paths,
            } => {
                info!(
                    "Received scan request: job_id={}, type={:?}, series={:?}",
                    job_id, scan_type, series_ids
                );

                // Pair series_ids with paths (1:1 aligned) and filter to paths we handle
                let relevant: Vec<(i64, &String)> = series_ids
                    .iter()
                    .zip(paths.iter())
                    .filter(|(_, p)| self.handles_path(p))
                    .map(|(&sid, p)| (sid, p))
                    .collect();

                if relevant.is_empty() {
                    debug!("Scan request not for our paths, ignoring");
                    return;
                }

                // Execute the scan
                let result = self.execute_scan(job_id, scan_type, &relevant).await;

                // Publish results
                for scan_result in result {
                    event_bus.publish(scan_result).await;
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
                debug!("Other worker heartbeat: {}", worker_id);
            }
            Message::WorkerOnline { worker_id, paths } => {
                info!("Worker came online: {} with paths {:?}", worker_id, paths);
            }
            Message::WorkerOffline { worker_id } => {
                info!("Worker went offline: {}", worker_id);
            }
            // File move requests from server (download import Phase 3)
            Message::ImportFilesRequest { job_id, files } => {
                info!(
                    "Received import files request: job_id={}, files={}",
                    job_id,
                    files.len()
                );

                // Check if any source paths are on our volumes
                let any_ours = files.iter().any(|f| self.handles_path(&f.source_path.to_string_lossy()));
                if !any_ours {
                    debug!("Import files request not for our paths, ignoring");
                    return;
                }

                let mut results = Vec::new();
                for spec in files {
                    let result = match move_file(&spec.source_path, &spec.dest_path) {
                        Ok(size) => {
                            info!(
                                "Moved file: {} -> {} ({} bytes)",
                                spec.source_path.display(),
                                spec.dest_path.display(),
                                size
                            );
                            crate::core::messaging::ImportFileResult {
                                source_path: spec.source_path.clone(),
                                dest_path: spec.dest_path.clone(),
                                success: true,
                                file_size: size,
                                error: None,
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to move file {} -> {}: {}",
                                spec.source_path.display(),
                                spec.dest_path.display(),
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
                info!(
                    "Received delete paths request: job_id={}, paths={}, recursive={}",
                    job_id,
                    paths.len(),
                    recursive
                );

                // Check if any paths are on our volumes
                let any_ours = paths.iter().any(|p| self.handles_path(p));
                if !any_ours {
                    debug!("Delete paths request not for our paths, ignoring");
                    return;
                }

                let mut results = Vec::new();
                for path_str in paths {
                    let path = std::path::Path::new(path_str.as_str());
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
                            info!("Deleted: {}", path_str);
                            results.push((path_str.clone(), true, None));
                        }
                        Err(e) => {
                            error!("Failed to delete {}: {}", path_str, e);
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
            // Scan results and import results are for the server, not workers
            Message::ScanResult { .. } | Message::ImportFilesResult { .. } | Message::DeletePathsResult { .. } => {
                // Ignore - these are for the server to process
            }
            // Log other message types at trace level
            other => {
                debug!("Ignoring message: {}", message_type_name(other));
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

    /// Execute a scan request and return results
    ///
    /// Each entry in `series_paths` is a `(series_id, path)` pair — the dispatcher
    /// keeps these aligned so the worker can tag each result with the correct series.
    /// For `RescanMovie`, series_id is actually movie_id (reused field).
    async fn execute_scan(
        &self,
        job_id: &str,
        scan_type: &ScanType,
        series_paths: &[(i64, &String)],
    ) -> Vec<Message> {
        let mut results = Vec::new();
        let mut total_files_found: u64 = 0;

        match scan_type {
            ScanType::RescanSeries => {
                for &(series_id, path_str) in series_paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        warn!("Scan path does not exist: {}", path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![format!("Path does not exist: {}", path_str)],
                        });
                        continue;
                    }

                    info!("Scanning path: {} (series_id={})", path_str, series_id);
                    let mut files = scanner::scan_series_directory(&path);

                    // Enrich each file with FFmpeg probe + BLAKE3 hash (LOCAL disk = fast)
                    for file in &mut files {
                        enrich_scanned_file(file).await;
                    }

                    total_files_found += files.len() as u64;
                    info!("Found and enriched {} video files in {}", files.len(), path_str);

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
                        warn!("Movie scan path does not exist: {}", path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: movie_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![format!("Path does not exist: {}", path_str)],
                        });
                        continue;
                    }

                    info!("Scanning movie path: {} (movie_id={})", path_str, movie_id);
                    if let Some(mut file) = scanner::scan_movie_directory(&path) {
                        enrich_scanned_file(&mut file).await;
                        total_files_found += 1;
                        info!(
                            "Found movie file: {} ({} bytes, hash={})",
                            file.filename,
                            file.size,
                            file.file_hash.as_deref().unwrap_or("none")
                        );
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: movie_id,
                            worker_id: self.worker_id.clone(),
                            files_found: vec![file],
                            errors: vec![],
                        });
                    } else {
                        info!("No video files found in movie path: {}", path_str);
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
            ScanType::DownloadedEpisodesScan => {
                // For download scanning, we scan the download directories
                for &(series_id, path_str) in series_paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        continue;
                    }

                    info!("Scanning download path: {}", path_str);
                    let mut files = scanner::scan_series_directory(&path);

                    for file in &mut files {
                        enrich_scanned_file(file).await;
                    }

                    total_files_found += files.len() as u64;

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

/// Move a file from source to dest, trying rename first (instant on same filesystem).
///
/// On the Synology NAS, `/volume1/downloads` and `/volume1/Shows` live on the same
/// Btrfs volume, so `rename()` is a metadata-only operation — atomic and instant.
/// Falls back to copy+delete for cross-filesystem moves.
fn move_file(source: &std::path::Path, dest: &std::path::Path) -> std::io::Result<i64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Try rename first (instant on same filesystem)
    if std::fs::rename(source, dest).is_ok() {
        let size = std::fs::metadata(dest)?.len() as i64;
        return Ok(size);
    }
    // Cross-filesystem fallback: copy + delete
    std::fs::copy(source, dest)?;
    let size = std::fs::metadata(dest)?.len() as i64;
    let _ = std::fs::remove_file(source);
    Ok(size)
}

/// Enrich a scanned file with FFmpeg media info and BLAKE3 content hash.
///
/// This runs on the worker with LOCAL disk access, making it fast (seconds
/// instead of minutes over NFS). The enriched data travels back to the server
/// via Redis so the server can skip redundant I/O.
async fn enrich_scanned_file(file: &mut ScannedFile) {
    use crate::core::mediafiles::compute_file_hash;

    let path = std::path::Path::new(&file.path);

    // FFmpeg probe (feature-gated)
    #[cfg(feature = "media-probe")]
    {
        use crate::core::mediafiles::{derive_quality_from_media, MediaAnalyzer};
        match MediaAnalyzer::analyze(path).await {
            Ok(info) => {
                let quality = derive_quality_from_media(&info, &file.filename);
                file.quality = serde_json::to_string(&quality).ok();
                file.media_info = serde_json::to_string(&info).ok();
            }
            Err(e) => {
                debug!("FFmpeg probe failed for {}: {}", file.filename, e);
            }
        }
    }

    // BLAKE3 content hash (pure Rust, no feature gate needed)
    match compute_file_hash(path).await {
        Ok(hash) => {
            file.file_hash = Some(hash);
        }
        Err(e) => {
            debug!("BLAKE3 hash failed for {}: {}", file.filename, e);
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
