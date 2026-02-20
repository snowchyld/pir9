//! Download queue module
//! Manages pending and active downloads

pub mod service;

pub use service::TrackedDownloadService;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Queue item status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueStatus {
    Unknown,
    Queued,
    Paused,
    Downloading,
    Completed,
    Failed,
    Warning,
    Delay,
    DownloadClientUnavailable,
}

/// Tracked download status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackedDownloadStatus {
    Ok,
    Warning,
    Error,
}

/// Tracked download state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackedDownloadState {
    Downloading,
    ImportBlocked,
    ImportPending,
    Importing,
    Imported,
    FailedPending,
    Failed,
    Ignored,
}

/// Queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i64,
    pub series_id: i64,
    pub episode_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub title: String,
    pub status: QueueStatus,
    pub tracked_download_status: TrackedDownloadStatus,
    pub tracked_download_state: TrackedDownloadState,
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
    pub download_id: Option<String>,
    pub protocol: Protocol,
    pub download_client: String,
    pub indexer: String,
    pub output_path: Option<String>,
    pub episode_has_file: bool,
    pub size: i64,
    pub sizeleft: i64,
    pub timeleft: Option<String>,
    pub estimated_completion_time: Option<DateTime<Utc>>,
    pub added: DateTime<Utc>,
    pub quality: crate::core::profiles::qualities::QualityModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub title: String,
    pub messages: Vec<String>,
}

/// Download protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    Unknown,
    Usenet,
    Torrent,
}

/// Queue service
pub struct QueueService;

impl QueueService {
    /// Get all queue items
    pub async fn get_all(&self) -> Vec<QueueItem> {
        vec![]
    }
    
    /// Get queue item by ID
    pub async fn get_by_id(&self, id: i64) -> Option<QueueItem> {
        None
    }
    
    /// Add item to queue
    pub async fn add(&self, item: QueueItem) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Remove item from queue
    pub async fn remove(&self, id: i64, blacklist: bool) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Grab a release
    pub async fn grab(&self, release_id: i64) -> anyhow::Result<()> {
        Ok(())
    }
}
