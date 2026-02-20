//! Download queue management

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Queue item for downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadQueueItem {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub release: crate::core::indexers::ReleaseInfo,
    pub status: QueueStatus,
    pub added: DateTime<Utc>,
    pub started: Option<DateTime<Utc>>,
    pub completed: Option<DateTime<Utc>>,
    pub download_id: Option<String>,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
}

/// Queue status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueueStatus {
    Pending,
    Downloading,
    ImportPending,
    Importing,
    Completed,
    Failed,
    Cancelled,
}

/// Queue service
pub struct DownloadQueueService;

impl DownloadQueueService {
    /// Add item to queue
    pub async fn enqueue(&self, series_id: i64, episode_ids: Vec<i64>, release: crate::core::indexers::ReleaseInfo) -> anyhow::Result<i64> {
        Ok(1)
    }
    
    /// Process queue
    pub async fn process(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Get queue items
    pub async fn get_items(&self) -> anyhow::Result<Vec<DownloadQueueItem>> {
        Ok(vec![])
    }
    
    /// Remove item from queue
    pub async fn remove(&self, id: i64, blacklist: bool) -> anyhow::Result<()> {
        Ok(())
    }
}
