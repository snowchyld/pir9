//! Backup API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
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
pub async fn get_backups(State(state): State<Arc<AppState>>) -> Json<Vec<BackupResource>> {
    let backup_dir = &state.config.paths.backup_dir;
    let mut backups = Vec::new();

    if let Ok(entries) = std::fs::read_dir(backup_dir) {
        for (idx, entry) in entries.flatten().enumerate() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("zip")
                || path.extension().and_then(|e| e.to_str()) == Some("sql")
            {
                if let Ok(meta) = entry.metadata() {
                    let modified = meta
                        .modified()
                        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                        .unwrap_or_default();
                    backups.push(BackupResource {
                        id: (idx + 1) as i32,
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                        backup_type: "manual".to_string(),
                        size: meta.len() as i64,
                        time: modified,
                    });
                }
            }
        }
    }

    Json(backups)
}

/// POST /api/v3/system/backup
pub async fn create_backup(State(state): State<Arc<AppState>>) -> Json<BackupResource> {
    let backup_dir = &state.config.paths.backup_dir;
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("pir9_backup_{}.sql", timestamp);
    let filepath = backup_dir.join(&filename);

    let _ = std::fs::create_dir_all(backup_dir);

    let conn = &state.config.database.connection_string;
    let result = tokio::process::Command::new("pg_dump")
        .arg(conn)
        .arg("--no-owner")
        .arg("--no-acl")
        .arg("-f")
        .arg(&filepath)
        .output()
        .await;

    let size = match result {
        Ok(output) if output.status.success() => {
            tracing::info!("Backup created: {}", filepath.display());
            std::fs::metadata(&filepath)
                .map(|m| m.len() as i64)
                .unwrap_or(0)
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("pg_dump failed: {}", stderr);
            0
        }
        Err(e) => {
            tracing::error!("Failed to run pg_dump: {}", e);
            0
        }
    };

    Json(BackupResource {
        id: 1,
        name: filename,
        path: filepath.to_string_lossy().to_string(),
        backup_type: "manual".to_string(),
        size,
        time: chrono::Utc::now().to_rfc3339(),
    })
}

/// DELETE /api/v3/system/backup/:id
pub async fn delete_backup(State(state): State<Arc<AppState>>, Path(id): Path<i32>) -> StatusCode {
    let backup_dir = &state.config.paths.backup_dir;

    if let Ok(entries) = std::fs::read_dir(backup_dir) {
        for (idx, entry) in entries.flatten().enumerate() {
            if (idx + 1) as i32 == id && std::fs::remove_file(entry.path()).is_ok() {
                tracing::info!("Deleted backup: {}", entry.path().display());
                return StatusCode::OK;
            }
        }
    }

    StatusCode::NOT_FOUND
}

/// POST /api/v3/system/backup/restore/:id
pub async fn restore_backup(Path(_id): Path<i32>) -> Json<serde_json::Value> {
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
