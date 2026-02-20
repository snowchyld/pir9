//! Download history tracking

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Download history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistory {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub download_id: String,
    pub source_title: String,
    pub quality: crate::core::profiles::qualities::QualityModel,
    pub languages: Vec<crate::core::profiles::languages::Language>,
    pub protocol: super::Protocol,
    pub indexer: String,
    pub download_client: String,
    pub event_type: HistoryEventType,
    pub date: DateTime<Utc>,
    pub data: serde_json::Value,
}

/// History event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryEventType {
    Unknown,
    Grabbed,
    DownloadFailed,
    DownloadFolderImported,
    DownloadIgnored,
    FileImported,
    FileDeleted,
    FileRenamed,
}

/// History service
pub struct HistoryService;

impl HistoryService {
    /// Record a grab event
    pub async fn record_grab(&self, series_id: i64, episode_ids: Vec<i64>, release: &super::super::indexers::ReleaseInfo) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Record a download failure
    pub async fn record_download_failed(&self, series_id: i64, episode_ids: Vec<i64>, download_id: &str, message: &str) -> anyhow::Result<()> {
        Ok(())
    }
    
    /// Record a successful import
    pub async fn record_import(&self, series_id: i64, episode_ids: Vec<i64>, episode_file_id: i64, is_upgrade: bool) -> anyhow::Result<()> {
        Ok(())
    }
}
