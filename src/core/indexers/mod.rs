//! Indexer module
//! RSS feed and search integration with Usenet/BitTorrent indexers

pub mod clients;
pub mod definitions;
pub mod search;
pub mod rss;

pub use clients::{IndexerClient, SearchQuery, IndexerCapabilities, create_client_from_model};

use serde::{Deserialize, Serialize};

/// Indexer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Indexer {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub implementation_name: String,
    pub config_contract: String,
    pub info_link: String,
    pub message: Option<IndexerMessage>,
    pub tags: Vec<i64>,
    pub presets: Vec<IndexerPreset>,
    pub enable_rss: bool,
    pub enable_automatic_search: bool,
    pub enable_interactive_search: bool,
    pub supports_rss: bool,
    pub supports_search: bool,
    pub protocol: Protocol,
    pub priority: i32,
    pub download_client_id: i64,
    pub fields: Vec<IndexerField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerMessage {
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
pub struct IndexerPreset {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerField {
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

/// Release info from indexer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub guid: String,
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub comment_url: Option<String>,
    pub indexer_id: i64,
    pub indexer: String,
    pub publish_date: chrono::DateTime<chrono::Utc>,
    pub download_protocol: Protocol,
    pub tvdb_id: Option<i64>,
    pub tv_rage_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub mapped_season_number: Option<i32>,
    pub mapped_episode_numbers: Vec<i32>,
    pub mapped_absolute_episode_numbers: Vec<i32>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub quality: crate::core::profiles::qualities::QualityModel,
    pub languages: Vec<crate::core::profiles::languages::Language>,
    pub approved: bool,
    pub temporarily_rejected: bool,
    pub rejected: bool,
    pub rejections: Vec<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub protocol: Protocol,
    pub is_daily: bool,
    pub is_absolute_numbering: bool,
    pub is_possible_special_episode: bool,
    pub special: bool,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub download_client_id: Option<i64>,
    pub download_client: Option<String>,
    pub episode_requested: bool,
    pub download_url_generator: Option<String>,
    pub magnet_url: Option<String>,
    pub info_hash: Option<String>,
    pub seed_ratio: Option<f64>,
    pub source_title: Option<String>,
    pub indexer_flags: i32,
}

/// Search criteria for indexer queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCriteria {
    /// TVDB ID for the series (used by some indexers)
    pub series_id: i64,
    /// Series title for text-based search
    pub series_title: String,
    pub episode_id: Option<i64>,
    pub season_number: Option<i32>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub special: bool,
}
