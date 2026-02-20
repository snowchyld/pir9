#![allow(dead_code, unused_imports)]
//! Notifications module
//! Integration with notification providers

pub mod providers;
pub mod service;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use providers::*;
pub use service::NotificationService;

/// Notification payload sent to providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    pub event_type: NotificationEventType,
    pub title: String,
    pub message: String,
    pub series_title: Option<String>,
    pub series_id: Option<i64>,
    pub episode_info: Option<EpisodeInfo>,
    pub release_info: Option<ReleaseInfo>,
    pub health_info: Option<HealthInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NotificationEventType {
    Grab,
    Download,
    Upgrade,
    Rename,
    SeriesAdd,
    SeriesDelete,
    EpisodeFileDelete,
    EpisodeFileDeleteForUpgrade,
    HealthIssue,
    HealthRestored,
    ManualInteractionRequired,
    ApplicationUpdate,
    Test,
}

impl NotificationEventType {
    /// Get the event type string used for database filtering
    pub fn as_event_key(&self) -> &'static str {
        match self {
            NotificationEventType::Grab => "grab",
            NotificationEventType::Download => "download",
            NotificationEventType::Upgrade => "upgrade",
            NotificationEventType::Rename => "rename",
            NotificationEventType::SeriesAdd => "series_add",
            NotificationEventType::SeriesDelete => "series_delete",
            NotificationEventType::EpisodeFileDelete => "episode_file_delete",
            NotificationEventType::EpisodeFileDeleteForUpgrade => "episode_file_delete_for_upgrade",
            NotificationEventType::HealthIssue => "health_issue",
            NotificationEventType::HealthRestored => "health_restored",
            NotificationEventType::ManualInteractionRequired => "manual_interaction_required",
            NotificationEventType::ApplicationUpdate => "application_update",
            NotificationEventType::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInfo {
    pub season_number: i32,
    pub episode_number: i32,
    pub title: Option<String>,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub release_title: String,
    pub indexer: String,
    pub size: i64,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub source: String,
    pub check_type: String,
    pub message: String,
    pub wiki_url: Option<String>,
}

/// Notification provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub implementation_name: String,
    pub config_contract: String,
    pub info_link: String,
    pub message: Option<NotificationMessage>,
    pub tags: Vec<i64>,
    pub presets: Vec<NotificationPreset>,
    pub on_grab: bool,
    pub on_download: bool,
    pub on_upgrade: bool,
    pub on_rename: bool,
    pub on_series_add: bool,
    pub on_series_delete: bool,
    pub on_episode_file_delete: bool,
    pub on_episode_file_delete_for_upgrade: bool,
    pub on_health_issue: bool,
    pub on_health_restored: bool,
    pub on_manual_interaction_required: bool,
    pub on_application_update: bool,
    pub supports_on_grab: bool,
    pub supports_on_download: bool,
    pub supports_on_upgrade: bool,
    pub supports_on_rename: bool,
    pub supports_on_series_add: bool,
    pub supports_on_series_delete: bool,
    pub supports_on_episode_file_delete: bool,
    pub supports_on_episode_file_delete_for_upgrade: bool,
    pub supports_on_health_issue: bool,
    pub supports_on_health_restored: bool,
    pub supports_on_manual_interaction_required: bool,
    pub supports_on_application_update: bool,
    pub include_health_warnings: bool,
    pub test_command: String,
    pub fields: Vec<NotificationField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub message_type: MessageType,
    pub message: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageType {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreset {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationField {
    pub order: i32,
    pub name: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub field_type: String,
    pub advanced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_link: Option<String>,
    #[serde(default)]
    pub privacy: String,
    #[serde(default)]
    pub is_float: bool,
}

/// Trait for notification providers
#[async_trait]
pub trait NotificationProvider: Send + Sync {
    /// Provider implementation name (e.g., "Discord", "Webhook")
    fn name(&self) -> &str;

    /// Provider implementation identifier
    fn implementation(&self) -> &str;

    /// Config contract name for the API
    fn config_contract(&self) -> &str;

    /// Test the notification connection
    async fn test(&self) -> Result<()>;

    /// Send a notification
    async fn send(&self, payload: &NotificationPayload) -> Result<()>;

    /// Get the configurable fields for this provider
    fn get_fields(&self) -> Vec<NotificationField>;

    /// Get info link for documentation
    fn info_link(&self) -> &str {
        ""
    }
}
