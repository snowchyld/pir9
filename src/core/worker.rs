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
                let total = files.len();
                info!("[import] Starting import job {} — {} file(s)", job_id, total);

                // Check if any source paths are on our volumes
                let any_ours = files.iter().any(|f| self.handles_path(&f.source_path.to_string_lossy()));
                if !any_ours {
                    debug!("Import files request not for our paths, ignoring");
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

                    // Enrich each file with FFmpeg probe + BLAKE3 hash (LOCAL disk = fast)
                    for (idx, file) in files.iter_mut().enumerate() {
                        enrich_scanned_file(file, (idx + 1, file_count)).await;
                    }

                    let elapsed = scan_start.elapsed();
                    total_files_found += file_count as u64;
                    info!(
                        "[scan][{}] Complete — {} file(s) enriched in {:.1}s (id={})",
                        media_type, file_count, elapsed.as_secs_f64(), series_id
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
                        enrich_scanned_file(&mut file, (1, 1)).await;
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
                        continue;
                    }

                    let scan_start = std::time::Instant::now();
                    info!("[scan][download] Scanning {}", path_str);
                    let mut files = scanner::scan_series_directory(&path);
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

/// Result of a file move, including whether it was a rename or copy
struct MoveResult {
    size: i64,
    method: &'static str, // "rename" or "copy"
}

/// Move a file from source to dest, trying rename first (instant on same filesystem).
///
/// On the Synology NAS, `/volume1/downloads` and `/volume1/Shows` live on the same
/// Btrfs volume, so `rename()` is a metadata-only operation — atomic and instant.
/// Falls back to copy+delete for cross-filesystem moves.
fn move_file(source: &std::path::Path, dest: &std::path::Path) -> std::io::Result<MoveResult> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Try rename first (instant on same filesystem)
    if std::fs::rename(source, dest).is_ok() {
        let size = std::fs::metadata(dest)?.len() as i64;
        return Ok(MoveResult { size, method: "rename" });
    }
    // Cross-filesystem fallback: copy with progress + delete
    let source_size = std::fs::metadata(source)?.len();
    copy_with_progress(source, dest, source_size)?;
    let size = std::fs::metadata(dest)?.len() as i64;
    let _ = std::fs::remove_file(source);
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

/// Enrich a scanned file with FFmpeg media info and BLAKE3 content hash.
///
/// This runs on the worker with LOCAL disk access, making it fast (seconds
/// instead of minutes over NFS). The enriched data travels back to the server
/// via Redis so the server can skip redundant I/O.
///
/// `progress` is a `(current, total)` tuple for logging progress like "(3/10)".
async fn enrich_scanned_file(file: &mut ScannedFile, progress: (usize, usize)) {
    use crate::core::mediafiles::compute_file_hash;

    let path = std::path::Path::new(&file.path);
    let (current, total) = progress;

    // FFmpeg probe (feature-gated)
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

    // BLAKE3 content hash (pure Rust, no feature gate needed)
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
