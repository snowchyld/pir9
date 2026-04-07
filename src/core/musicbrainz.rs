//! MusicBrainz service client
//!
//! HTTP client for the pir9-musicbrainz microservice, which maintains a local
//! PostgreSQL database of MusicBrainz data (artists, albums, tracks).
//! This client is shared via AppState so all layers can query MusicBrainz.

use serde::{Deserialize, Serialize};
use tracing::warn;

/// HTTP client for the pir9-musicbrainz microservice
#[derive(Clone)]
pub struct MusicBrainzClient {
    base_url: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for MusicBrainzClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MusicBrainzClient")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl MusicBrainzClient {
    /// Create a new client from the `PIR9_MB_URL` environment variable.
    /// Returns `None` if the env var is not set, meaning the service is disabled.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("PIR9_MB_URL").ok()?;
        if base_url.is_empty() {
            return None;
        }
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        })
    }

    /// Search artists by name
    pub async fn search_artists(
        &self,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<MbArtist>> {
        let url = format!(
            "{}/api/artists/search?q={}&limit={}",
            self.base_url,
            urlencoding::encode(query),
            limit
        );
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(response) => {
                warn!("MusicBrainz service search returned error: {}", response.status());
                Ok(vec![])
            }
            Err(e) => {
                warn!("Failed to connect to MusicBrainz service: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Get a single artist by MusicBrainz ID (MBID)
    pub async fn get_artist(&self, mbid: &str) -> anyhow::Result<Option<MbArtist>> {
        let url = format!("{}/api/artists/{}", self.base_url, mbid);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("MusicBrainz service returned error: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to MusicBrainz service: {}", e);
                Ok(None)
            }
        }
    }

    /// Get albums for an artist by MusicBrainz ID
    pub async fn get_artist_albums(&self, mbid: &str) -> anyhow::Result<Vec<MbAlbum>> {
        let url = format!("{}/api/artists/{}/albums", self.base_url, mbid);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    /// Get a single album by MusicBrainz ID
    pub async fn get_album(&self, mbid: &str) -> anyhow::Result<Option<MbAlbum>> {
        let url = format!("{}/api/albums/{}", self.base_url, mbid);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(response) if response.status().as_u16() == 404 => Ok(None),
            Ok(response) => {
                warn!("MusicBrainz service returned error: {}", response.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to connect to MusicBrainz service: {}", e);
                Ok(None)
            }
        }
    }

    /// Get track listing for a specific release MBID
    pub async fn get_release_tracks(&self, release_mbid: &str) -> anyhow::Result<Vec<MbReleaseTrack>> {
        let url = format!("{}/api/releases/{}/tracks", self.base_url, release_mbid);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    /// Get releases (editions) for an album by release group MBID
    pub async fn get_album_releases(&self, release_group_mbid: &str) -> anyhow::Result<Vec<MbRelease>> {
        let url = format!("{}/api/albums/{}/releases", self.base_url, release_group_mbid);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                Ok(response.json().await.unwrap_or_default())
            }
            Ok(_) | Err(_) => Ok(vec![]),
        }
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> anyhow::Result<Option<MbStats>> {
        let url = format!("{}/api/stats", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => Ok(response.json().await.ok()),
            Ok(_) | Err(_) => Ok(None),
        }
    }

    /// Trigger a MusicBrainz data sync
    pub async fn trigger_sync(&self) -> anyhow::Result<MbProxyResponse> {
        self.trigger_sync_selective(&[]).await
    }

    /// Trigger a MusicBrainz data sync for specific datasets
    pub async fn trigger_sync_selective(&self, datasets: &[String]) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/sync", self.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/sync/status", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }

    /// Get dataset metadata (sizes, cache status)
    pub async fn get_datasets(&self) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/datasets", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }

    /// Download datasets to cache without processing
    pub async fn start_download(&self, datasets: &[String]) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/download", self.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }

    /// Process cached datasets without re-downloading
    pub async fn start_process(&self, datasets: &[String]) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/process", self.base_url);
        match self
            .client
            .post(&url)
            .json(&serde_json::json!({ "datasets": datasets }))
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }

    /// Cancel a running sync
    pub async fn cancel_sync(&self) -> anyhow::Result<MbProxyResponse> {
        let url = format!("{}/api/sync/cancel", self.base_url);
        match self.client.post(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body: serde_json::Value = response
                    .json()
                    .await
                    .unwrap_or_else(|_| serde_json::json!({ "error": "Failed to parse response" }));
                Ok(MbProxyResponse { status, body })
            }
            Err(e) => Ok(MbProxyResponse {
                status: 502,
                body: serde_json::json!({ "error": format!("MusicBrainz service unavailable: {}", e) }),
            }),
        }
    }
}

pub struct MbProxyResponse {
    pub status: u16,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbArtist {
    pub mbid: String,
    pub name: String,
    #[serde(default)]
    pub sort_name: Option<String>,
    #[serde(default)]
    pub artist_type: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub disambiguation: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub begin_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub rating_count: Option<i32>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub images: Vec<MbImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbAlbum {
    pub mbid: String,
    pub title: String,
    #[serde(default)]
    pub artist_mbid: Option<String>,
    /// Primary type — deserialized from either "albumType" or "primaryType" (MusicBrainz service)
    #[serde(default, alias = "primaryType")]
    pub album_type: Option<String>,
    /// Secondary types (Compilation, Soundtrack, Live, etc.)
    #[serde(default)]
    pub secondary_types: Vec<String>,
    #[serde(default, alias = "firstReleaseDate")]
    pub release_date: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub images: Vec<MbImage>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub rating_count: Option<i32>,
    /// Cover art URL from Cover Art Archive (via MusicBrainz service)
    #[serde(default)]
    pub cover_art_url: Option<String>,
}

/// A track within a release (position + recording info)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbReleaseTrack {
    pub disc_number: i32,
    pub position: i32,
    pub title: String,
    #[serde(default)]
    pub recording_mbid: Option<String>,
    #[serde(default)]
    pub length_ms: Option<i32>,
}

/// A specific release (edition) of a release group (album)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbRelease {
    pub mbid: String,
    pub title: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub packaging: Option<String>,
    #[serde(default)]
    pub track_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbImage {
    pub cover_type: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MbStats {
    pub artist_count: i64,
    #[serde(default)]
    pub release_group_count: i64,
    #[serde(default)]
    pub release_count: i64,
    #[serde(default)]
    pub cover_art_count: i64,
    #[serde(default)]
    pub label_count: i64,
    #[serde(default)]
    pub recording_count: i64,
    #[serde(default)]
    pub work_count: i64,
    #[serde(default)]
    pub area_count: i64,
    #[serde(default)]
    pub series_count: i64,
    #[serde(default)]
    pub event_count: i64,
    #[serde(default)]
    pub instrument_count: i64,
    #[serde(default)]
    pub place_count: i64,
    #[serde(default)]
    pub last_sync: Option<String>,
    #[serde(default)]
    pub db_size_bytes: Option<i64>,
}
