//! Update API endpoints

use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResource {
    pub version: String,
    pub branch: String,
    pub release_date: String,
    pub file_name: Option<String>,
    pub url: Option<String>,
    pub installed: bool,
    pub installed_on: Option<String>,
    pub installable: bool,
    pub latest: bool,
    pub changes: UpdateChanges,
    pub hash: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChanges {
    pub new: Vec<String>,
    pub fixed: Vec<String>,
}

/// GET /api/v3/update
pub async fn get_updates() -> Json<Vec<UpdateResource>> {
    // Return current version as installed, no updates available
    Json(vec![
        UpdateResource {
            version: "0.1.0".to_string(),
            branch: "main".to_string(),
            release_date: chrono::Utc::now().to_rfc3339(),
            file_name: None,
            url: None,
            installed: true,
            installed_on: Some(chrono::Utc::now().to_rfc3339()),
            installable: false,
            latest: true,
            changes: UpdateChanges {
                new: vec!["Initial Rust port".to_string()],
                fixed: vec![],
            },
            hash: None,
        },
    ])
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_updates))
}
