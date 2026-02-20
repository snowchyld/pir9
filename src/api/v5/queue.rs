//! Queue API endpoints

use axum::{
    Router,
    routing::get,
    extract::Query,
    Json,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use crate::web::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_queue).delete(remove_from_queue))
        .route("/status", get(get_queue_status))
        .route("/details", get(get_queue_details))
        .route("/{id}", get(get_queue_item).delete(remove_queue_item))
        .route("/{id}/grab", get(grab_release))
}

async fn list_queue(
    Query(params): Query<QueueListQuery>,
) -> Json<QueueResponse> {
    Json(QueueResponse {
        page: params.page.unwrap_or(1),
        page_size: params.page_size.unwrap_or(20),
        sort_key: params.sort_key.unwrap_or_else(|| "timeleft".to_string()),
        sort_direction: params.sort_direction.unwrap_or_else(|| "ascending".to_string()),
        total_records: 0,
        records: vec![],
    })
}

async fn get_queue_status() -> Json<QueueStatusResource> {
    Json(QueueStatusResource {
        total_count: 0,
        count: 0,
        unknown_count: 0,
        errors: false,
        warnings: false,
        unknown_errors: false,
        unknown_warnings: false,
    })
}

async fn get_queue_details(
    Query(params): Query<QueueDetailsQuery>,
) -> Json<Vec<QueueItem>> {
    Json(vec![])
}

async fn get_queue_item() -> Json<Option<QueueItem>> {
    Json(None)
}

async fn remove_queue_item() -> Json<QueueActionResponse> {
    Json(QueueActionResponse { success: true })
}

async fn remove_from_queue(
    Query(_params): Query<RemoveFromQueueQuery>,
) -> Json<QueueActionResponse> {
    Json(QueueActionResponse { success: true })
}

async fn grab_release() -> Json<QueueActionResponse> {
    Json(QueueActionResponse { success: true })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueListQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueDetailsQuery {
    pub series_id: Option<i32>,
    pub episode_ids: Option<String>,
    pub include_series: Option<bool>,
    pub include_episode: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveFromQueueQuery {
    pub blacklist: Option<bool>,
    pub skip_redownload: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResponse {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
    pub records: Vec<QueueItem>,
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
pub struct QueueItem {
    pub id: i64,
    pub series_id: i64,
    pub episode_id: i64,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub title: String,
    pub status: String,
    pub tracked_download_status: String,
    pub status_messages: Vec<StatusMessage>,
    pub error_message: Option<String>,
    pub download_id: Option<String>,
    pub protocol: String,
    pub download_client: String,
    pub indexer: String,
    pub output_path: Option<String>,
    pub episode_has_file: bool,
    pub size: i64,
    pub sizeleft: i64,
    pub timeleft: Option<String>,
    pub estimated_completion_time: Option<DateTime<Utc>>,
    pub added: DateTime<Utc>,
    pub quality: QualityModel,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusMessage {
    pub title: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: Quality,
    pub revision: Revision,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueActionResponse {
    pub success: bool,
}
