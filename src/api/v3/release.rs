//! Release API endpoints (search results)

use axum::{
    extract::{Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::datastore::models::HistoryDbModel;
use crate::core::datastore::repositories::{
    EpisodeRepository, HistoryRepository, IndexerRepository, SeriesRepository,
};
use crate::core::indexers::search::IndexerSearchService;
use crate::core::indexers::{Protocol, ReleaseInfo, SearchCriteria};
use crate::core::queue::TrackedDownloadService;
use crate::web::AppState;

/// Cache for recent search results - releases are cached by guid+indexer_id
/// Expires after 15 minutes
static RELEASE_CACHE: Lazy<RwLock<ReleaseCache>> = Lazy::new(|| RwLock::new(ReleaseCache::new()));

struct ReleaseCache {
    releases: HashMap<String, CachedRelease>,
}

struct CachedRelease {
    release: ReleaseInfo,
    cached_at: chrono::DateTime<Utc>,
}

impl ReleaseCache {
    fn new() -> Self {
        Self {
            releases: HashMap::new(),
        }
    }

    fn cache_key(guid: &str, indexer_id: i32) -> String {
        format!("{}:{}", indexer_id, guid)
    }

    fn get(&self, guid: &str, indexer_id: i32) -> Option<ReleaseInfo> {
        let key = Self::cache_key(guid, indexer_id);
        if let Some(cached) = self.releases.get(&key) {
            // Check if still valid (15 min expiry)
            if Utc::now()
                .signed_duration_since(cached.cached_at)
                .num_minutes()
                < 15
            {
                return Some(cached.release.clone());
            }
        }
        None
    }

    fn insert(&mut self, release: ReleaseInfo) {
        let key = Self::cache_key(&release.guid, release.indexer_id as i32);
        self.releases.insert(
            key,
            CachedRelease {
                release,
                cached_at: Utc::now(),
            },
        );

        // Clean up expired entries if cache is getting large
        if self.releases.len() > 1000 {
            let now = Utc::now();
            self.releases
                .retain(|_, v| now.signed_duration_since(v.cached_at).num_minutes() < 15);
        }
    }

    fn insert_many(&mut self, releases: &[ReleaseInfo]) {
        for release in releases {
            self.insert(release.clone());
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseQuery {
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub season_number: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseResource {
    pub guid: String,
    pub quality: QualityModel,
    pub quality_weight: i32,
    pub age: i32,
    pub age_hours: f64,
    pub age_minutes: f64,
    pub size: i64,
    pub indexer_id: i32,
    pub indexer: String,
    pub release_group: Option<String>,
    pub sub_group: Option<String>,
    pub release_hash: Option<String>,
    pub title: String,
    pub full_season: bool,
    pub scene_source: bool,
    pub season_number: i32,
    pub languages: Vec<LanguageResource>,
    pub language_weight: i32,
    pub air_date: Option<String>,
    pub series_title: Option<String>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub mapped_episode_numbers: Vec<i32>,
    pub mapped_absolute_episode_numbers: Vec<i32>,
    pub mapped_series_id: Option<i32>,
    pub mapped_season_number: Option<i32>,
    pub approved: bool,
    pub temporarily_rejected: bool,
    pub rejected: bool,
    pub tv_db_id: i32,
    pub tv_rage_id: i32,
    pub rejections: Vec<String>,
    pub publish_date: String,
    pub comment_url: Option<String>,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub episode_requested: bool,
    pub download_allowed: bool,
    pub release_weight: i32,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
    pub scene_mapping: Option<serde_json::Value>,
    pub indexer_flags: i32,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub protocol: String,
    pub is_daily: bool,
    pub is_absolute_numbering: bool,
    pub is_possible_special_episode: bool,
    pub special: bool,
    #[serde(default)]
    pub categories: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

/// Convert ReleaseInfo to ReleaseResource
fn release_to_resource(release: &ReleaseInfo) -> ReleaseResource {
    let now = Utc::now();
    let age = now.signed_duration_since(release.publish_date);
    let age_days = age.num_days() as i32;
    let age_hours = age.num_hours() as f64;
    let age_minutes = age.num_minutes() as f64;

    let protocol = match release.protocol {
        Protocol::Usenet => "usenet",
        Protocol::Torrent => "torrent",
        Protocol::Unknown => "unknown",
    };

    ReleaseResource {
        guid: release.guid.clone(),
        quality: QualityModel {
            quality: QualityResource {
                id: release.quality.quality.weight(),
                name: format!("{:?}", release.quality.quality),
                source: "unknown".to_string(),
                resolution: release.quality.quality.resolution_width(),
            },
            revision: RevisionResource {
                version: release.quality.revision.version,
                real: release.quality.revision.real,
                is_repack: release.quality.revision.is_repack,
            },
        },
        quality_weight: release.quality.quality.weight(),
        age: age_days,
        age_hours,
        age_minutes,
        size: release.size,
        indexer_id: release.indexer_id as i32,
        indexer: release.indexer.clone(),
        release_group: release.release_group.clone(),
        sub_group: None,
        release_hash: release.release_hash.clone(),
        title: release.title.clone(),
        full_season: release.season_number.is_some() && release.episode_numbers.is_empty(),
        scene_source: false,
        season_number: release.season_number.unwrap_or(0),
        languages: release
            .languages
            .iter()
            .map(|l| LanguageResource {
                id: l.id,
                name: l.name.clone(),
            })
            .collect(),
        language_weight: 1,
        air_date: None,
        series_title: release.series_title.clone(),
        episode_numbers: release.episode_numbers.clone(),
        absolute_episode_numbers: release.absolute_episode_numbers.clone(),
        mapped_episode_numbers: release.mapped_episode_numbers.clone(),
        mapped_absolute_episode_numbers: release.mapped_absolute_episode_numbers.clone(),
        mapped_series_id: release.series_id.map(|id| id as i32),
        mapped_season_number: release.mapped_season_number,
        approved: release.approved,
        temporarily_rejected: release.temporarily_rejected,
        rejected: release.rejected,
        tv_db_id: release.tvdb_id.unwrap_or(0) as i32,
        tv_rage_id: release.tv_rage_id.unwrap_or(0) as i32,
        rejections: release.rejections.clone(),
        publish_date: release.publish_date.to_rfc3339(),
        comment_url: release.comment_url.clone(),
        download_url: release.download_url.clone(),
        info_url: release.info_url.clone(),
        episode_requested: release.episode_requested,
        download_allowed: true,
        release_weight: release.quality.quality.weight(),
        custom_formats: vec![],
        custom_format_score: 0,
        scene_mapping: None,
        indexer_flags: release.indexer_flags,
        seeders: release.seeders,
        leechers: release.leechers,
        protocol: protocol.to_string(),
        is_daily: release.is_daily,
        is_absolute_numbering: release.is_absolute_numbering,
        is_possible_special_episode: release.is_possible_special_episode,
        special: release.special,
        categories: release.categories.clone(),
    }
}

/// GET /api/v3/release
/// Search for releases (interactive search)
pub async fn get_releases(
    State(state): State<Arc<AppState>>,
    query: Query<ReleaseQuery>,
) -> Json<Vec<ReleaseResource>> {
    // If no series_id provided, return empty
    let series_id = match query.series_id {
        Some(id) => id,
        None => return Json(vec![]),
    };

    // Get series from database to get TVDB ID and title
    let series_repo = SeriesRepository::new(state.db.clone());
    let series = match series_repo.get_by_id(series_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            tracing::error!("Series {} not found", series_id);
            return Json(vec![]);
        }
        Err(e) => {
            tracing::error!("Failed to fetch series: {}", e);
            return Json(vec![]);
        }
    };

    // If episode_id provided, fetch episode details for proper season/episode numbers
    let (season_number, episode_numbers) = if let Some(episode_id) = query.episode_id {
        let episode_repo = EpisodeRepository::new(state.db.clone());
        match episode_repo.get_by_id(episode_id).await {
            Ok(Some(ep)) => (Some(ep.season_number), vec![ep.episode_number]),
            Ok(None) => {
                tracing::warn!("Episode {} not found, using query params", episode_id);
                (query.season_number, vec![])
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch episode {}: {}, using query params",
                    episode_id,
                    e
                );
                (query.season_number, vec![])
            }
        }
    } else {
        (query.season_number, vec![])
    };

    // Get indexers from database
    let indexer_repo = IndexerRepository::new(state.db.clone());
    let indexers = match indexer_repo.get_all().await {
        Ok(i) => i,
        Err(e) => {
            tracing::error!("Failed to fetch indexers: {}", e);
            return Json(vec![]);
        }
    };

    if indexers.is_empty() {
        tracing::debug!("No indexers configured, returning empty results");
        return Json(vec![]);
    }

    // Build search criteria with TVDB ID and series title
    let criteria = SearchCriteria {
        series_id: series.tvdb_id,          // Use TVDB ID for indexer search
        series_title: series.title.clone(), // Text search for all indexers
        episode_id: query.episode_id,
        season_number,
        episode_numbers,
        absolute_episode_numbers: vec![],
        special: false,
    };

    // Perform search
    let search_service = IndexerSearchService::new(indexers);
    let releases = match search_service.interactive_search(&criteria).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Search failed: {}", e);
            return Json(vec![]);
        }
    };

    tracing::info!("Interactive search returned {} releases", releases.len());

    // Stamp the pir9 series_id onto each release so grab can track it
    let releases: Vec<ReleaseInfo> = releases
        .into_iter()
        .map(|mut r| {
            r.series_id = Some(series_id);
            r
        })
        .collect();

    // Cache results for grab
    {
        let mut cache = RELEASE_CACHE.write().await;
        cache.insert_many(&releases);
    }

    // Convert to resources
    let resources: Vec<ReleaseResource> = releases.iter().map(release_to_resource).collect();

    Json(resources)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabReleaseRequest {
    pub guid: String,
    pub indexer_id: i32,
    /// Episode IDs to associate with this grab (optional, will be parsed from release if not provided)
    #[serde(default)]
    pub episode_ids: Vec<i64>,
}

/// POST /api/v3/release
/// Grab a release (download it)
pub async fn create_release(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GrabReleaseRequest>,
) -> Json<serde_json::Value> {
    tracing::info!(
        "Grab request for release: {} from indexer {}",
        body.guid,
        body.indexer_id
    );

    // Look up the release from cache
    let release = {
        let cache = RELEASE_CACHE.read().await;
        cache.get(&body.guid, body.indexer_id)
    };

    let release = match release {
        Some(r) => r,
        None => {
            tracing::warn!("Release {} not found in cache, searching...", body.guid);

            // Try to re-fetch from indexer
            let indexer_repo = IndexerRepository::new(state.db.clone());
            let _indexer = match indexer_repo.get_by_id(body.indexer_id as i64).await {
                Ok(Some(i)) => i,
                _ => {
                    return Json(serde_json::json!({
                        "success": false,
                        "message": "Indexer not found"
                    }));
                }
            };

            // We can't search without knowing what to search for
            // In a full implementation, we'd store enough info to re-fetch
            return Json(serde_json::json!({
                "success": false,
                "message": "Release expired from cache. Please search again."
            }));
        }
    };

    // Get episode IDs - use provided ones or parse from release
    let episode_ids = if !body.episode_ids.is_empty() {
        body.episode_ids
    } else if let Some(series_id) = release.series_id {
        // Try to look up episodes based on parsed info
        let episode_repo = EpisodeRepository::new(state.db.clone());
        let mut ids = Vec::new();

        if let Some(season) = release.season_number {
            for &ep_num in &release.episode_numbers {
                if let Ok(Some(ep)) = episode_repo
                    .get_by_series_season_episode(series_id, season, ep_num)
                    .await
                {
                    ids.push(ep.id);
                }
            }
        }
        ids
    } else {
        vec![]
    };

    // Grab the release using TrackedDownloadService
    let service = TrackedDownloadService::new(state.db.clone());
    match service.grab_release(&release, episode_ids.clone(), None).await {
        Ok(tracked_id) => {
            tracing::info!("Release grabbed and tracked: id={}", tracked_id);

            // Record in history
            if let Some(series_id) = release.series_id {
                let history_repo = HistoryRepository::new(state.db.clone());
                let episode_id = episode_ids.first().copied().unwrap_or(0);

                // Skip history for movie grabs (no valid episode_id)
                if episode_id > 0 {
                    let history = HistoryDbModel {
                        id: 0,
                        series_id: Some(series_id),
                        episode_id: Some(episode_id),
                        movie_id: None,
                        source_title: release.title.clone(),
                        quality: serde_json::to_string(&release.quality).unwrap_or_default(),
                        languages: serde_json::to_string(&release.languages).unwrap_or_default(),
                        custom_formats: "[]".to_string(),
                        custom_format_score: 0,
                        quality_cutoff_not_met: false,
                        date: Utc::now(),
                        download_id: Some(format!("{}", tracked_id)),
                        event_type: 1, // Grabbed
                        data: serde_json::json!({
                            "indexer": release.indexer,
                            "releaseGroup": release.release_group,
                            "size": release.size,
                            "downloadClient": "auto",
                        })
                        .to_string(),
                    };

                    if let Err(e) = history_repo.insert(&history).await {
                        tracing::warn!("Failed to record grab in history: {}", e);
                    }
                }
            }

            crate::core::logging::log_info(
                "ReleaseGrabbed",
                &format!("Grabbed release: {}", release.title),
            )
            .await;

            // Publish event for UI refresh
            state
                .event_bus
                .publish(crate::core::messaging::Message::ReleaseGrabbed {
                    download_id: format!("{}", tracked_id),
                    series_id: release.series_id.unwrap_or(0),
                    episode_ids: episode_ids.clone(),
                    release_title: release.title.clone(),
                    indexer: release.indexer.clone(),
                    size: release.size,
                })
                .await;

            Json(serde_json::json!({
                "success": true,
                "message": "Release grabbed successfully",
                "trackedDownloadId": tracked_id
            }))
        }
        Err(e) => {
            tracing::error!("Failed to grab release: {}", e);

            crate::core::logging::log_error(
                "ReleaseGrabFailed",
                &format!("Failed to grab release {}: {}", release.title, e),
            )
            .await;

            Json(serde_json::json!({
                "success": false,
                "message": format!("Failed to grab release: {}", e)
            }))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PushReleaseRequest {
    pub title: String,
    pub download_url: Option<String>,
    pub protocol: String,
    pub publish_date: Option<String>,
}

/// POST /api/v3/release/push
/// Push a release manually
pub async fn push_release(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<PushReleaseRequest>,
) -> Json<Vec<ReleaseResource>> {
    tracing::info!("Push release request: {}", body.title);

    // This would parse the release title and match it to a series/episode
    // For now, return empty
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_releases).post(create_release))
        .route("/push", post(push_release))
}
