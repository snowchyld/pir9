//! Download client module
//! Integration with download clients like qBittorrent, SABnzbd, NZBGet

pub mod clients;
pub mod history;
pub mod import;
pub mod queue;

pub use import::ImportService;

use serde::{Deserialize, Serialize};

/// Download client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadClient {
    pub id: i64,
    pub enable: bool,
    pub protocol: Protocol,
    pub priority: i32,
    pub name: String,
    pub fields: Vec<DownloadClientField>,
    pub implementation_name: String,
    pub implementation: String,
    pub config_contract: String,
    pub info_link: String,
    pub tags: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadClientField {
    pub order: i32,
    pub name: String,
    pub label: String,
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub field_type: String,
    pub advanced: bool,
    pub help_text: Option<String>,
}

/// Download protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    Unknown,
    Usenet,
    Torrent,
}

/// Download item status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadItemStatus {
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

/// Download item in queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub download_id: String,
    pub download_client_id: i64,
    pub download_client: String,
    pub title: String,
    pub status: DownloadItemStatus,
    pub tracked_download_status: TrackedDownloadStatus,
    pub tracked_download_state: TrackedDownloadState,
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
    pub protocol: Protocol,
    pub indexer: String,
    pub output_path: Option<String>,
    pub size: i64,
    pub sizeleft: i64,
    pub timeleft: Option<String>,
    pub estimated_completion_time: Option<chrono::DateTime<chrono::Utc>>,
    pub added: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackedDownloadStatus {
    Ok,
    Warning,
    Error,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub title: String,
    pub messages: Vec<String>,
}
