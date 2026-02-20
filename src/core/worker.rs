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

use crate::core::messaging::{Message, ScanType};
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
    pub async fn new(
        redis_url: &str,
        worker_id: &str,
        worker_paths: Vec<String>,
    ) -> Result<Self> {
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
        self.worker_paths.iter()
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

        info!("Starting worker {} with paths: {:?}", self.worker_id, self.worker_paths);

        // Connect to Redis
        let event_bus = HybridEventBus::new_redis(&self.redis_url).await
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
        event_bus.publish(Message::WorkerOnline {
            worker_id: self.worker_id.clone(),
            paths: self.paths_as_strings(),
        }).await;

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
        event_bus.publish(Message::WorkerOffline {
            worker_id: self.worker_id.clone(),
        }).await;

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
    async fn handle_message(&self, message: Message, event_bus: &crate::core::messaging::HybridEventBus) {
        match &message {
            Message::ScanRequest { job_id, scan_type, series_ids, paths } => {
                info!(
                    "Received scan request: job_id={}, type={:?}, series={:?}",
                    job_id, scan_type, series_ids
                );

                // Check if this request is for paths we handle
                let relevant_paths: Vec<&String> = paths.iter()
                    .filter(|p| self.handles_path(p))
                    .collect();

                if relevant_paths.is_empty() {
                    debug!("Scan request not for our paths, ignoring");
                    return;
                }

                // Execute the scan
                let result = self.execute_scan(job_id, scan_type, series_ids, &relevant_paths).await;

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
            // Scan results are for the server, not workers
            Message::ScanResult { .. } => {
                // Ignore - this is for the server to process
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
    async fn execute_scan(
        &self,
        job_id: &str,
        scan_type: &ScanType,
        series_ids: &[i64],
        paths: &[&String],
    ) -> Vec<Message> {
        let mut results = Vec::new();
        let mut total_files_found: u64 = 0;

        match scan_type {
            ScanType::RescanSeries => {
                for path_str in paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        warn!("Scan path does not exist: {}", path_str);
                        results.push(Message::ScanResult {
                            job_id: job_id.to_string(),
                            series_id: 0, // Unknown series
                            worker_id: self.worker_id.clone(),
                            files_found: vec![],
                            errors: vec![format!("Path does not exist: {}", path_str)],
                        });
                        continue;
                    }

                    info!("Scanning path: {}", path_str);
                    let files = scanner::scan_series_directory(&path);
                    total_files_found += files.len() as u64;

                    info!("Found {} video files in {}", files.len(), path_str);

                    // For now, use series_id 0 since we don't have series mapping
                    // The server will need to match paths to series IDs
                    let series_id = series_ids.first().copied().unwrap_or(0);

                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id,
                        worker_id: self.worker_id.clone(),
                        files_found: files,
                        errors: vec![],
                    });
                }
            }
            ScanType::DownloadedEpisodesScan => {
                // For download scanning, we scan the download directories
                for path_str in paths {
                    let path = PathBuf::from(path_str);

                    if !path.exists() {
                        continue;
                    }

                    info!("Scanning download path: {}", path_str);
                    let files = scanner::scan_series_directory(&path);
                    total_files_found += files.len() as u64;

                    results.push(Message::ScanResult {
                        job_id: job_id.to_string(),
                        series_id: 0,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_path() {
        let worker = WorkerRunner {
            worker_id: "test".to_string(),
            worker_paths: vec![
                PathBuf::from("/media/tv"),
                PathBuf::from("/media/anime"),
            ],
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
