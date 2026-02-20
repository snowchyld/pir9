//! History API endpoints (v5)

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResource {
    pub id: i32,
    pub episode_id: i32,
    pub series_id: i32,
    pub source_title: String,
    pub languages: Vec<LanguageResource>,
    pub quality: QualityModel,
    pub date: String,
    pub download_id: Option<String>,
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<HistoryResource>,
}

pub async fn get_history(query: Query<HistoryQuery>) -> Json<HistoryPagingResource> {
    Json(HistoryPagingResource {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_key: query.sort_key.clone().unwrap_or("date".to_string()),
        sort_direction: query.sort_direction.clone().unwrap_or("descending".to_string()),
        total_records: 0,
        records: vec![],
    })
}

pub async fn get_history_since() -> Json<Vec<HistoryResource>> {
    Json(vec![])
}

pub async fn get_history_series() -> Json<Vec<HistoryResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_history))
        .route("/since", get(get_history_since))
        .route("/series", get(get_history_series))
}
