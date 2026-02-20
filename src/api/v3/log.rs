//! Log API endpoints

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub sort_key: Option<String>,
    pub sort_direction: Option<String>,
    pub level: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogResource {
    pub id: i32,
    pub time: String,
    pub exception: Option<String>,
    pub exception_type: Option<String>,
    pub level: String,
    pub logger: String,
    pub message: String,
    pub method: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogResourcePagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i32,
    pub records: Vec<LogResource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFileResource {
    pub id: i32,
    pub filename: String,
    pub last_write_time: String,
    pub contents_url: String,
    pub download_url: String,
}

/// GET /api/v3/log
pub async fn get_logs(query: Query<LogQuery>) -> Json<LogResourcePagingResource> {
    Json(LogResourcePagingResource {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(50),
        sort_key: query.sort_key.clone().unwrap_or("time".to_string()),
        sort_direction: query.sort_direction.clone().unwrap_or("descending".to_string()),
        total_records: 0,
        records: vec![],
    })
}

/// GET /api/v3/log/file
pub async fn get_log_files() -> Json<Vec<LogFileResource>> {
    Json(vec![])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_logs))
        .route("/file", get(get_log_files))
}
