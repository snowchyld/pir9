#![allow(dead_code, unused_imports)]
//! Messaging and event bus module
//! For inter-component communication and real-time updates

pub mod redis_bus;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::broadcast;

pub use redis_bus::HybridEventBus;

// ============================================================================
// Distributed Scanner Types
// ============================================================================

/// Type of scan operation for distributed scanning
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanType {
    /// Rescan series directories for episode files
    RescanSeries,
    /// Scan download directory for completed downloads
    DownloadedEpisodesScan,
    /// Rescan movie directories for video files
    RescanMovie,
    /// Placeholder for podcast library scanning (not yet implemented)
    RescanPodcast,
    /// Placeholder for music library scanning (not yet implemented)
    RescanMusic,
}

/// Known file metadata sent from server to worker for skip-enrichment optimization.
/// If a file's path and size match, the worker reuses the existing enrichment data
/// instead of running expensive FFmpeg probe + BLAKE3 hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFileInfo {
    pub size: i64,
    pub media_info: Option<String>,
    pub quality: Option<String>,
    pub file_hash: Option<String>,
}

// ============================================================================
// Worker File Operations Types
// ============================================================================

/// Specification for moving a single file from download dir to library
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileSpec {
    /// Source file path (in download dir)
    pub source_path: PathBuf,
    /// Destination file path (in library dir)
    pub dest_path: PathBuf,
}

/// Result of importing (moving) a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileResult {
    /// Source path (from the request)
    pub source_path: PathBuf,
    /// Destination path (from the request)
    pub dest_path: PathBuf,
    /// Whether the move succeeded
    pub success: bool,
    /// File size in bytes (after move)
    pub file_size: i64,
    /// Error message if move failed
    pub error: Option<String>,
}

/// A file discovered during scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedFile {
    /// Full path to the file
    pub path: PathBuf,
    /// File size in bytes
    pub size: i64,
    /// Parsed season number (if detected)
    pub season_number: Option<i32>,
    /// Parsed episode numbers (supports multi-episode files)
    pub episode_numbers: Vec<i32>,
    /// Extracted release group (e.g., "DIMENSION")
    pub release_group: Option<String>,
    /// Original filename
    pub filename: String,
    /// FFmpeg media info JSON (set by worker if available)
    #[serde(default)]
    pub media_info: Option<String>,
    /// Quality JSON derived from resolution (set by worker if available)
    #[serde(default)]
    pub quality: Option<String>,
    /// BLAKE3 content hash (set by worker if available)
    #[serde(default)]
    pub file_hash: Option<String>,
}

