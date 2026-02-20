//! FileSystem API endpoints (v5)

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct FileSystemQuery {
    pub path: Option<String>,
    pub include_files: Option<bool>,
    pub allow_folders_without_trailing_slashes: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemResource {
    pub parent: Option<String>,
    pub directories: Vec<FileSystemEntryResource>,
    pub files: Vec<FileSystemEntryResource>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemEntryResource {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub name: String,
    pub path: String,
    pub relative_path: Option<String>,
    pub last_modified: Option<String>,
    pub size: Option<i64>,
}

pub async fn get_filesystem(query: Query<FileSystemQuery>) -> Json<FileSystemResource> {
    let path = query.path.clone().unwrap_or_else(|| "/".to_string());
    let include_files = query.include_files.unwrap_or(false);

    let mut directories = Vec::new();
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.flatten() {
            let file_type = entry.file_type().ok();
            let metadata = entry.metadata().ok();
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            let fs_entry = FileSystemEntryResource {
                entry_type: if file_type.map(|ft| ft.is_dir()).unwrap_or(false) {
                    "folder".to_string()
                } else {
                    "file".to_string()
                },
                name: name.clone(),
                path: entry_path.to_string_lossy().to_string(),
                relative_path: Some(name),
                last_modified: metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
                size: metadata.as_ref().map(|m| m.len() as i64),
            };

            if file_type.map(|ft| ft.is_dir()).unwrap_or(false) {
                directories.push(fs_entry);
            } else if include_files {
                files.push(fs_entry);
            }
        }
    }

    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let parent = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    Json(FileSystemResource {
        parent,
        directories,
        files,
    })
}

pub async fn get_filesystem_type() -> Json<serde_json::Value> {
    Json(serde_json::json!({"type": "folder"}))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_filesystem))
        .route("/type", get(get_filesystem_type))
}
