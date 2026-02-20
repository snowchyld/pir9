//! History API endpoints (v5)

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::datastore::models::HistoryDbModel;
use crate::core::datastore::repositories::{HistoryRepository, SeriesRepository};
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct HistoryQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
    pub event_type: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResource {
    pub id: i64,
    pub episode_id: i64,
    pub series_id: i64,
    pub series_title_slug: Option<String>,
    pub source_title: String,
    pub languages: serde_json::Value,
    pub quality: serde_json::Value,
    pub custom_formats: serde_json::Value,
    pub custom_format_score: i32,
    pub quality_cutoff_not_met: bool,
    pub date: String,
    pub download_id: Option<String>,
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
    pub records: Vec<HistoryResource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySinceQuery {
    pub date: Option<String>,
    pub event_type: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySeriesQuery {
    pub series_id: Option<i64>,
    pub event_type: Option<i32>,
}

pub async fn get_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryPagingResource>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let sort_key = query.sort_key.unwrap_or_else(|| "date".to_string());
    let sort_direction = query
        .sort_direction
        .unwrap_or_else(|| "descending".to_string());

    let repo = HistoryRepository::new(state.db.clone());
    let (items, total) = repo
        .get_paged(
            page,
            page_size,
            &sort_key,
            &sort_direction,
            query.event_type,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut records: Vec<HistoryResource> = items.iter().map(db_to_resource).collect();
    enrich_series_slugs(&state, &mut records).await;

    Ok(Json(HistoryPagingResource {
        page,
        page_size,
        sort_key,
        sort_direction,
        total_records: total,
        records,
    }))
}

pub async fn get_history_since(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistorySinceQuery>,
) -> Result<Json<Vec<HistoryResource>>, StatusCode> {
    let repo = HistoryRepository::new(state.db.clone());

    let date = query
        .date
        .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| chrono::Utc::now() - chrono::Duration::days(7));

    let items = repo
        .get_since(date)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut records: Vec<HistoryResource> = items.iter().map(db_to_resource).collect();

    if let Some(evt) = query.event_type {
        let evt_str = event_type_to_string(evt);
        records.retain(|r| r.event_type == evt_str);
    }

    Ok(Json(records))
}

pub async fn get_history_series(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistorySeriesQuery>,
) -> Result<Json<Vec<HistoryResource>>, StatusCode> {
    let series_id = query.series_id.unwrap_or(0);
    if series_id == 0 {
        return Ok(Json(vec![]));
    }

    let repo = HistoryRepository::new(state.db.clone());
    let items = repo
        .get_by_series_id(series_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut records: Vec<HistoryResource> = items.iter().map(db_to_resource).collect();

    if let Some(evt) = query.event_type {
        let evt_str = event_type_to_string(evt);
        records.retain(|r| r.event_type == evt_str);
    }

    Ok(Json(records))
}

fn event_type_to_string(event_type: i32) -> String {
    match event_type {
        1 => "grabbed".to_string(),
        2 => "downloadFailed".to_string(),
        3 => "downloadFolderImported".to_string(),
        4 => "episodeFileDeleted".to_string(),
        5 => "episodeFileRenamed".to_string(),
        6 => "downloadIgnored".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Look up title slugs for all unique series IDs and populate `series_title_slug`.
async fn enrich_series_slugs(state: &AppState, records: &mut [HistoryResource]) {
    let series_ids: Vec<i64> = records
        .iter()
        .map(|r| r.series_id)
        .filter(|id| *id > 0)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if series_ids.is_empty() {
        return;
    }

    let series_repo = SeriesRepository::new(state.db.clone());
    let mut slug_map: HashMap<i64, String> = HashMap::new();

    for id in series_ids {
        if let Ok(Some(s)) = series_repo.get_by_id(id).await {
            slug_map.insert(id, s.title_slug);
        }
    }

    for record in records.iter_mut() {
        if let Some(slug) = slug_map.get(&record.series_id) {
            record.series_title_slug = Some(slug.clone());
        }
    }
}

fn db_to_resource(model: &HistoryDbModel) -> HistoryResource {
    let quality: serde_json::Value =
        serde_json::from_str(&model.quality).unwrap_or(serde_json::json!({}));
    let languages: serde_json::Value =
        serde_json::from_str(&model.languages).unwrap_or(serde_json::json!([]));
    let custom_formats: serde_json::Value =
        serde_json::from_str(&model.custom_formats).unwrap_or(serde_json::json!([]));
    let data: serde_json::Value =
        serde_json::from_str(&model.data).unwrap_or(serde_json::json!({}));

    HistoryResource {
        id: model.id,
        episode_id: model.episode_id,
        series_id: model.series_id,
        series_title_slug: None,
        source_title: model.source_title.clone(),
        languages,
        quality,
        custom_formats,
        custom_format_score: model.custom_format_score,
        quality_cutoff_not_met: model.quality_cutoff_not_met,
        date: model.date.to_rfc3339(),
        download_id: model.download_id.clone(),
        event_type: event_type_to_string(model.event_type),
        data,
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_history))
        .route("/since", get(get_history_since))
        .route("/series", get(get_history_series))
}
