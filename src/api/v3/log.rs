//! Log API endpoints

use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::LogRepository;
use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
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
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    query: Query<LogQuery>,
) -> Json<LogResourcePagingResource> {
    let repo = LogRepository::new(state.db.clone());

    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50);
    let sort_key = query.sort_key.clone().unwrap_or_else(|| "time".to_string());
    let sort_direction = query
        .sort_direction
        .clone()
        .unwrap_or_else(|| "descending".to_string());
    let level = query.level.as_deref();

    match repo
        .get_paged(page, page_size, level, &sort_key, &sort_direction)
        .await
    {
        Ok((logs, total)) => {
            let records: Vec<LogResource> = logs
                .into_iter()
                .map(|log| LogResource {
                    id: log.id as i32,
                    time: log.time.to_rfc3339(),
                    exception: log.exception,
                    exception_type: log.exception_type,
                    level: log.level,
                    logger: log.logger,
                    message: log.message,
                    method: None,
                })
                .collect();

            Json(LogResourcePagingResource {
                page,
                page_size,
                sort_key,
                sort_direction,
                total_records: total as i32,
                records,
            })
        }
        Err(e) => {
            tracing::error!("Failed to fetch logs: {}", e);
            Json(LogResourcePagingResource {
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

/// GET /api/v3/log/file
pub async fn get_log_files(State(state): State<Arc<AppState>>) -> Json<Vec<LogFileResource>> {
    let log_dir = &state.config.paths.log_dir;
    let mut files = Vec::new();
    let mut id = 1;

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log")
                || path.extension().and_then(|e| e.to_str()) == Some("txt")
            {
                if let Ok(meta) = entry.metadata() {
                    let modified = meta
                        .modified()
                        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                        .unwrap_or_default();
                    let filename = entry.file_name().to_string_lossy().to_string();
                    files.push(LogFileResource {
                        id,
                        filename: filename.clone(),
                        last_write_time: modified,
                        contents_url: format!("/api/v3/log/file/{}", filename),
                        download_url: format!("/api/v3/log/file/{}", filename),
                    });
                    id += 1;
                }
            }
        }
    }

    Json(files)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_logs))
        .route("/file", get(get_log_files))
}
