//! Log API endpoints (v5)

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::LogRepository;
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
    pub id: i64,
    pub time: String,
    pub exception: Option<String>,
    pub exception_type: Option<String>,
    pub level: String,
    pub logger: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPagingResource {
    pub page: i32,
    pub page_size: i32,
    pub sort_key: String,
    pub sort_direction: String,
    pub total_records: i64,
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

pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    query: Query<LogQuery>,
) -> Json<LogPagingResource> {
    let repo = LogRepository::new(state.db.clone());

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50);
    let sort_key = query.sort_key.clone().unwrap_or_else(|| "time".to_string());
    let sort_direction = query.sort_direction.clone().unwrap_or_else(|| "descending".to_string());
    let level = query.level.as_deref();

    match repo.get_paged(page, page_size, level, &sort_key, &sort_direction).await {
        Ok((logs, total)) => {
            let records: Vec<LogResource> = logs
                .into_iter()
                .map(|log| LogResource {
                    id: log.id,
                    time: log.time.to_rfc3339(),
                    exception: log.exception,
                    exception_type: log.exception_type,
                    level: log.level,
                    logger: log.logger,
                    message: log.message,
                })
                .collect();

            Json(LogPagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: total,
                records,
            })
        }
        Err(e) => {
            tracing::error!("Failed to fetch logs: {}", e);
            Json(LogPagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: 0,
                records: vec![],
            })
        }
    }
}

pub async fn get_log_files() -> Json<Vec<LogFileResource>> {
    // Log files are primarily for file-based logging (not implemented)
    // Return empty for now - database logging doesn't use file rotation
    Json(vec![])
}

pub async fn delete_logs(State(state): State<Arc<AppState>>) -> StatusCode {
    let repo = LogRepository::new(state.db.clone());

    match repo.clear_all().await {
        Ok(count) => {
            tracing::info!("Deleted {} log entries", count);
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to delete logs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_logs).delete(delete_logs))
        .route("/file", get(get_log_files))
}