/// Event bus for publishing and subscribing to events
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Message>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.sender.subscribe()
    }

    /// Publish an event
    pub async fn publish(&self, message: Message) {
        let _ = self.sender.send(message);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Messages/events that can be published on the event bus
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    // Command events (for SignalR-style updates)
    CommandStarted {
        command_id: i64,
        name: String,
        message: Option<String>,
    },
    CommandUpdated {
        command_id: i64,
        name: String,
        message: Option<String>,
        body: serde_json::Value,
    },
    CommandCompleted {
        command_id: i64,
        name: String,
        message: Option<String>,
    },
    CommandFailed {
        command_id: i64,
        name: String,
        message: Option<String>,
        error: String,
    },

    // Series events
    SeriesAdded {
        series_id: i64,
        title: String,
    },
    SeriesUpdated {
        series_id: i64,
        title: String,
    },
    SeriesDeleted {
        series_id: i64,
        title: String,
    },
    SeriesRefreshed {
        series_id: i64,
        title: String,
    },
    SeriesScanned {
        series_id: i64,
        title: String,
        files_found: usize,
        episodes_matched: usize,
    },

    // Movie events
    MovieAdded {
        movie_id: i64,
        title: String,
    },
    MovieUpdated {
        movie_id: i64,
        title: String,
    },
    MovieDeleted {
        movie_id: i64,
        title: String,
    },
    MovieRefreshed {
        movie_id: i64,
        title: String,
    },
    MovieFileImported {
        movie_file_id: i64,
        movie_id: i64,
    },
    MovieFileDeleted {
        movie_file_id: i64,
        movie_id: i64,
    },

    // Episode events
    EpisodeAdded {
        episode_id: i64,
        series_id: i64,
        season_number: i32,
        episode_number: i32,
    },
    EpisodeUpdated {
        episode_id: i64,
        series_id: i64,
    },
    EpisodeFileImported {
        episode_file_id: i64,
        series_id: i64,
        episode_ids: Vec<i64>,
    },
    EpisodeFileDeleted {
        episode_file_id: i64,
        series_id: i64,
    },

    // Search events
    EpisodeSearchRequested {
        episode_id: i64,
        series_id: i64,
        season_number: i32,
        episode_number: i32,
    },
    SeasonSearchRequested {
        series_id: i64,
        season_number: i32,
    },
    SeriesSearchRequested {
        series_id: i64,
    },

    // Download events
    ReleaseGrabbed {
        download_id: String,
        series_id: i64,
        episode_ids: Vec<i64>,
        release_title: String,
        indexer: String,
        size: i64,
    },
    DownloadStarted {
        download_id: String,
        series_id: i64,
    },
    DownloadCompleted {
        download_id: String,
        series_id: i64,
        episode_file_id: i64,
    },
    DownloadFailed {
        download_id: String,
        series_id: i64,
        message: String,
    },

    // Queue events
    QueueUpdated,

    // System events
    HealthCheckChanged,
    ConfigUpdated,

    // Notification events
    NotificationSent {
        notification_type: String,
        success: bool,
    },

    // Distributed scanning events (for worker mode)
    /// Request sent from server to workers to scan files
    ScanRequest {
        /// Unique job ID for tracking
        job_id: String,
        /// Type of scan to perform
        scan_type: ScanType,
        /// Series IDs to scan (empty = all configured series)
        series_ids: Vec<i64>,
        /// Paths to scan (used by worker to know which paths it handles)
        paths: Vec<String>,
        /// Known file data from DB (path → metadata) for skip-enrichment optimization.
        /// Worker compares on-disk size against known size; if matched and enrichment
        /// data exists, skips expensive FFmpeg probe + BLAKE3 hash.
        #[serde(default)]
        known_files: HashMap<String, KnownFileInfo>,
    },
    /// Progress update from worker during file enrichment (probe + hash)
    ScanProgress {
        /// Unique job ID matching the scan request
        job_id: String,
        /// Worker instance ID
        worker_id: String,
        /// Current stage: "scanning", "probing", "hashing"
        stage: String,
        /// File currently being processed
        current_file: Option<String>,
        /// Total number of files to process
        files_total: usize,
        /// Number of files fully processed so far
        files_processed: usize,
        /// Overall percent complete (0-100)
        percent: u8,
        /// Detail string after probe: e.g. "1080p x265 HDR10", or "unchanged" for skipped files
        detail: Option<String>,
    },
    /// Result sent from worker back to server
    ScanResult {
        /// Job ID matching the request
        job_id: String,
        /// Series ID that was scanned
        series_id: i64,
        /// Worker instance ID that performed the scan
        worker_id: String,
        /// Files discovered during scan
        files_found: Vec<ScannedFile>,
        /// Any errors encountered
        errors: Vec<String>,
    },
    /// Worker announcing it's online and what paths it handles
    WorkerOnline {
        /// Worker instance ID
        worker_id: String,
        /// Paths this worker can scan (local to the worker)
        paths: Vec<String>,
    },
    /// Worker going offline
    WorkerOffline {
        /// Worker instance ID
        worker_id: String,
    },
    /// Worker heartbeat (sent periodically to indicate worker is alive)
    WorkerHeartbeat {
        /// Worker instance ID
        worker_id: String,
        /// Paths this worker handles
        paths: Vec<String>,
        /// Number of scans completed since startup
        scans_completed: u64,
        /// Number of files found in total
        files_found: u64,
        /// Worker uptime in seconds
        uptime_seconds: u64,
    },

    // Worker file operations (for download import & cleanup)
    /// Server asks worker to move files from download dir → library
    ImportFilesRequest {
        /// Unique job ID for tracking
        job_id: String,
        /// Files to move (source → dest pairs)
        files: Vec<ImportFileSpec>,
    },
    /// Worker confirms file moves completed
    ImportFilesResult {
        /// Job ID matching the request
        job_id: String,
        /// Worker instance ID that performed the moves
        worker_id: String,
        /// Per-file results
        results: Vec<ImportFileResult>,
    },
    /// Server asks worker to delete paths from disk
    DeletePathsRequest {
        /// Unique job ID for tracking
        job_id: String,
        /// Paths to delete
        paths: Vec<String>,
        /// Whether to recursively delete directories
        recursive: bool,
    },
    /// Worker confirms deletions completed
    DeletePathsResult {
        /// Job ID matching the request
        job_id: String,
        /// Worker instance ID
        worker_id: String,
        /// Per-path results: (path, success, error message)
        results: Vec<(String, bool, Option<String>)>,
    },
}
