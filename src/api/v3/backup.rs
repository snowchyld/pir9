//! Backup API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResource {
    pub id: i32,
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub backup_type: String,
    pub size: i64,
    pub time: String,
}

/// GET /api/v3/system/backup
pub async fn get_backups() -> Json<Vec<BackupResource>> {
    Json(vec![])
}

/// POST /api/v3/system/backup
pub async fn create_backup() -> Json<BackupResource> {
    Json(BackupResource {
        id: 1,
        name: format!("pir9_backup_{}.zip", chrono::Utc::now().format("%Y%m%d%H%M%S")),
        path: "Backups".to_string(),
        backup_type: "manual".to_string(),
        size: 0,
        time: chrono::Utc::now().to_rfc3339(),
    })
}

/// DELETE /api/v3/system/backup/:id
pub async fn delete_backup(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// POST /api/v3/system/backup/restore/:id
pub async fn restore_backup(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// POST /api/v3/system/backup/restore/upload
pub async fn restore_backup_upload() -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_backups).post(create_backup))
        .route("/{id}", delete(delete_backup))
        .route("/restore/{id}", post(restore_backup))
        .route("/restore/upload", post(restore_backup_upload))
}
