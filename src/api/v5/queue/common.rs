//! Shared types and helpers for queue endpoints.

use serde::{Deserialize, Serialize};

use crate::core::queue::{
    Protocol as QueueProtocol, QueueStatus, TrackedDownloadState, TrackedDownloadStatus,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueueListQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_unknown_series_items: Option<bool>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct QueueDetailsQuery {
    pub series_id: Option<i32>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct RemoveFromQueueQuery {
    #[serde(default)]
    pub remove_from_client: bool,
    #[serde(default)]
    pub blocklist: bool,
    #[serde(default)]
    pub skip_redownload: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueResource {
    pub id: i64,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub size: f64,
    pub title: String,
    pub sizeleft: f64,
    pub timeleft: Option<String>,
    pub estimated_completion_time: Option<String>,
    pub added: Option<String>,
    pub status: String,
    pub tracked_download_status: Option<String>,
    pub tracked_download_state: Option<String>,
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
    pub download_id: Option<String>,
    pub protocol: String,
    pub download_client: Option<String>,
    pub download_client_has_post_import_category: bool,
    pub indexer: Option<String>,
    pub output_path: Option<String>,
    pub episode_has_file: bool,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audiobook_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leechers: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leech_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<QueueEpisodeResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<QueueSeriesResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movie: Option<QueueMovieResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<QueueArtistResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audiobook: Option<QueueAudiobookResource>,
    /// Live import progress when trackedDownloadState is "importing"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_progress: Option<ImportProgressResource>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressResource {
    /// Current stage: "scanning", "probing", "hashing", "copying"
    pub stage: String,
    /// File currently being processed
    pub current_file: Option<String>,
    /// Total number of files to import
    pub files_total: usize,
    /// Number of files processed so far
    pub files_processed: usize,
    /// Overall percent complete (0.0-100.0)
    pub percent: f32,
    /// Detail string (e.g. "1080p x265 HDR10")
    pub detail: Option<String>,
    /// Bytes copied so far (only during "copying" stage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_copied: Option<u64>,
    /// Total bytes to copy (only during "copying" stage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueEpisodeResource {
    pub id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: String,
    pub air_date_utc: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueSeriesResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueMovieResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueArtistResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueueAudiobookResource {
    pub id: i64,
    pub title: String,
    pub title_slug: String,
}

/// Per-content-type category mappings parsed from download client settings.
pub(super) struct ClientCategories {
    pub movie: Vec<String>,
    pub anime: Vec<String>,
    pub music: Vec<String>,
    pub audiobook: Vec<String>,
    pub podcast: Vec<String>,
    /// Union of all categories — used for download filtering.
    pub all: Vec<String>,
}

impl ClientCategories {
    /// Parse category fields from download client settings JSON.
    /// Supports both the new split format (category/movieCategory/animeCategory/musicCategory/etc.)
    /// and the legacy comma-separated format in the `category` field.
    pub fn from_settings(settings: &serde_json::Value) -> Self {
        let get_cats = |key: &str| -> Vec<String> {
            settings
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.split(',')
                        .map(|c| c.trim().to_lowercase())
                        .filter(|c| !c.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };

        let has_new_format = settings.get("movieCategory").is_some();

        if has_new_format {
            let series = get_cats("category");
            let movie = get_cats("movieCategory");
            let anime = get_cats("animeCategory");
            let music = get_cats("musicCategory");
            let audiobook = get_cats("audiobookCategory");
            let podcast = get_cats("podcastCategory");

            let mut all = Vec::new();
            for cats in [&series, &movie, &anime, &music, &audiobook, &podcast] {
                all.extend(cats.iter().cloned());
            }
            all.sort();
            all.dedup();

            Self {
                movie,
                anime,
                music,
                audiobook,
                podcast,
                all,
            }
        } else {
            // Legacy format: infer content type from well-known category names.
            let all_cats = get_cats("category");
            let mut movie = Vec::new();
            let mut anime = Vec::new();
            let mut music = Vec::new();
            let mut audiobook = Vec::new();
            let mut podcast = Vec::new();

            for cat in &all_cats {
                match cat.as_str() {
                    "radarr" | "movies" | "movie" => movie.push(cat.clone()),
                    "anime" | "sonarr-anime" | "anime-sonarr" => anime.push(cat.clone()),
                    "music" | "lidarr" => music.push(cat.clone()),
                    "audiobook" | "audiobooks" | "readarr" => audiobook.push(cat.clone()),
                    "podcast" | "podcasts" => podcast.push(cat.clone()),
                    _ => {} // series (default)
                }
            }

            Self {
                movie,
                anime,
                music,
                audiobook,
                podcast,
                all: all_cats,
            }
        }
    }

    /// Determine content type for a download based on its category.
    pub fn content_type_for(&self, category: &str) -> &'static str {
        let cat = category.to_lowercase();
        if self.movie.iter().any(|c| c == &cat) {
            return "movie";
        }
        if self.anime.iter().any(|c| c == &cat) {
            return "anime";
        }
        if self.music.iter().any(|c| c == &cat) {
            return "music";
        }
        if self.audiobook.iter().any(|c| c == &cat) {
            return "audiobook";
        }
        if self.podcast.iter().any(|c| c == &cat) {
            return "podcast";
        }
        "series"
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessage {
    pub title: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
    pub records: Vec<QueueResource>,
    /// Number of previously imported downloads hidden from the queue.
    /// These have tracked_download records with status=4 (Imported) that
    /// suppress the torrent from reappearing. Clear them to reimport.
    pub hidden_imported_count: i64,
    /// Completed/imported tracked downloads shown on the Completed tab.
    pub completed_records: Vec<QueueResource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatusResource {
    pub total_count: i32,
    pub count: i32,
    pub unknown_count: i32,
    pub errors: bool,
    pub warnings: bool,
    pub unknown_errors: bool,
    pub unknown_warnings: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueActionResponse {
    pub success: bool,
}

/// Convert a `QueueItem` from the service layer into a `QueueResource` for the API.
pub(super) fn queue_item_to_resource(item: &crate::core::queue::QueueItem) -> QueueResource {
    let protocol = match item.protocol {
        QueueProtocol::Usenet => "usenet",
        QueueProtocol::Torrent => "torrent",
        QueueProtocol::Unknown => "unknown",
    };

    let status = match item.status {
        QueueStatus::Queued => "queued",
        QueueStatus::Paused => "paused",
        QueueStatus::Downloading => "downloading",
        QueueStatus::Completed => "completed",
        QueueStatus::Failed => "failed",
        QueueStatus::Warning => "warning",
        QueueStatus::Delay => "delay",
        QueueStatus::DownloadClientUnavailable => "downloadClientUnavailable",
        QueueStatus::Unknown => "unknown",
    };

    // Override status to "stalled" when the download is stalled (Warning from a Stalled state)
    // We detect this by checking if it's a warning with active seed/leech data showing 0 seeds
    let status = if status == "warning" && item.seeds == Some(0) && item.leechers == Some(0) {
        "stalled"
    } else {
        status
    };

    let tracked_state = match item.tracked_download_state {
        TrackedDownloadState::Downloading => "downloading",
        TrackedDownloadState::ImportBlocked => "importBlocked",
        TrackedDownloadState::ImportPending => "importPending",
        TrackedDownloadState::Importing => "importing",
        TrackedDownloadState::Imported => "imported",
        TrackedDownloadState::FailedPending => "failedPending",
        TrackedDownloadState::Failed => "failed",
        TrackedDownloadState::Ignored => "ignored",
    };

    let tracked_status = match item.tracked_download_status {
        TrackedDownloadStatus::Ok => "ok",
        TrackedDownloadStatus::Warning => "warning",
        TrackedDownloadStatus::Error => "error",
    };

    let quality_model = QualityModel {
        quality: QualityResource {
            id: item.quality.quality.weight(),
            name: format!("{:?}", item.quality.quality),
            source: "unknown".to_string(),
            resolution: item.quality.quality.resolution_width(),
        },
        revision: RevisionResource {
            version: item.quality.revision.version,
            real: item.quality.revision.real,
            is_repack: item.quality.revision.is_repack,
        },
    };

    let status_messages: Vec<StatusMessage> = item
        .status_messages
        .iter()
        .map(|sm| StatusMessage {
            title: sm.title.clone(),
            messages: sm.messages.clone(),
        })
        .collect();

    // Use stored content type from tracked download
    let content_type = &item.content_type;

    QueueResource {
        id: item.id,
        series_id: if item.series_id > 0 {
            Some(item.series_id)
        } else {
            None
        },
        episode_id: if item.episode_id > 0 {
            Some(item.episode_id)
        } else {
            None
        },
        languages: vec![LanguageResource {
            id: 1,
            name: "English".to_string(),
        }],
        quality: quality_model,
        custom_formats: vec![],
        custom_format_score: 0,
        size: item.size as f64,
        title: item.title.clone(),
        sizeleft: item.sizeleft as f64,
        timeleft: item.timeleft.clone(),
        estimated_completion_time: item.estimated_completion_time.map(|t| t.to_rfc3339()),
        added: Some(item.added.to_rfc3339()),
        status: status.to_string(),
        tracked_download_status: Some(tracked_status.to_string()),
        tracked_download_state: Some(tracked_state.to_string()),
        status_messages,
        error_message: item.error_message.clone(),
        download_id: item.download_id.clone(),
        protocol: protocol.to_string(),
        download_client: Some(item.download_client.clone()),
        download_client_has_post_import_category: false,
        indexer: Some(item.indexer.clone()),
        output_path: item.output_path.clone(),
        episode_has_file: item.episode_has_file,
        content_type: content_type.to_string(),
        movie_id: if item.movie_id > 0 {
            Some(item.movie_id)
        } else {
            None
        },
        artist_id: item.artist_id,
        audiobook_id: item.audiobook_id,
        album_id: None,
        seeds: item.seeds,
        leechers: item.leechers,
        seed_count: item.seed_count,
        leech_count: item.leech_count,
        episode: None,
        series: None,
        movie: None,
        artist: None,
        audiobook: None,
        import_progress: None,
    }
}
