//! Per-content-type tracked download bookmark types.
//!
//! A tracked download is a **bookmark** linking a download client's ID
//! (info_hash, nzo_id) to a pir9 content entity (series, movie, artist, etc.).
//! Only fields that genuinely need persistence are stored here — runtime state
//! like download progress, status, and output path are derived from live
//! download client polling.

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ContentRef trait — implemented by each content-type's FK struct
// ---------------------------------------------------------------------------

/// Marker trait for the content-specific payload inside a `TrackedDownload`.
pub trait ContentRef: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {
    /// The content type name used in API responses and JSONL filenames.
    fn content_type_name() -> &'static str;
}

// ---------------------------------------------------------------------------
// Per-content reference types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesRef {
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
}

impl ContentRef for SeriesRef {
    fn content_type_name() -> &'static str {
        "series"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieRef {
    pub movie_id: i64,
}

impl ContentRef for MovieRef {
    fn content_type_name() -> &'static str {
        "movie"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicRef {
    pub artist_id: i64,
}

impl ContentRef for MusicRef {
    fn content_type_name() -> &'static str {
        "music"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookRef {
    pub audiobook_id: i64,
}

impl ContentRef for AudiobookRef {
    fn content_type_name() -> &'static str {
        "audiobook"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastRef {
    pub podcast_id: i64,
}

impl ContentRef for PodcastRef {
    fn content_type_name() -> &'static str {
        "podcast"
    }
}

/// Placeholder for suppressed (soft-removed) untracked downloads.
/// Only `download_id` + `client_id` + `added` matter — no content FK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressedRef;

impl ContentRef for SuppressedRef {
    fn content_type_name() -> &'static str {
        "suppressed"
    }
}

// ---------------------------------------------------------------------------
// TrackedDownload<C> — the persisted bookmark
// ---------------------------------------------------------------------------

/// A tracked download bookmark. Only fields that **must** persist across
/// restarts are stored here. Runtime state (download progress, status,
/// output path, error messages) is derived from live download client polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "C: DeserializeOwned"))]
pub struct TrackedDownload<C: ContentRef> {
    /// Stable monotonic ID (assigned by the store).
    pub id: i64,
    /// ID from the download client (info_hash for torrents, nzo_id for usenet).
    pub download_id: String,
    /// Which download client owns this download.
    pub client_id: i64,
    /// Content-type-specific foreign key(s).
    pub content: C,
    /// Release title — used for display when the download client is unavailable.
    pub title: String,
    /// JSON-serialized `QualityModel` — used for duplicate prevention during RSS sync.
    pub quality: String,
    /// Indexer name (for display).
    #[serde(default)]
    pub indexer: Option<String>,
    /// When this download was first tracked.
    pub added: DateTime<Utc>,
    /// Whether this download is an upgrade over an existing file.
    #[serde(default)]
    pub is_upgrade: bool,
}

// ---------------------------------------------------------------------------
// AnyTrackedDownload — type-erased wrapper for cross-store lookups
// ---------------------------------------------------------------------------

/// Type-erased tracked download for operations that span all stores
/// (e.g. "find by ID across all content types").
#[derive(Debug, Clone)]
pub struct AnyTrackedDownload {
    pub id: i64,
    pub download_id: String,
    pub client_id: i64,
    pub content_type: &'static str,
    pub title: String,
    pub quality: String,
    pub indexer: Option<String>,
    pub added: DateTime<Utc>,
    pub is_upgrade: bool,
    // Content-specific FKs (only the relevant ones are non-zero/non-empty)
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub movie_id: i64,
    pub artist_id: i64,
    pub audiobook_id: i64,
    pub podcast_id: i64,
}

impl<C: ContentRef> TrackedDownload<C> {
    /// Convert to a type-erased representation.
    /// Content-specific fields default to 0/empty; callers populate them
    /// via the `From` impls below.
    fn to_any_base(&self) -> AnyTrackedDownload {
        AnyTrackedDownload {
            id: self.id,
            download_id: self.download_id.clone(),
            client_id: self.client_id,
            content_type: C::content_type_name(),
            title: self.title.clone(),
            quality: self.quality.clone(),
            indexer: self.indexer.clone(),
            added: self.added,
            is_upgrade: self.is_upgrade,
            series_id: 0,
            episode_ids: vec![],
            movie_id: 0,
            artist_id: 0,
            audiobook_id: 0,
            podcast_id: 0,
        }
    }
}

impl From<&TrackedDownload<SeriesRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<SeriesRef>) -> Self {
        let mut any = td.to_any_base();
        any.series_id = td.content.series_id;
        any.episode_ids = td.content.episode_ids.clone();
        any
    }
}

impl From<&TrackedDownload<MovieRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<MovieRef>) -> Self {
        let mut any = td.to_any_base();
        any.movie_id = td.content.movie_id;
        any
    }
}

impl From<&TrackedDownload<MusicRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<MusicRef>) -> Self {
        let mut any = td.to_any_base();
        any.artist_id = td.content.artist_id;
        any
    }
}

impl From<&TrackedDownload<AudiobookRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<AudiobookRef>) -> Self {
        let mut any = td.to_any_base();
        any.audiobook_id = td.content.audiobook_id;
        any
    }
}

impl From<&TrackedDownload<PodcastRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<PodcastRef>) -> Self {
        let mut any = td.to_any_base();
        any.podcast_id = td.content.podcast_id;
        any
    }
}

impl From<&TrackedDownload<SuppressedRef>> for AnyTrackedDownload {
    fn from(td: &TrackedDownload<SuppressedRef>) -> Self {
        td.to_any_base()
    }
}
