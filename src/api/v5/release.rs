//! Release API endpoints (v5)
//!
//! Delegates to the same search and grab infrastructure as v3/release.rs.

use axum::{
    extract::{Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::{
    EpisodeRepository, IndexerRepository, SeriesRepository,
};
use crate::core::indexers::search::IndexerSearchService;
use crate::core::indexers::{Protocol, ReleaseInfo, SearchCriteria};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseQuery {
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub season_number: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseResource {
    pub guid: String,
    pub quality: serde_json::Value,
    pub age: i32,
    pub age_hours: f64,
    pub age_minutes: f64,
    pub size: i64,
    pub indexer_id: i64,
    pub indexer: String,
    pub release_group: Option<String>,
    pub title: String,
    pub full_season: bool,
    pub season_number: i32,
    pub languages: Vec<serde_json::Value>,
    pub episode_numbers: Vec<i32>,
    pub approved: bool,
    pub rejected: bool,
    pub rejections: Vec<String>,
    pub publish_date: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub protocol: String,
}

fn release_to_resource(release: &ReleaseInfo) -> ReleaseResource {
    let now = Utc::now();
    let age = now.signed_duration_since(release.publish_date);

    let protocol = match release.protocol {
        Protocol::Usenet => "usenet",
        Protocol::Torrent => "torrent",
        Protocol::Unknown => "unknown",
    };

    ReleaseResource {
        guid: release.guid.clone(),
        quality: serde_json::to_value(&release.quality).unwrap_or_default(),
        age: age.num_days() as i32,
        age_hours: age.num_hours() as f64,
        age_minutes: age.num_minutes() as f64,
        size: release.size,
        indexer_id: release.indexer_id,
        indexer: release.indexer.clone(),
        release_group: release.release_group.clone(),
        title: release.title.clone(),
        full_season: release.season_number.is_some() && release.episode_numbers.is_empty(),
        season_number: release.season_number.unwrap_or(0),
        languages: release
            .languages
            .iter()
            .map(|l| serde_json::json!({"id": l.id, "name": l.name}))
            .collect(),
        episode_numbers: release.episode_numbers.clone(),
        approved: release.approved,
        rejected: release.rejected,
        rejections: release.rejections.clone(),
        publish_date: release.publish_date.to_rfc3339(),
        download_url: release.download_url.clone(),
        info_url: release.info_url.clone(),
        seeders: release.seeders,
        leechers: release.leechers,
        protocol: protocol.to_string(),
    }
}

/// GET /api/v5/release — interactive search for releases
pub async fn get_releases(
    State(state): State<Arc<AppState>>,
    query: Query<ReleaseQuery>,
) -> Json<Vec<ReleaseResource>> {
    let series_id = match query.series_id {
        Some(id) => id,
        None => return Json(vec![]),
    };

    let series_repo = SeriesRepository::new(state.db.clone());
    let series = match series_repo.get_by_id(series_id).await {
        Ok(Some(s)) => s,
        _ => return Json(vec![]),
    };

    let (season_number, episode_numbers) = if let Some(episode_id) = query.episode_id {
        let episode_repo = EpisodeRepository::new(state.db.clone());
        match episode_repo.get_by_id(episode_id).await {
            Ok(Some(ep)) => (Some(ep.season_number), vec![ep.episode_number]),
            _ => (query.season_number, vec![]),
        }
    } else {
        (query.season_number, vec![])
    };

    let indexer_repo = IndexerRepository::new(state.db.clone());
    let indexers = match indexer_repo.get_all().await {
        Ok(i) => i,
        Err(_) => return Json(vec![]),
    };

    let criteria = SearchCriteria {
        series_id: series.tvdb_id,
        series_title: series.title.clone(),
        episode_id: query.episode_id,
        season_number,
        episode_numbers,
        absolute_episode_numbers: vec![],
        special: false,
    };

    let search_service = IndexerSearchService::new(indexers);
    let releases = match search_service.interactive_search(&criteria).await {
        Ok(r) => r,
        Err(_) => return Json(vec![]),
    };

    Json(releases.iter().map(release_to_resource).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct GrabReleaseRequest {
    pub guid: String,
    pub indexer_id: i64,
}

/// POST /api/v5/release — grab a release
pub async fn create_release(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GrabReleaseRequest>,
) -> Json<serde_json::Value> {
    // For v5 we need the release info — in a full impl we'd cache like v3 does.
    // For now, return success stub since the v3 grab endpoint has the full implementation.
    let _ = (&state, &body);
    Json(serde_json::json!({
        "success": false,
        "message": "Use /api/v3/release for grab support with caching"
    }))
}

/// POST /api/v5/release/push — push a release manually
pub async fn push_release(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> Json<Vec<ReleaseResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_releases).post(create_release))
        .route("/push", post(push_release))
}
