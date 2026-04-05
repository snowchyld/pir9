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
// Redis Stream Constants
// ============================================================================

/// Redis stream key for durable job dispatch (server → workers)
#[cfg(feature = "redis-events")]
pub const REDIS_JOB_STREAM: &str = "pir9:stream:jobs";

/// Redis stream key for durable result delivery (workers → server)
#[cfg(feature = "redis-events")]
pub const REDIS_RESULT_STREAM: &str = "pir9:stream:results";

/// Consumer group name for workers reading from the job stream
#[cfg(feature = "redis-events")]
pub const REDIS_WORKER_GROUP: &str = "pir9-workers";

/// Consumer group name for the server reading from the result stream
#[cfg(feature = "redis-events")]
pub const REDIS_SERVER_GROUP: &str = "pir9-server";

/// Maximum stream length (approximate trimming to bound memory)
#[cfg(feature = "redis-events")]
pub const STREAM_MAXLEN: usize = 10000;

// ============================================================================
// Message Transport Classification
// ============================================================================

/// Determines which Redis transport a message should use.
///
/// - `Job`: Durable delivery via job stream (server → workers, exclusive via consumer group)
/// - `Result`: Durable delivery via result stream (workers → server)
/// - `Ephemeral`: Fire-and-forget pub/sub broadcast (progress, heartbeats, UI events)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageCategory {
    /// Server → worker job dispatch (XADD to job stream)
    Job,
    /// Worker → server result delivery (XADD to result stream)
    Result,
    /// Broadcast to all subscribers (PUBLISH to pub/sub channel)
    Ephemeral,
}

impl Message {
    /// Classify this message for transport routing.
    pub fn category(&self) -> MessageCategory {
        match self {
            // Jobs: server dispatches to workers
            Message::ScanRequest { .. }
            | Message::ProbeFileRequest { .. }
            | Message::HashFileRequest { .. }
            | Message::ImportFilesRequest { .. }
            | Message::DeletePathsRequest { .. }
            | Message::RenameFilesRequest { .. } => MessageCategory::Job,

            // Results: workers send back to server
            Message::ScanResult { .. }
            | Message::ProbeFileResult { .. }
            | Message::HashFileResult { .. }
            | Message::ImportFilesResult { .. }
            | Message::DeletePathsResult { .. }
            | Message::RenameFilesResult { .. } => MessageCategory::Result,

            // Everything else: ephemeral broadcast
            _ => MessageCategory::Ephemeral,
        }
    }
}

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
    /// Import movie from download directory (probe + hash, then server dispatches move)
    DownloadedMovieScan,
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

    // Podcast events
    PodcastAdded {
        podcast_id: i64,
        title: String,
    },
    PodcastUpdated {
        podcast_id: i64,
        title: String,
    },
    PodcastDeleted {
        podcast_id: i64,
        title: String,
    },
    PodcastRefreshed {
        podcast_id: i64,
        title: String,
    },

    // Artist (music) events
    ArtistAdded {
        artist_id: i64,
        name: String,
    },
    ArtistUpdated {
        artist_id: i64,
        name: String,
    },
    ArtistDeleted {
        artist_id: i64,
        name: String,
    },
    ArtistRefreshed {
        artist_id: i64,
        name: String,
    },
    TrackFileImported {
        track_file_id: i64,
        artist_id: i64,
        album_id: i64,
    },
    TrackFileDeleted {
        track_file_id: i64,
        artist_id: i64,
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
        /// Current stage: "scanning", "probing", "hashing", "copying"
        stage: String,
        /// File currently being processed
        current_file: Option<String>,
        /// Total number of files to process
        files_total: usize,
        /// Number of files fully processed so far
        files_processed: usize,
        /// Overall percent complete (0.0-100.0, supports 0.1% granularity)
        percent: f32,
        /// Detail string after probe: e.g. "1080p x265 HDR10", or "unchanged" for skipped files
        detail: Option<String>,
        /// Entity IDs associated with this scan (filled by consumer, not by worker)
        #[serde(default)]
        entity_ids: Vec<i64>,
        /// Scan type (filled by consumer, not by worker)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scan_type: Option<ScanType>,
        /// Bytes copied so far (only set during "copying" stage)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes_copied: Option<u64>,
        /// Total bytes to copy (only set during "copying" stage)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes_total: Option<u64>,
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
    // Granular per-file enrichment jobs (probe and hash independently)
    /// Server asks a worker to FFmpeg-probe a single file
    ProbeFileRequest {
        /// Unique job ID for this probe operation
        job_id: String,
        /// Parent scan job ID (for tracking which scan this belongs to)
        parent_job_id: String,
        /// Full path to the file to probe
        file_path: String,
        /// Entity ID (series_id or movie_id) this file belongs to
        entity_id: i64,
        /// Scan type context (for consumer routing)
        scan_type: ScanType,
    },
    /// Worker returns FFmpeg probe results for a single file
    ProbeFileResult {
        /// Job ID matching the request
        job_id: String,
        /// Parent scan job ID
        parent_job_id: String,
        /// File that was probed
        file_path: String,
        /// Entity ID
        entity_id: i64,
        /// Worker that performed the probe
        worker_id: String,
        /// FFmpeg media info JSON
        media_info: Option<String>,
        /// Quality JSON derived from resolution + filename analysis
        quality: Option<String>,
    },
    /// Server asks a worker to BLAKE3-hash a single file
    HashFileRequest {
        /// Unique job ID for this hash operation
        job_id: String,
        /// Parent scan job ID
        parent_job_id: String,
        /// Full path to the file to hash
        file_path: String,
        /// Entity ID (series_id or movie_id)
        entity_id: i64,
    },
    /// Worker returns BLAKE3 hash for a single file
    HashFileResult {
        /// Job ID matching the request
        job_id: String,
        /// Parent scan job ID
        parent_job_id: String,
        /// File that was hashed
        file_path: String,
        /// Entity ID
        entity_id: i64,
        /// Worker that performed the hash
        worker_id: String,
        /// BLAKE3 content hash (hex string)
        file_hash: Option<String>,
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

    /// Server asks worker to rename (move) files on local disk
    RenameFilesRequest {
        /// Unique job ID for tracking
        job_id: String,
        /// Files to rename (source → dest pairs)
        files: Vec<ImportFileSpec>,
        /// Episode file IDs matching each file (for DB update after rename)
        episode_file_ids: Vec<i64>,
    },
    /// Worker confirms renames completed
    RenameFilesResult {
        /// Job ID matching the request
        job_id: String,
        /// Worker instance ID
        worker_id: String,
        /// Per-file results
        results: Vec<ImportFileResult>,
    },
}
